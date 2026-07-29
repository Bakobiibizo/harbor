import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import toast from 'react-hot-toast';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { playMessageSound } from '../services/audioNotifications';
import { mediaService } from '../services/media';
import { activateProfile, suspendProfile } from '../services/profileSession';
import {
  useAccountsStore,
  useIdentityStore,
  useNotificationsStore,
  useSettingsStore,
} from '../stores';
import { SettingsPage } from './Settings';

vi.mock('../hooks', () => ({ useAppVersion: () => 'v9.8.7' }));
vi.mock('../services/audioNotifications', () => ({ playMessageSound: vi.fn() }));
vi.mock('../services/updater', () => ({
  checkForUpdate: vi.fn(),
  downloadAndInstallUpdate: vi.fn(),
}));
vi.mock('../services/nativeNotifications', () => ({
  requestNativeNotificationPermission: vi.fn(),
}));
vi.mock('../services/mediaPermissions', () => ({
  requestCallMediaAccess: vi.fn(),
}));
vi.mock('../services/media', () => ({
  mediaService: {
    getCacheDiagnostics: vi.fn(),
    updateCacheSettings: vi.fn(),
  },
}));
vi.mock('../components/identity', () => ({
  BugReportForm: () => <div>Bug reporting settings</div>,
  MentionInbox: () => <div>Mention review settings</div>,
  requestIdentityVerification: vi.fn(),
}));
vi.mock('../components/account', () => ({
  IdentityBackupActions: () => <div>Encrypted account backup controls</div>,
}));
vi.mock('@tauri-apps/plugin-process', () => ({ relaunch: vi.fn() }));

const identity = {
  peerId: 'peer-settings',
  publicKey: 'public-key',
  x25519Public: 'x25519-key',
  displayName: 'Settings Tester',
  relayNameClaim: null,
  relayNameVerified: false,
  avatarHash: null,
  bio: null,
  passphraseHint: null,
  createdAt: 1,
  updatedAt: 1,
};

const mediaDiagnostics = {
  settings: {
    enabled: true,
    retentionSeconds: 604_800,
    maxBytes: 536_870_912,
  },
  cachedBytes: 0,
  cachedCount: 0,
  pendingCount: 0,
  evictedLastRun: 0,
};

function renderSettingsRoute() {
  render(
    <MemoryRouter initialEntries={['/settings']}>
      <Routes>
        <Route path="/settings" element={<SettingsPage />} />
      </Routes>
    </MemoryRouter>,
  );
}

function openSection(name: string) {
  const controls = screen.getAllByRole('button', { name: new RegExp(`^${name}`) });
  fireEvent.click(controls[0]);
}

describe('routed Settings implementation', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    suspendProfile();
    activateProfile(identity.peerId);
    useIdentityStore.setState({
      state: { status: 'unlocked', identity },
      error: null,
    });
    useAccountsStore.setState({
      activeAccount: { id: 'account-settings', peerId: identity.peerId } as never,
      accounts: [{ id: 'account-settings', peerId: identity.peerId } as never],
      loading: false,
      error: null,
    });
    useNotificationsStore.getState().reset();
    useSettingsStore.setState({
      soundEnabled: false,
      showReadReceipts: false,
      readReceiptsStatus: 'ready',
      readReceiptsError: null,
      defaultVisibility: 'contacts',
      providerEmbedConsent: 'per-use',
      theme: 'system',
      iceServers: [],
    });
    vi.mocked(mediaService.getCacheDiagnostics).mockResolvedValue(mediaDiagnostics);
    vi.mocked(mediaService.updateCacheSettings).mockResolvedValue(mediaDiagnostics);
  });

  it.each([
    ['Profile', 'Manage your identity and how others see you'],
    ['Appearance', 'Customize how Harbor looks'],
    ['Security', 'Manage your password and encryption keys'],
    ['Calls', 'Help Harbor connect calls across different networks'],
    ['Notifications', 'Choose how Harbor alerts you to private messages and calls'],
    ['Privacy', 'Control who can see your content'],
    ['Updates', 'Keep Harbor up to date with the latest features and fixes'],
    ['Support', 'Review private mentions and report problems to Harbor.'],
  ])('routes the %s navigation control to its live section', (section, description) => {
    renderSettingsRoute();
    openSection(section);

    expect(screen.getByRole('heading', { name: section, level: 3 })).toBeInTheDocument();
    expect(screen.getByText(description)).toBeInTheDocument();
  });

  it('exposes the persisted sound control and previews sound only after enabling it', () => {
    renderSettingsRoute();
    openSection('Notifications');
    const sound = screen.getByRole('switch', { name: 'Sound notifications' });

    expect(sound).toHaveAttribute('aria-checked', 'false');
    fireEvent.click(sound);

    expect(useSettingsStore.getState().soundEnabled).toBe(true);
    expect(playMessageSound).toHaveBeenCalledTimes(1);
    expect(toast.success).toHaveBeenCalledWith('Sound notifications enabled');

    fireEvent.click(sound);
    expect(useSettingsStore.getState().soundEnabled).toBe(false);
    expect(playMessageSound).toHaveBeenCalledTimes(1);
    expect(toast.success).toHaveBeenCalledWith('Sound notifications disabled');
  });

  it('shows the same installed version in the sidebar and update diagnostics', () => {
    renderSettingsRoute();

    expect(screen.getByText('Harbor v9.8.7')).toBeInTheDocument();
    openSection('Updates');
    expect(screen.getByText('v9.8.7')).toBeInTheDocument();
    expect(screen.queryByText(/Harbor v1\.0\.0/)).not.toBeInTheDocument();
  });

  it('keeps the live privacy policy controls and omits the discarded fake presence control', async () => {
    renderSettingsRoute();
    openSection('Privacy');

    expect(screen.queryByText(/show online status/i)).not.toBeInTheDocument();
    expect(screen.getByText(/controls signed read acknowledgements/i)).toBeInTheDocument();
    await waitFor(() => expect(mediaService.getCacheDiagnostics).toHaveBeenCalledTimes(1));
  });

  it('keeps encrypted backup and authenticated deletion reachable in live Security', () => {
    renderSettingsRoute();
    openSection('Security');

    expect(screen.getByText('Encrypted account backup controls')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Delete Account' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Update Password' })).toBeInTheDocument();
  });
});
