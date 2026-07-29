import { useEffect, useState } from 'react';
import { HashRouter, Routes, Route, Navigate } from 'react-router-dom';
import toast, { Toaster } from 'react-hot-toast';
import { isTauri } from '@tauri-apps/api/core';
import { useIdentityStore, useNetworkStore, useSettingsStore, useAccountsStore } from './stores';
import { useHarborControlEvents, useTauriEvents } from './hooks';
import { MainLayout, WindowsTitleBar } from './components/layout';
import {
  AccountSelection,
  CreateIdentity,
  IdentityInitializationFailure,
  UnlockIdentity,
} from './components/onboarding';
import { IdentityPublishingGate } from './components/identity';
import { AddContactDialog, ErrorBoundary } from './components/common';
import { CallOverlay } from './components/calling/CallOverlay';
import { HarborIcon } from './components/icons';
import {
  BoardsPage,
  ChatPage,
  ContactWallPage,
  WallPage,
  FeedPage,
  NetworkPage,
  SettingsPage,
} from './pages';
import { NamedContactWallPage } from './pages/NamedContactWall';
import { checkForUpdate } from './services/updater';
import { getErrorMessage } from './utils/errors';
import {
  activateProfile,
  isCurrentProfile,
  onProfileSuspend,
  suspendProfile,
  type ProfileToken,
} from './services/profileSession';
import {
  hydrateProfilePersistence,
  resetProfilePersistenceMemory,
} from './services/profilePersistence';
import { resetProfileRuntime } from './services/profileRuntime';

function LoadingScreen() {
  return (
    <div
      className="min-h-screen flex items-center justify-center"
      style={{
        background:
          'linear-gradient(135deg, hsl(var(--harbor-brand-backdrop-start)) 0%, hsl(var(--harbor-brand-backdrop-mid)) 50%, hsl(var(--harbor-brand-backdrop-end)) 100%)',
      }}
    >
      <div className="text-center">
        {/* Animated logo container */}
        <div className="relative mb-8">
          {/* Logo */}
          <div className="relative w-20 h-20 flex items-center justify-center mx-auto">
            <HarborIcon className="w-20 h-20" />
          </div>
        </div>

        {/* Loading text */}
        <h2
          className="text-xl font-semibold mb-2"
          style={{ color: 'hsl(var(--harbor-text-primary))' }}
        >
          Harbor
        </h2>
        <p className="text-sm mb-6" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
          Initializing secure connection...
        </p>

        {/* Loading bar */}
        <div
          className="w-48 h-1 rounded-full mx-auto overflow-hidden"
          style={{ background: 'hsl(var(--harbor-surface-2))' }}
        >
          <div
            className="h-full rounded-full"
            style={{
              background:
                'linear-gradient(90deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
              animation: 'loading-bar 1.5s ease-in-out infinite',
            }}
          />
        </div>
      </div>

      {/* CSS animation for loading bar */}
      <style>{`
        @keyframes loading-bar {
          0% { width: 0%; margin-left: 0%; }
          50% { width: 60%; margin-left: 20%; }
          100% { width: 0%; margin-left: 100%; }
        }
      `}</style>
    </div>
  );
}

function ProfileEventBridge({ token }: { token: ProfileToken }) {
  useTauriEvents(token);
  return null;
}

function GlobalControlBridge() {
  useHarborControlEvents();
  return null;
}

export function AppContent() {
  const { state, initialize } = useIdentityStore();
  const { checkStatus, startNetwork, pendingDeepLinkContact, setPendingDeepLinkContact } =
    useNetworkStore();
  const { autoStartNetwork } = useSettingsStore();
  const { accounts, activeAccount, loading: accountsLoading, loadAccounts } = useAccountsStore();

  // UI state for account flow
  const [showCreateAccount, setShowCreateAccount] = useState(false);
  const [showAccountSelection, setShowAccountSelection] = useState(false);
  const [profileToken, setProfileToken] = useState<ProfileToken | null>(null);
  const [entryReady, setEntryReady] = useState(false);
  const [persistenceReady, setPersistenceReady] = useState(false);
  const [profileHydrationError, setProfileHydrationError] = useState<string | null>(null);
  const [profileHydrationAttempt, setProfileHydrationAttempt] = useState(0);

  useEffect(
    () =>
      onProfileSuspend(() => {
        resetProfileRuntime();
        resetProfilePersistenceMemory();
        setPersistenceReady(false);
        setProfileHydrationError(null);
      }),
    [],
  );

  // Check quietly on launch. Installation remains an explicit user decision in Settings.
  useEffect(() => {
    if (!isTauri()) return;

    checkForUpdate()
      .then((update) => {
        if (update.available) {
          toast.success(`Harbor ${update.version} is available in Settings`, { duration: 8000 });
        }
      })
      .catch((error) => {
        console.warn('Automatic update check failed', error);
      });
  }, []);

  // Load accounts on mount
  useEffect(() => {
    loadAccounts();
  }, [loadAccounts]);

  // Establish the trusted backend-selected namespace before identity or any
  // profile service starts. Persisted profile data is hydrated only after the
  // identity is unlocked.
  useEffect(() => {
    if (accountsLoading) return;
    // A populated session is never rebound in-process. AccountSelection commits
    // the target then relaunches; keep this UI stable during that short handoff.
    if (entryReady && profileToken) return;

    let cancelled = false;
    setEntryReady(false);
    setPersistenceReady(false);
    setProfileHydrationError(null);

    const initializeEntry = async () => {
      if (activeAccount) {
        const token = activateProfile(activeAccount.id);
        setProfileToken(token);
        await initialize();
        if (!cancelled && isCurrentProfile(token)) setEntryReady(true);
        return;
      }

      suspendProfile();
      resetProfileRuntime();
      resetProfilePersistenceMemory();
      setProfileToken(null);
      await initialize();
      if (!cancelled) setEntryReady(true);
    };

    void initializeEntry();
    return () => {
      cancelled = true;
    };
  }, [accountsLoading, activeAccount?.id, initialize]);

  // Identity creation registers the first account in the backend. Refresh the
  // registry so the new trusted profile can be activated before main UI mounts.
  useEffect(() => {
    if (entryReady && state.status === 'unlocked' && !activeAccount && !accountsLoading) {
      void loadAccounts();
    }
  }, [entryReady, state.status, activeAccount, accountsLoading, loadAccounts]);

  // A successful lock suspends the old epoch. Reactivate only the namespace and
  // listener bridge for the locked screen; private persistence stays cleared.
  useEffect(() => {
    if (
      entryReady &&
      state.status === 'locked' &&
      activeAccount &&
      !isCurrentProfile(profileToken)
    ) {
      const token = activateProfile(activeAccount.id);
      setProfileToken(token);
      setPersistenceReady(false);
    }
  }, [entryReady, state.status, activeAccount, profileToken]);

  // Unlocking restores only the selected profile's persisted state. The epoch
  // check prevents a delayed hydration from profile A committing after B starts.
  useEffect(() => {
    if (
      !entryReady ||
      state.status !== 'unlocked' ||
      !profileToken ||
      persistenceReady ||
      !isCurrentProfile(profileToken)
    ) {
      return;
    }

    let cancelled = false;
    void hydrateProfilePersistence()
      .then(() => {
        if (!cancelled && isCurrentProfile(profileToken)) setPersistenceReady(true);
      })
      .catch((error) => {
        if (!cancelled && isCurrentProfile(profileToken)) {
          setProfileHydrationError(getErrorMessage(error));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [entryReady, state.status, profileToken, persistenceReady, profileHydrationAttempt]);

  // Auto-start network when identity is unlocked (if enabled in settings)
  useEffect(() => {
    if (state.status === 'unlocked' && persistenceReady) {
      void checkStatus()
        .then(async () => {
          // Only auto-start if setting is enabled and network isn't already running
          const networkState = useNetworkStore.getState();
          if (autoStartNetwork && !networkState.isRunning) {
            console.log('[Harbor] Auto-starting network...');
            await startNetwork();

            // Auto-connect to public relays for circuit addressing
            console.log('[Harbor] Auto-connecting to public relays...');
            const { connectToPublicRelays } = useNetworkStore.getState();
            try {
              await connectToPublicRelays();
              console.log('[Harbor] Connected to public relays');
            } catch (error) {
              console.error('[Harbor] Failed to connect to public relays:', error);
            }
          }
        })
        .catch((error) => {
          console.error('[Harbor] Automatic network startup failed:', error);
        });
    }
  }, [state.status, persistenceReady, checkStatus, autoStartNetwork, startNetwork]);

  // Loading state
  if (
    accountsLoading ||
    !entryReady ||
    state.status === 'loading' ||
    (state.status === 'unlocked' && !persistenceReady && !profileHydrationError)
  ) {
    return <LoadingScreen />;
  }

  const profileBridge =
    profileToken && isCurrentProfile(profileToken) ? (
      <ProfileEventBridge key={profileToken.epoch} token={profileToken} />
    ) : null;

  if (profileHydrationError) {
    return (
      <IdentityInitializationFailure
        state={{
          status: 'recoverableError',
          source: 'profileStorage',
          error: {
            code: 'PROFILE_STORAGE_ERROR',
            message: profileHydrationError,
            recovery: 'Check local storage access, then retry.',
          },
        }}
        onRetry={() => {
          setProfileHydrationError(null);
          setProfileHydrationAttempt((attempt) => attempt + 1);
        }}
      />
    );
  }

  // Show create account screen if user chose to create new or no accounts exist
  if (showCreateAccount || (accounts.length === 0 && state.status === 'absent')) {
    return (
      <CreateIdentity
        onBack={accounts.length > 0 ? () => setShowCreateAccount(false) : undefined}
      />
    );
  }

  // Account selection is an explicit switch flow. Normal startup opens the active account.
  if (showAccountSelection && accounts.length > 1 && state.status !== 'unlocked') {
    return (
      <>
        {profileBridge}
        <AccountSelection onCreateAccount={() => setShowCreateAccount(true)} />
      </>
    );
  }

  // Only authoritative backend absence may offer account creation.
  if (state.status === 'absent') {
    return <CreateIdentity />;
  }

  if (state.status === 'recoverableError' || state.status === 'fatalError') {
    return (
      <>
        {profileBridge}
        <IdentityInitializationFailure
          state={state}
          onRetry={() => void initialize()}
          onSwitchAccount={accounts.length > 1 ? () => setShowAccountSelection(true) : undefined}
        />
      </>
    );
  }

  // Identity locked - show unlock screen
  if (state.status === 'locked') {
    return (
      <>
        {profileBridge}
        <UnlockIdentity
          onSwitchAccount={
            accounts.length > 1
              ? () => {
                  setShowAccountSelection(true);
                }
              : undefined
          }
        />
      </>
    );
  }

  // Identity unlocked - show main app
  return (
    <>
      {profileBridge}
      <IdentityPublishingGate identity={state.identity}>
        <>
          <MainLayout>
            <Routes>
              <Route path="/chat" element={<ChatPage />} />
              <Route path="/wall" element={<WallPage />} />
              <Route path="/contacts/:peerId/wall" element={<ContactWallPage />} />
              <Route path="/name/:qualifiedName/wall" element={<NamedContactWallPage />} />
              <Route path="/feed" element={<FeedPage />} />
              <Route path="/boards" element={<BoardsPage />} />
              <Route path="/network" element={<NetworkPage />} />
              <Route path="/settings" element={<SettingsPage />} />
              <Route path="*" element={<Navigate to="/chat" replace />} />
            </Routes>
          </MainLayout>
          <CallOverlay />
          {pendingDeepLinkContact && (
            <AddContactDialog
              contactString={pendingDeepLinkContact}
              onClose={() => setPendingDeepLinkContact(null)}
            />
          )}
        </>
      </IdentityPublishingGate>
    </>
  );
}

export default function App() {
  const showWindowsTitleBar = isTauri() && /Windows/i.test(navigator.userAgent);

  return (
    <ErrorBoundary>
      <HashRouter>
        <GlobalControlBridge />
        <div className="flex h-screen flex-col overflow-hidden">
          {showWindowsTitleBar && <WindowsTitleBar />}
          <div className="harbor-app-content min-h-0 flex-1 overflow-hidden">
            <AppContent />
          </div>
        </div>
        <Toaster
          position="bottom-right"
          toastOptions={{
            duration: 3000,
            style: {
              background: 'hsl(222 41% 13%)',
              color: 'hsl(220 14% 96%)',
              border: '1px solid hsl(222 30% 22%)',
              borderRadius: '12px',
              padding: '12px 16px',
              fontSize: '14px',
              boxShadow: '0 10px 40px rgba(0, 0, 0, 0.4)',
            },
            success: {
              iconTheme: {
                primary: 'hsl(152 69% 40%)',
                secondary: 'white',
              },
            },
            error: {
              iconTheme: {
                primary: 'hsl(0 84% 60%)',
                secondary: 'white',
              },
            },
          }}
        />
      </HashRouter>
    </ErrorBoundary>
  );
}
