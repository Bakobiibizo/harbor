import { type ReactNode, useEffect, useRef, useState } from 'react';
import { NavLink, useLocation, useNavigate } from 'react-router-dom';
import {
  useIdentityStore,
  useNetworkStore,
  useMessagingStore,
  useContactsStore,
  useNotificationsStore,
} from '../../stores';
import {
  formatShortcut,
  getShortcutPlatform,
  HARBOR_SHORTCUT_EVENTS,
  KEYBOARD_SHORTCUTS,
  useKeyboardNavigation,
} from '../../hooks';
import { useMediaUrl } from '../../hooks/useMediaUrl';
import { safeIdentityLabel } from '../../utils/relayName';
import {
  ComposePostModal,
  KeyboardShortcutsModal,
  CustomizationPanel,
  NotificationCenter,
} from '../common';
import { LockAccountDialog } from './LockAccountDialog';
import { OnboardingHero } from '../onboarding';
import { requestIdentityVerification } from '../identity';
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
  InfoIcon,
  MenuIcon,
} from '../icons';

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
  const { isRunning, status, stats } = useNetworkStore();
  const { clearConversationSelection } = useMessagingStore();
  const requests = useContactsStore((store) => store.requests);
  const loadRequests = useContactsStore((store) => store.loadRequests);
  const storedNotifications = useNotificationsStore((store) => store.notifications);
  const location = useLocation();
  const navigate = useNavigate();
  const [isLocking, setIsLocking] = useState(false);
  const [showLockWarning, setShowLockWarning] = useState(false);
  const [isCustomizationOpen, setIsCustomizationOpen] = useState(false);
  const [isComposerOpen, setIsComposerOpen] = useState(false);
  const [isUtilityMenuOpen, setIsUtilityMenuOpen] = useState(false);
  const utilityMenuRef = useRef<HTMLDivElement>(null);

  // Enable keyboard navigation
  useKeyboardNavigation();

  useEffect(() => {
    void loadRequests();
  }, [loadRequests]);

  const incomingRequestCount = requests.filter(
    (request) => request.direction === 'incoming' && request.status === 'review',
  ).length;

  const identity = state.status === 'unlocked' ? state.identity : null;
  const unreadNotificationCount = identity
    ? storedNotifications.filter(
        (notification) => notification.ownerPeerId === identity.peerId && !notification.read,
      ).length
    : 0;
  const avatarMediaUrl = useMediaUrl(identity?.avatarHash);
  const shortcutReference = KEYBOARD_SHORTCUTS.find((shortcut) => shortcut.id === 'shortcuts');

  useEffect(() => {
    const openComposer = () => setIsComposerOpen(true);
    window.addEventListener(HARBOR_SHORTCUT_EVENTS.newPost, openComposer);
    return () => window.removeEventListener(HARBOR_SHORTCUT_EVENTS.newPost, openComposer);
  }, []);

  useEffect(() => {
    setIsUtilityMenuOpen(false);
  }, [location.pathname]);

  useEffect(() => {
    if (!isUtilityMenuOpen) return;

    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setIsUtilityMenuOpen(false);
    };
    const closeOutside = (event: PointerEvent) => {
      if (!utilityMenuRef.current?.contains(event.target as Node)) {
        setIsUtilityMenuOpen(false);
      }
    };

    window.addEventListener('keydown', closeOnEscape);
    window.addEventListener('pointerdown', closeOutside);
    return () => {
      window.removeEventListener('keydown', closeOnEscape);
      window.removeEventListener('pointerdown', closeOutside);
    };
  }, [isUtilityMenuOpen]);

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

  const handleLock = async () => {
    setIsLocking(true);
    try {
      await lock();
      setShowLockWarning(false);
    } finally {
      setIsLocking(false);
    }
  };

  // Generate avatar initials
  const getInitials = (name: string) => {
    return name
      .split(' ')
      .map((n) => n[0])
      .join('')
      .toUpperCase()
      .slice(0, 2);
  };

  return (
    <div
      className="harbor-app-shell flex h-full min-h-0"
      style={{ background: 'hsl(var(--harbor-bg-primary))' }}
    >
      {/* Sidebar */}
      <aside
        className="harbor-sidebar flex shrink-0 flex-col border-r"
        style={{
          background: 'hsl(var(--harbor-bg-elevated))',
          borderColor: 'hsl(var(--harbor-border-subtle))',
        }}
      >
        {/* App Branding - clickable for customization */}
        <div
          className="harbor-sidebar-brand relative border-b p-5"
          style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
        >
          <button
            onClick={() => setIsCustomizationOpen((prev) => !prev)}
            className="harbor-interactive flex items-center gap-3 w-full text-left group transition-opacity duration-200 hover:opacity-80"
            title="Customize Harbor"
          >
            <div className="w-10 h-10 flex items-center justify-center transition-transform duration-200 group-hover:scale-105">
              <HarborIcon className="w-10 h-10" />
            </div>
            <div className="harbor-sidebar-brand-copy min-w-0 flex-1">
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
            <svg
              className="harbor-sidebar-brand-chevron h-4 w-4 transition-transform duration-200"
              style={{
                color: 'hsl(var(--harbor-text-tertiary))',
                transform: isCustomizationOpen ? 'rotate(180deg)' : 'rotate(0deg)',
              }}
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth={2}
            >
              <path strokeLinecap="round" strokeLinejoin="round" d="M19.5 8.25l-7.5 7.5-7.5-7.5" />
            </svg>
          </button>
          <CustomizationPanel
            isOpen={isCustomizationOpen}
            onClose={() => setIsCustomizationOpen(false)}
          />
        </div>

        {/* User profile opens the personal post history. */}
        {identity && (
          <div className="harbor-sidebar-profile p-4">
            <button
              onClick={() => navigate('/wall')}
              aria-label={`Open ${safeIdentityLabel(identity)} profile`}
              title={`Open ${safeIdentityLabel(identity)} profile`}
              className="harbor-sidebar-profile-card harbor-interactive card-interactive w-full rounded-xl p-3 text-left transition-all duration-200 hover:opacity-90"
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
                      background: avatarMediaUrl ? 'transparent' : 'hsl(var(--harbor-surface-3))',
                    }}
                  >
                    {avatarMediaUrl ? (
                      <img
                        src={avatarMediaUrl}
                        alt=""
                        className="w-full h-full rounded-full object-cover"
                      />
                    ) : (
                      getInitials(safeIdentityLabel(identity))
                    )}
                  </div>
                  {/* Local network indicator. Harbor does not publish presence. */}
                  <div
                    className={`absolute -bottom-0.5 -right-0.5 w-3.5 h-3.5 rounded-full border-2 ${
                      isRunning && status === 'connected' ? 'animate-pulse' : ''
                    }`}
                    style={{
                      background: getStatusColor(),
                      borderColor: 'hsl(var(--harbor-bg-elevated))',
                    }}
                    title={
                      !isRunning
                        ? 'Offline - Network not running'
                        : status === 'connecting'
                          ? 'Connecting...'
                          : 'Harbor network connected'
                    }
                  />
                </div>
                <div className="harbor-sidebar-profile-copy min-w-0 flex-1">
                  <p
                    className="font-semibold text-sm truncate"
                    style={{ color: 'hsl(var(--harbor-text-primary))' }}
                  >
                    {safeIdentityLabel(identity)}
                  </p>
                  <p
                    className="text-xs truncate"
                    style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                  >
                    Your Harbor account
                  </p>
                </div>
              </div>
            </button>
          </div>
        )}

        {identity && (
          <div className="harbor-sidebar-compose px-4 pb-2">
            <button
              type="button"
              onClick={() => setIsComposerOpen(true)}
              aria-label="Add post"
              title="Add post"
              className="harbor-sidebar-compose-button harbor-interactive flex w-full items-center justify-center gap-2 rounded-xl px-4 py-3 text-sm font-semibold text-white"
              style={{ background: 'hsl(var(--harbor-primary))' }}
            >
              <WallIcon className="h-5 w-5" />
              <span className="harbor-sidebar-compose-label">Add post</span>
            </button>
          </div>
        )}

        {/* Navigation */}
        <nav className="harbor-sidebar-nav hide-scrollbar flex-1 space-y-1 overflow-y-auto px-3 py-2">
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
              <NavLink
                key={item.to}
                to={item.to}
                aria-label={item.label}
                title={item.label}
                className="harbor-interactive group block rounded-xl"
                onClick={handleNavClick}
              >
                <div
                  className="harbor-sidebar-nav-item flex items-center gap-3 rounded-xl px-3 py-2.5 transition-all duration-200"
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
                  <div className="harbor-sidebar-nav-copy min-w-0 flex-1">
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
                  {item.to === '/network' && incomingRequestCount > 0 && (
                    <span
                      aria-label={`${incomingRequestCount} contact requests awaiting review`}
                      className="harbor-sidebar-request-badge flex h-5 min-w-5 items-center justify-center rounded-full px-1 text-xs font-semibold"
                      style={{
                        background: 'hsl(var(--harbor-accent))',
                        color: 'hsl(var(--harbor-bg-primary))',
                      }}
                    >
                      {incomingRequestCount}
                    </span>
                  )}
                  <ChevronRightIcon
                    className="harbor-sidebar-nav-chevron h-4 w-4 opacity-0 transition-opacity duration-200 group-hover:opacity-100"
                    style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                  />
                </div>
              </NavLink>
            );
          })}
        </nav>

        {/* Bottom Actions */}
        <div
          className="harbor-sidebar-expanded-utilities space-y-1 border-t p-3"
          style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
        >
          <div className="flex items-center gap-3 px-3 py-1.5">
            <NotificationCenter />
            <span
              className="text-sm font-medium"
              style={{ color: 'hsl(var(--harbor-text-primary))' }}
            >
              Notifications
            </span>
          </div>
          <button
            type="button"
            onClick={() =>
              window.dispatchEvent(new CustomEvent(HARBOR_SHORTCUT_EVENTS.showShortcuts))
            }
            className="harbor-interactive flex w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left"
          >
            <div
              className="flex h-9 w-9 items-center justify-center rounded-lg"
              style={{ background: 'hsl(var(--harbor-surface-2))' }}
            >
              <InfoIcon
                className="h-5 w-5"
                style={{ color: 'hsl(var(--harbor-text-secondary))' }}
              />
            </div>
            <span
              className="flex-1 text-sm font-medium"
              style={{ color: 'hsl(var(--harbor-text-primary))' }}
            >
              Keyboard shortcuts
            </span>
            {shortcutReference && (
              <kbd
                className="rounded px-1.5 py-0.5 text-[11px]"
                style={{
                  background: 'hsl(var(--harbor-surface-1))',
                  color: 'hsl(var(--harbor-text-tertiary))',
                }}
              >
                {formatShortcut(shortcutReference, getShortcutPlatform())}
              </kbd>
            )}
          </button>

          {/* Settings */}
          <NavLink to="/settings" className="harbor-interactive group block rounded-xl">
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
            <button
              onClick={() => setShowLockWarning(true)}
              disabled={isLocking}
              className="w-full group"
            >
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
          className="harbor-sidebar-network-footer border-t px-4 py-3"
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

        <div className="harbor-sidebar-compact-utilities relative mt-auto" ref={utilityMenuRef}>
          <button
            type="button"
            aria-label={
              unreadNotificationCount > 0
                ? `Application menu, ${unreadNotificationCount} unread notification${
                    unreadNotificationCount === 1 ? '' : 's'
                  }`
                : 'Application menu'
            }
            aria-expanded={isUtilityMenuOpen}
            aria-controls="harbor-utility-menu"
            aria-haspopup="dialog"
            onClick={() => setIsUtilityMenuOpen((open) => !open)}
            className="harbor-interactive relative mx-auto flex h-11 w-11 items-center justify-center rounded-xl"
            style={{
              background: isUtilityMenuOpen
                ? 'hsl(var(--harbor-primary) / 0.16)'
                : 'hsl(var(--harbor-surface-2))',
              color: isUtilityMenuOpen
                ? 'hsl(var(--harbor-primary))'
                : 'hsl(var(--harbor-text-secondary))',
            }}
          >
            <MenuIcon className="h-5 w-5" />
            {unreadNotificationCount > 0 && (
              <span
                className="absolute -right-1 -top-1 min-w-5 rounded-full px-1 text-[11px] font-semibold"
                style={{
                  background: 'hsl(var(--harbor-accent))',
                  color: 'hsl(var(--harbor-bg-primary))',
                }}
              >
                {unreadNotificationCount > 99 ? '99+' : unreadNotificationCount}
              </span>
            )}
            <span
              aria-hidden="true"
              className={`absolute bottom-1.5 right-1.5 h-2 w-2 rounded-full ${
                isRunning && status === 'connected' ? 'animate-pulse' : ''
              }`}
              style={{ background: getStatusColor() }}
            />
          </button>

          {isUtilityMenuOpen && (
            <div
              id="harbor-utility-menu"
              role="dialog"
              aria-label="Application menu"
              className="harbor-sidebar-utility-popover absolute bottom-0 left-full z-[170] ml-2 w-72 overflow-visible rounded-2xl border p-2 shadow-2xl"
              style={{
                background: 'hsl(var(--harbor-bg-elevated))',
                borderColor: 'hsl(var(--harbor-border-subtle))',
              }}
            >
              <div className="flex items-center gap-3 rounded-xl px-3 py-2">
                <NotificationCenter />
                <span className="text-sm font-medium">Notifications</span>
              </div>
              <button
                type="button"
                onClick={() => {
                  setIsUtilityMenuOpen(false);
                  window.dispatchEvent(new CustomEvent(HARBOR_SHORTCUT_EVENTS.showShortcuts));
                }}
                className="harbor-interactive flex w-full items-center gap-3 rounded-xl px-3 py-2 text-left"
              >
                <InfoIcon className="h-5 w-5" />
                <span className="flex-1 text-sm font-medium">Keyboard shortcuts</span>
                {shortcutReference && (
                  <kbd
                    className="rounded px-1.5 py-0.5 text-[11px]"
                    style={{
                      background: 'hsl(var(--harbor-surface-1))',
                      color: 'hsl(var(--harbor-text-tertiary))',
                    }}
                  >
                    {formatShortcut(shortcutReference, getShortcutPlatform())}
                  </kbd>
                )}
              </button>
              <NavLink
                to="/settings"
                onClick={() => setIsUtilityMenuOpen(false)}
                className="harbor-interactive flex items-center gap-3 rounded-xl px-3 py-2"
              >
                <SettingsIcon className="h-5 w-5" />
                <span className="text-sm font-medium">Settings</span>
              </NavLink>
              {identity && (
                <button
                  type="button"
                  disabled={isLocking}
                  onClick={() => {
                    setIsUtilityMenuOpen(false);
                    setShowLockWarning(true);
                  }}
                  className="harbor-interactive flex w-full items-center gap-3 rounded-xl px-3 py-2 text-left"
                >
                  <LockIcon className="h-5 w-5" style={{ color: 'hsl(var(--harbor-warning))' }} />
                  <span className="text-sm font-medium">
                    {isLocking ? 'Locking...' : 'Lock Account'}
                  </span>
                </button>
              )}
              <div
                className="mt-1 flex items-center gap-2 border-t px-3 pt-2 text-xs"
                style={{
                  borderColor: 'hsl(var(--harbor-border-subtle))',
                  color: 'hsl(var(--harbor-text-tertiary))',
                }}
              >
                <span
                  className={`h-2 w-2 rounded-full ${
                    isRunning && status === 'connected' ? 'animate-pulse' : ''
                  }`}
                  style={{ background: getStatusColor() }}
                />
                <span>{getStatusText()}</span>
              </div>
            </div>
          )}
        </div>
      </aside>

      {/* Main content */}
      <main
        className="harbor-main-content min-w-0 flex-1 overflow-auto"
        style={{ background: 'hsl(var(--harbor-bg-primary))' }}
      >
        {identity && !identity.relayNameVerified && (
          <div
            className="harbor-verification-banner flex flex-wrap items-center justify-center gap-3 px-4 py-2 text-sm"
            style={{
              background: 'hsl(var(--harbor-warning) / .16)',
              color: 'hsl(var(--harbor-warning))',
            }}
          >
            <span>
              Name not verified. Your activity is signed, but other people cannot yet verify this
              account name.
            </span>
            <button
              type="button"
              onClick={requestIdentityVerification}
              className="shrink-0 rounded-md px-3 py-1 text-xs font-semibold transition-all hover:brightness-110 active:scale-95"
              style={{
                color: 'hsl(var(--harbor-bg-primary))',
                background: 'hsl(var(--harbor-warning))',
              }}
            >
              Verify name
            </button>
          </div>
        )}
        {children}
      </main>

      {showLockWarning && (
        <LockAccountDialog
          isLocking={isLocking}
          onCancel={() => setShowLockWarning(false)}
          onConfirm={handleLock}
        />
      )}

      {identity && <OnboardingHero key={identity.peerId} identityId={identity.peerId} />}

      <ComposePostModal isOpen={isComposerOpen} onClose={() => setIsComposerOpen(false)} />

      {/* Keyboard shortcuts modal */}
      <KeyboardShortcutsModal />
    </div>
  );
}
