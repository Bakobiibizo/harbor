import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { open } from '@tauri-apps/plugin-dialog';
import { GamesPage, getHarborParentOrigin, parseStoreInstallIntent } from './Games';
import {
  getGameDiscoveryFolder,
  importGamePackage,
  inspectGamePackage,
  listInstalledGames,
} from '../services/games';
import type { GamePackagePreview } from '../types';

vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
vi.mock('../services/games', () => ({
  configureGameDiscoveryFolder: vi.fn(),
  deleteGameSaves: vi.fn(),
  getGameDiscoveryFolder: vi.fn(),
  getStoreGameMetadata: vi.fn(),
  importGamePackage: vi.fn(),
  inspectDiscoveredGamePackages: vi.fn(),
  inspectGamePackage: vi.fn(),
  installStoreGame: vi.fn(),
  listInstalledGames: vi.fn(),
  uninstallGame: vi.fn(),
}));
vi.mock('../games/harborGameRuntime', () => ({
  attachGameInput: vi.fn(() => vi.fn()),
  createHarborGameRuntime: vi.fn(),
}));

const preview: GamePackagePreview = {
  archiveDigest: 'a'.repeat(64),
  byteLength: 2855,
  creatorPeerId: '12D3KooWCreator',
  gameId: 'game-1',
  packageDigest: 'b'.repeat(64),
  permissions: ['save_data'],
  title: 'Golden Runner',
  versionId: 'version-1',
};

describe('GamesPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(listInstalledGames).mockResolvedValue([]);
    vi.mocked(getGameDiscoveryFolder).mockResolvedValue(null);
    vi.mocked(inspectGamePackage).mockResolvedValue(preview);
    vi.mocked(importGamePackage).mockResolvedValue({
      ...preview,
      installedAt: 1,
      lastPlayedAt: null,
      playCount: 0,
      source: 'manual',
      storeApproval: null,
    });
  });

  it('embeds only the trusted sandboxed store and explains offline local play', async () => {
    const view = render(<GamesPage />);
    await waitFor(() => expect(listInstalledGames).toHaveBeenCalled());
    const frame = screen.getByTitle('Neo Grounds approved Harbor games store');
    expect(frame).toHaveAttribute(
      'src',
      'https://games.social-harbor.com/store?parentOrigin=https%3A%2F%2Ftauri.localhost',
    );
    expect(frame).toHaveAttribute('sandbox', 'allow-forms allow-same-origin allow-scripts');
    Object.defineProperty(navigator, 'onLine', { configurable: true, value: false });
    fireEvent(window, new Event('offline'));
    expect(screen.getByText(/installed games remain available/iu)).toBeInTheDocument();
    Object.defineProperty(navigator, 'onLine', { configurable: true, value: true });
    view.unmount();
  });

  it('shows package identity and permissions before manual installation', async () => {
    vi.mocked(open).mockResolvedValue('/tmp/game.harborgame');
    render(<GamesPage />);
    fireEvent.click(screen.getByRole('tab', { name: /library/iu }));
    fireEvent.click(screen.getByRole('button', { name: 'Import .harborgame' }));
    expect(await screen.findByRole('dialog', { name: 'Confirm game installation' })).toBeVisible();
    expect(screen.getByText('Golden Runner')).toBeInTheDocument();
    expect(screen.getByText('12D3KooWCreator')).toBeInTheDocument();
    expect(screen.getByText('save_data')).toBeInTheDocument();
    expect(importGamePackage).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Verify & Install' }));
    await waitFor(() =>
      expect(importGamePackage).toHaveBeenCalledWith('/tmp/game.harborgame', ['save_data']),
    );
  });

  it('rejects forged origins, sources, schemas, identifiers, and remote URLs', () => {
    const valid = {
      gameId: 'game-1',
      source: 'neo-grounds',
      type: 'harbor-game-install-intent',
      version: 1,
      versionId: 'version-1',
    };
    expect(
      parseStoreInstallIntent(
        { data: valid, origin: 'https://games.social-harbor.com', source: window },
        window,
      ),
    ).toEqual(valid);
    expect(
      parseStoreInstallIntent(
        { data: valid, origin: 'https://attacker.invalid', source: window },
        window,
      ),
    ).toBeNull();
    expect(
      parseStoreInstallIntent(
        { data: valid, origin: 'https://games.social-harbor.com', source: null },
        window,
      ),
    ).toBeNull();
    expect(
      parseStoreInstallIntent(
        {
          data: { ...valid, packageUrl: 'https://attacker.invalid/game' },
          origin: 'https://games.social-harbor.com',
          source: window,
        },
        window,
      ),
    ).toBeNull();
    expect(
      parseStoreInstallIntent(
        {
          data: { ...valid, gameId: '../escape' },
          origin: 'https://games.social-harbor.com',
          source: window,
        },
        window,
      ),
    ).toBeNull();
  });

  it('derives only compiled Tauri parent origins', () => {
    expect(getHarborParentOrigin({ origin: 'null', protocol: 'tauri:' } as Location)).toBe(
      'tauri://localhost',
    );
    expect(
      getHarborParentOrigin({ origin: 'https://attacker.invalid', protocol: 'https:' } as Location),
    ).toBe('https://tauri.localhost');
  });
});
