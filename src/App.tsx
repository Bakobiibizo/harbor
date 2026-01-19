import { useEffect, useState, Component, type ReactNode } from 'react';
import { HashRouter, Routes, Route, Navigate } from 'react-router-dom';
import { Toaster } from 'react-hot-toast';
import { useIdentityStore, useNetworkStore, useSettingsStore, useAccountsStore } from './stores';
import { useTauriEvents } from './hooks';
import { MainLayout } from './components/layout';
import { AccountSelection, CreateIdentity, UnlockIdentity } from './components/onboarding';
import { HarborIcon } from './components/icons';
import { ChatPage, WallPage, FeedPage, NetworkPage, SettingsPage } from './pages';
import type { AccountInfo } from './types';

// Error boundary to catch and display React errors
interface ErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
}

class ErrorBoundary extends Component<{ children: ReactNode }, ErrorBoundaryState> {
  constructor(props: { children: ReactNode }) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error) {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.error('React Error Boundary caught:', error, errorInfo);
  }

  render() {
    if (this.state.hasError) {
      return (
        <div
          style={{
            padding: '40px',
            background: '#1a1a2e',
            color: '#fff',
            minHeight: '100vh',
            fontFamily: 'monospace',
          }}
        >
          <h1 style={{ color: '#ff6b6b', marginBottom: '20px' }}>Something went wrong</h1>
          <pre
            style={{
              background: '#0d0d1a',
              padding: '20px',
              borderRadius: '8px',
              overflow: 'auto',
              whiteSpace: 'pre-wrap',
              wordBreak: 'break-word',
            }}
          >
            {this.state.error?.message}
            {'\n\n'}
            {this.state.error?.stack}
          </pre>
          <button
            onClick={() => window.location.reload()}
            style={{
              marginTop: '20px',
              padding: '10px 20px',
              background: '#6366f1',
              color: 'white',
              border: 'none',
              borderRadius: '8px',
              cursor: 'pointer',
            }}
          >
            Reload App
          </button>
        </div>
      );
    }

    return this.props.children;
  }
}

function LoadingScreen() {
  return (
    <div
      className="min-h-screen flex items-center justify-center"
      style={{
        background:
          'linear-gradient(135deg, hsl(220 91% 8%) 0%, hsl(262 60% 12%) 50%, hsl(220 91% 8%) 100%)',
      }}
    >
      <div className="text-center">
        {/* Animated logo container */}
        <div className="relative mb-8">
          {/* Outer glow ring */}
          <div
            className="absolute inset-0 rounded-full animate-pulse"
            style={{
              background:
                'radial-gradient(circle, hsl(var(--harbor-primary) / 0.3) 0%, transparent 70%)',
              transform: 'scale(2)',
            }}
          />
          {/* Logo */}
          <div
            className="relative w-20 h-20 rounded-2xl flex items-center justify-center mx-auto"
            style={{
              background:
                'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
              boxShadow: '0 8px 32px hsl(var(--harbor-primary) / 0.4)',
            }}
          >
            <HarborIcon className="w-12 h-12 text-white" />
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

function AppContent() {
  const { state, initialize } = useIdentityStore();
  const { checkStatus, startNetwork } = useNetworkStore();
  const { autoStartNetwork } = useSettingsStore();
  const { accounts, loading: accountsLoading, loadAccounts } = useAccountsStore();

  // UI state for account flow
  const [showCreateAccount, setShowCreateAccount] = useState(false);
  const [selectedAccount, setSelectedAccount] = useState<AccountInfo | null>(null);

  // Set up Tauri event listeners for real-time updates from backend
  useTauriEvents();

  // Load accounts on mount
  useEffect(() => {
    loadAccounts();
  }, [loadAccounts]);

  // Initialize identity after accounts are loaded
  useEffect(() => {
    if (!accountsLoading) {
      initialize();
    }
  }, [accountsLoading, initialize]);

  // Auto-start network when identity is unlocked (if enabled in settings)
  useEffect(() => {
    if (state.status === 'unlocked') {
      checkStatus().then(async () => {
        // Only auto-start if setting is enabled and network isn't already running
        const networkState = useNetworkStore.getState();
        if (autoStartNetwork && !networkState.isRunning) {
          console.log('[Harbor] Auto-starting network...');
          await startNetwork();

          // Connect to saved bootstrap nodes
          const settingsState = useSettingsStore.getState();
          if (settingsState.bootstrapNodes.length > 0) {
            console.log('[Harbor] Connecting to saved bootstrap nodes...');
            const { addBootstrapNode } = useNetworkStore.getState();
            for (const node of settingsState.bootstrapNodes) {
              try {
                await addBootstrapNode(node);
                console.log(`[Harbor] Connected to bootstrap node: ${node}`);
              } catch (error) {
                console.error(`[Harbor] Failed to connect to bootstrap node: ${node}`, error);
              }
            }
          }
        }
      });
    }
  }, [state.status, checkStatus, autoStartNetwork, startNetwork]);

  // Loading state
  if (accountsLoading || state.status === 'loading') {
    return <LoadingScreen />;
  }

  // Show create account screen if user chose to create new or no accounts exist
  if (showCreateAccount || (accounts.length === 0 && state.status === 'no_identity')) {
    return (
      <CreateIdentity
        onBack={accounts.length > 0 ? () => setShowCreateAccount(false) : undefined}
      />
    );
  }

  // Multiple accounts exist - show account selection
  if (accounts.length > 1 && state.status !== 'unlocked' && !selectedAccount) {
    return (
      <AccountSelection
        onSelectAccount={(account) => {
          setSelectedAccount(account);
          // Re-initialize identity to load the selected account's data
          initialize();
        }}
        onCreateAccount={() => setShowCreateAccount(true)}
      />
    );
  }

  // No identity in current profile - show create screen
  if (state.status === 'no_identity') {
    return <CreateIdentity />;
  }

  // Identity locked - show unlock screen
  if (state.status === 'locked') {
    return (
      <UnlockIdentity
        onSwitchAccount={
          accounts.length > 1
            ? () => {
                setSelectedAccount(null);
              }
            : undefined
        }
      />
    );
  }

  // Identity unlocked - show main app
  return (
    <MainLayout>
      <Routes>
        <Route path="/chat" element={<ChatPage />} />
        <Route path="/wall" element={<WallPage />} />
        <Route path="/feed" element={<FeedPage />} />
        <Route path="/network" element={<NetworkPage />} />
        <Route path="/settings" element={<SettingsPage />} />
        <Route path="*" element={<Navigate to="/chat" replace />} />
      </Routes>
    </MainLayout>
  );
}

export default function App() {
  return (
    <ErrorBoundary>
      <HashRouter>
        <AppContent />
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
