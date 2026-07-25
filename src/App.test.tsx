import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { IdentityState } from './types';
import App, { AppContent } from './App';

const mocks = vi.hoisted(() => ({
  initialize: vi.fn(),
  loadAccounts: vi.fn(),
  checkStatus: vi.fn(),
  startNetwork: vi.fn(),
  setPendingDeepLinkContact: vi.fn(),
  hydrateProfilePersistence: vi.fn().mockResolvedValue(undefined),
  useHarborControlEvents: vi.fn(),
  useTauriEvents: vi.fn(),
  publishingGateBlocks: false,
  identityState: { status: 'locked' } as IdentityState,
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
  isTauri: () => false,
}));

vi.mock('./stores', () => {
  const useNetworkStore = Object.assign(
    () => ({
      checkStatus: mocks.checkStatus,
      startNetwork: mocks.startNetwork,
      pendingDeepLinkContact: null,
      setPendingDeepLinkContact: mocks.setPendingDeepLinkContact,
    }),
    { getState: () => ({ isRunning: false }) },
  );
  return {
    useIdentityStore: () => ({
      state: mocks.identityState,
      initialize: mocks.initialize,
    }),
    useNetworkStore,
    useSettingsStore: () => ({ autoStartNetwork: false }),
    useAccountsStore: () => ({
      accounts: [{ id: 'personal' }, { id: 'work' }],
      activeAccount: { id: 'personal' },
      loading: false,
      loadAccounts: mocks.loadAccounts,
    }),
  };
});

vi.mock('./hooks', () => ({
  useHarborControlEvents: mocks.useHarborControlEvents,
  useTauriEvents: mocks.useTauriEvents,
}));
vi.mock('./services/updater', () => ({ checkForUpdate: vi.fn() }));
vi.mock('./services/profilePersistence', () => ({
  hydrateProfilePersistence: mocks.hydrateProfilePersistence,
  resetProfilePersistenceMemory: vi.fn(),
}));
vi.mock('./services/profileRuntime', () => ({ resetProfileRuntime: vi.fn() }));
vi.mock('./components/layout', () => ({
  MainLayout: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="main-layout">{children}</div>
  ),
  WindowsTitleBar: () => null,
}));
vi.mock('./components/onboarding', () => ({
  AccountSelection: () => <div>Choose another account</div>,
  CreateIdentity: () => <div>Create account</div>,
  IdentityInitializationFailure: ({ state }: { state: IdentityState }) => (
    <div>Initialization {state.status}</div>
  ),
  UnlockIdentity: ({ onSwitchAccount }: { onSwitchAccount?: () => void }) => (
    <div>
      <span>Unlock active account</span>
      {onSwitchAccount && <button onClick={onSwitchAccount}>Switch account</button>}
    </div>
  ),
}));
vi.mock('./components/identity', () => ({
  IdentityPublishingGate: ({ children }: { children: React.ReactNode }) =>
    mocks.publishingGateBlocks ? <div>Verify before publishing</div> : <>{children}</>,
}));
vi.mock('./components/common', () => ({
  AddContactDialog: () => null,
  ErrorBoundary: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));
vi.mock('./components/calling/CallOverlay', () => ({ CallOverlay: () => null }));
vi.mock('./components/icons', () => ({ HarborIcon: () => null }));
vi.mock('./pages', () => ({
  BoardsPage: () => null,
  ChatPage: () => null,
  ContactWallPage: () => null,
  WallPage: () => null,
  FeedPage: () => null,
  NetworkPage: () => null,
  SettingsPage: () => null,
}));
vi.mock('./pages/NamedContactWall', () => ({ NamedContactWallPage: () => null }));

describe('account startup flow', () => {
  beforeEach(() => {
    mocks.identityState = {
      status: 'locked',
      identity: {
        peerId: 'peer-personal',
        publicKey: 'public-key',
        x25519Public: 'x25519-key',
        displayName: 'Personal',
        avatarHash: null,
        bio: null,
        passphraseHint: null,
        createdAt: 1,
        updatedAt: 1,
      },
    };
    vi.clearAllMocks();
    mocks.publishingGateBlocks = false;
    mocks.checkStatus.mockResolvedValue(undefined);
  });

  it('opens the active account by default and only shows the chooser after an explicit switch', async () => {
    render(<AppContent />);

    expect(await screen.findByText('Unlock active account')).toBeInTheDocument();
    expect(screen.queryByText('Choose another account')).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Switch account' }));

    expect(screen.getByText('Choose another account')).toBeInTheDocument();
  });

  it('mounts the authenticated control bridge independently of profile readiness', async () => {
    mocks.identityState = { status: 'loading' };

    render(<App />);

    expect(await screen.findByText('Initializing secure connection...')).toBeInTheDocument();
    expect(mocks.useHarborControlEvents).toHaveBeenCalledOnce();
  });

  it('mounts profile event listeners even while publishing is gated', async () => {
    mocks.identityState = {
      status: 'unlocked',
      identity: {
        peerId: 'peer-personal',
        publicKey: 'public-key',
        x25519Public: 'x25519-key',
        displayName: 'Personal',
        avatarHash: null,
        bio: null,
        passphraseHint: null,
        createdAt: 1,
        updatedAt: 1,
      },
    };
    mocks.publishingGateBlocks = true;

    render(
      <MemoryRouter>
        <AppContent />
      </MemoryRouter>,
    );

    expect(await screen.findByText('Verify before publishing')).toBeInTheDocument();
    expect(mocks.useTauriEvents).toHaveBeenCalledOnce();
  });

  it('offers account creation for authoritative absence', async () => {
    mocks.identityState = { status: 'absent' };

    render(<AppContent />);

    expect(await screen.findByText('Create account')).toBeInTheDocument();
  });

  it('keeps initialization loading distinct from absence and failure', async () => {
    mocks.identityState = { status: 'loading' };

    render(<AppContent />);

    expect(await screen.findByText('Initializing secure connection...')).toBeInTheDocument();
    expect(screen.queryByText('Create account')).not.toBeInTheDocument();
    expect(
      screen.queryByText(/Initialization (recoverableError|fatalError)/),
    ).not.toBeInTheDocument();
  });

  it.each(['recoverableError', 'fatalError'] as const)(
    'renders %s without offering account creation',
    async (status) => {
      mocks.identityState = {
        status,
        source: status === 'fatalError' ? 'identityCorruption' : 'accountRegistry',
        error: { code: 'INITIALIZATION_ERROR', message: 'Could not load account' },
      };

      render(<AppContent />);

      expect(await screen.findByText(`Initialization ${status}`)).toBeInTheDocument();
      expect(screen.queryByText('Create account')).not.toBeInTheDocument();
    },
  );

  it('hydrates the selected profile before mounting the unlocked application', async () => {
    mocks.identityState = {
      status: 'unlocked',
      identity: {
        peerId: 'peer-personal',
        publicKey: 'public-key',
        x25519Public: 'x25519-key',
        displayName: 'Personal',
        avatarHash: null,
        bio: null,
        passphraseHint: null,
        createdAt: 1,
        updatedAt: 1,
      },
    };
    let finishHydration!: () => void;
    mocks.hydrateProfilePersistence.mockReturnValueOnce(
      new Promise<void>((resolve) => {
        finishHydration = resolve;
      }),
    );

    render(
      <MemoryRouter>
        <AppContent />
      </MemoryRouter>,
    );

    await waitFor(() => expect(mocks.hydrateProfilePersistence).toHaveBeenCalledOnce());
    expect(screen.getByText('Initializing secure connection...')).toBeInTheDocument();
    expect(screen.queryByTestId('main-layout')).not.toBeInTheDocument();

    await act(async () => finishHydration());

    expect(await screen.findByTestId('main-layout')).toBeInTheDocument();
  });
});
