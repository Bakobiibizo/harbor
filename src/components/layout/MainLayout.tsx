import { type ReactNode, useState } from 'react';
import { NavLink, useLocation, useNavigate } from 'react-router-dom';
import {
  useIdentityStore,
  useSettingsStore,
  useNetworkStore,
  useMessagingStore,
} from '../../stores';
import { useKeyboardNavigation } from '../../hooks';
import { KeyboardShortcutsModal } from '../common';
import {
  BoardsIcon,
  ChatIcon,
  WallIcon,
  FeedIcon,
  NetworkIcon,
  SettingsIcon,
  LockIcon,
  HarborIcon,
  ChevronRightIcon,
} from '../icons';
import { getInitials } from '../../utils/formatting';

interface MainLayoutProps {
  children: ReactNode;
}

interface NavItem {
  to: string;
  label: string;
  icon: React.ComponentType<{ className?: string; style?: React.CSSProperties }>;
  description: string;
}

const navItems: NavItem[] = [
  {
    to: '/chat',
    label: 'Messages',
    icon: ChatIcon,
    description: 'Direct conversations',
  },
  {
    to: '/wall',
    label: 'My Wall',
    icon: WallIcon,
    description: 'Your posts & content',
  },
  {
    to: '/feed',
    label: 'Feed',
    icon: FeedIcon,
    description: 'Updates from contacts',
  },
  {
    to: '/boards',
    label: 'Boards',
    icon: BoardsIcon,
    description: 'Community discussions',
  },
  {
    to: '/network',
    label: 'Network',
    icon: NetworkIcon,
    description: 'Contacts & connections',
  },
];

export function MainLayout({ children }: MainLayoutProps) {
  const { state, lock } = useIdentityStore();
  const { showOnlineStatus, avatarUrl } = useSettingsStore();
  const { isRunning, status, stats } = useNetworkStore();
  const { clearConversationSelection } = useMessagingStore();
  const location = useLocation();
  const navigate = useNavigate();
  const [isLocking, setIsLocking] = useState(false);

  // Enable keyboard navigation
  useKeyboardNavigation();

  const identity = state.status === 'unlocked' ? state.identity : null;

  // Get indicator color based on network status
  const getStatusColor = () => {
    if (!isRunning) return 'hsl(var(--harbor-text-tertiary))'; // Gray when offline
    if (status === 'connecting') return 'hsl(var(--harbor-warning))'; // Yellow when connecting
    return 'hsl(var(--harbor-success))'; // Green when connected
  };

  // Get status text based on network status
  const getStatusText = () => {
    if (!isRunning) return 'Network offline';
    if (status === 'connecting') return 'Connecting...';
    if (stats.connectedPeers > 0)
      return `${stats.connectedPeers} peer${stats.connectedPeers !== 1 ? 's' : ''} connected`;
    return 'No peers found';
  };

  // Get user indicator color (combines network status + user preference)
  const getUserIndicatorColor = () => {
    if (!isRunning) return 'hsl(var(--harbor-text-tertiary))'; // Gray when network offline
    if (status === 'connecting') return 'hsl(var(--harbor-warning))'; // Yellow when connecting
    if (!showOnlineStatus) return 'hsl(var(--harbor-text-tertiary))'; // Gray when user wants to appear offline
    return 'hsl(var(--harbor-success))'; // Green when online and visible
  };

  const handleLock = async () => {
    setIsLocking(true);
    try {
      await lock();
    } finally {
      setIsLocking(false);
    }
  };

  return (
    <div className="flex h-screen" style={{ background: 'hsl(var(--harbor-bg-primary))' }}>
      {/* Sidebar */}
      <aside
        className="w-72 flex flex-col border-r"
        style={{
          background: 'hsl(var(--harbor-bg-elevated))',
          borderColor: 'hsl(var(--harbor-border-subtle))',
        }}
      >
        {/* App Branding */}
        <div className="p-5 border-b" style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}>
          <div className="flex items-center gap-3">
            <div
              className="w-10 h-10 rounded-xl flex items-center justify-center"
              style={{
                background:
                  'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
                boxShadow: '0 4px 12px hsl(var(--harbor-primary) / 0.3)',
              }}
            >
              <HarborIcon className="w-6 h-6 text-white" />
            </div>
            <div>
              <h1
                className="text-lg font-bold"
                style={{ color: 'hsl(var(--harbor-text-primary))' }}
              >
                Harbor
              </h1>
              <p className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                Decentralized Chat
              </p>
            </div>
          </div>
        </div>

        {/* User Profile Card - clickable to go to profile settings */}
        {identity && (
          <div className="p-4">
            <button
              onClick={() => navigate('/settings')}
              className="w-full p-3 rounded-xl text-left transition-all duration-200 hover:opacity-90"
              style={{
                background: 'hsl(var(--harbor-surface-1))',
                border: '1px solid hsl(var(--harbor-border-subtle))',
              }}
            >
              <div className="flex items-center gap-3">
                <div className="relative">
                  <div
                    className="w-11 h-11 rounded-full flex items-center justify-center text-sm font-semibold text-white overflow-hidden"
                    style={{
                      background: avatarUrl
                        ? 'transparent'
                        : 'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
                    }}
                  >
                    {avatarUrl ? (
                      <img
                        src={avatarUrl}
                        alt=""
                        className="w-full h-full rounded-full object-cover"
                      />
                    ) : identity.avatarHash ? (
                      <img
                        src={`/media/${identity.avatarHash}`}
                        alt=""
                        className="w-full h-full rounded-full object-cover"
                      />
                    ) : (
                      getInitials(identity.displayName)
                    )}
                  </div>
                  {/* Online indicator - reflects actual network status and user preference */}
                  <div
                    className={`absolute -bottom-0.5 -right-0.5 w-3.5 h-3.5 rounded-full border-2 ${
                      isRunning && status === 'connected' ? 'animate-pulse' : ''
                    }`}
                    style={{
                      background: getUserIndicatorColor(),
                      borderColor: 'hsl(var(--harbor-bg-elevated))',
                    }}
                    title={
                      !isRunning
                        ? 'Offline - Network not running'
                        : status === 'connecting'
                          ? 'Connecting...'
                          : !showOnlineStatus
                            ? 'Appearing offline'
                            : 'Online'
                    }
                  />
                </div>
                <div className="flex-1 min-w-0">
                  <p
                    className="font-semibold text-sm truncate"
                    style={{ color: 'hsl(var(--harbor-text-primary))' }}
                  >
                    {identity.displayName}
                  </p>
                  <p
                    className="text-xs truncate"
                    style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                  >
                    {identity.peerId.slice(0, 8)}...{identity.peerId.slice(-6)}
                  </p>
                </div>
              </div>
            </button>
          </div>
        )}

        {/* Navigation */}
        <nav className="flex-1 px-3 py-2 space-y-1 overflow-y-auto hide-scrollbar">
          {navItems.map((item) => {
            const isActive = location.pathname.startsWith(item.to);
            const Icon = item.icon;

            // Handle click - clear conversation selection when clicking Messages
            const handleNavClick = () => {
              if (item.to === '/chat') {
                clearConversationSelection();
              }
            };

            return (
              <NavLink key={item.to} to={item.to} className="group block" onClick={handleNavClick}>
                <div
                  className="flex items-center gap-3 px-3 py-2.5 rounded-xl transition-all duration-200"
                  style={{
                    background: isActive
                      ? 'linear-gradient(135deg, hsl(var(--harbor-primary) / 0.15), hsl(var(--harbor-accent) / 0.1))'
                      : 'transparent',
                    border: isActive
                      ? '1px solid hsl(var(--harbor-primary) / 0.2)'
                      : '1px solid transparent',
                  }}
                >
                  <div
                    className="w-9 h-9 rounded-lg flex items-center justify-center transition-all duration-200"
                    style={{
                      background: isActive
                        ? 'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))'
                        : 'hsl(var(--harbor-surface-2))',
                      boxShadow: isActive ? '0 4px 12px hsl(var(--harbor-primary) / 0.3)' : 'none',
                    }}
                  >
                    <Icon
                      className="w-5 h-5 transition-colors duration-200"
                      style={{
                        color: isActive ? 'white' : 'hsl(var(--harbor-text-secondary))',
                      }}
                    />
                  </div>
                  <div className="flex-1 min-w-0">
                    <p
                      className="text-sm font-medium transition-colors duration-200"
                      style={{
                        color: isActive
                          ? 'hsl(var(--harbor-primary))'
                          : 'hsl(var(--harbor-text-primary))',
                      }}
                    >
                      {item.label}
                    </p>
                    <p
                      className="text-xs truncate"
                      style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                    >
                      {item.description}
                    </p>
                  </div>
                  <ChevronRightIcon
                    className="w-4 h-4 opacity-0 group-hover:opacity-100 transition-opacity duration-200"
                    style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                  />
                </div>
              </NavLink>
            );
          })}
        </nav>

        {/* Bottom Actions */}
        <div
          className="p-3 border-t space-y-1"
          style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
        >
          {/* Settings */}
          <NavLink to="/settings" className="group block">
            {({ isActive }) => (
              <div
                className="flex items-center gap-3 px-3 py-2.5 rounded-xl transition-all duration-200"
                style={{
                  background: isActive ? 'hsl(var(--harbor-surface-1))' : 'transparent',
                }}
              >
                <div
                  className="w-9 h-9 rounded-lg flex items-center justify-center"
                  style={{ background: 'hsl(var(--harbor-surface-2))' }}
                >
                  <SettingsIcon
                    className="w-5 h-5"
                    style={{ color: 'hsl(var(--harbor-text-secondary))' }}
                  />
                </div>
                <span
                  className="text-sm font-medium"
                  style={{ color: 'hsl(var(--harbor-text-primary))' }}
                >
                  Settings
                </span>
              </div>
            )}
          </NavLink>

          {/* Lock Wallet */}
          {identity && (
            <button onClick={handleLock} disabled={isLocking} className="w-full group">
              <div
                className="flex items-center gap-3 px-3 py-2.5 rounded-xl transition-all duration-200 hover:bg-opacity-80"
                style={{
                  background: 'hsl(var(--harbor-surface-1))',
                  opacity: isLocking ? 0.6 : 1,
                }}
              >
                <div
                  className="w-9 h-9 rounded-lg flex items-center justify-center"
                  style={{ background: 'hsl(var(--harbor-warning) / 0.15)' }}
                >
                  <LockIcon className="w-5 h-5" style={{ color: 'hsl(var(--harbor-warning))' }} />
                </div>
                <span
                  className="text-sm font-medium"
                  style={{ color: 'hsl(var(--harbor-text-primary))' }}
                >
                  {isLocking ? 'Locking...' : 'Lock Account'}
                </span>
              </div>
            </button>
          )}
        </div>

        {/* Network Status Footer */}
        <div
          className="px-4 py-3 border-t"
          style={{
            borderColor: 'hsl(var(--harbor-border-subtle))',
            background: 'hsl(var(--harbor-surface-1))',
          }}
        >
          <div className="flex items-center gap-2">
            <div
              className={`w-2 h-2 rounded-full ${
                isRunning && status === 'connected' ? 'animate-pulse' : ''
              }`}
              style={{ background: getStatusColor() }}
            />
            <span className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
              {getStatusText()}
            </span>
          </div>
        </div>
      </aside>

      {/* Main content */}
      <main
        className="flex-1 overflow-auto"
        style={{ background: 'hsl(var(--harbor-bg-primary))' }}
      >
        {children}
      </main>

      {/* Keyboard shortcuts modal */}
      <KeyboardShortcutsModal />
    </div>
  );
}
