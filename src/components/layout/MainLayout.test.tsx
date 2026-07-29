import { fireEvent, render, screen, within } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { MainLayout } from './MainLayout';

const mocks = vi.hoisted(() => ({
  clearConversationSelection: vi.fn(),
  loadRequests: vi.fn().mockResolvedValue(undefined),
  lock: vi.fn().mockResolvedValue(undefined),
  notifications: [] as Array<{
    ownerPeerId: string;
    read: boolean;
  }>,
}));

vi.mock('../../stores', () => ({
  useIdentityStore: () => ({
    state: {
      status: 'unlocked',
      identity: {
        peerId: 'peer-local',
        name: 'Harbor Tester',
        relayNameVerified: true,
      },
    },
    lock: mocks.lock,
  }),
  useNetworkStore: () => ({
    isRunning: true,
    status: 'connected',
    stats: { connectedPeers: 2 },
  }),
  useMessagingStore: () => ({ clearConversationSelection: mocks.clearConversationSelection }),
  useContactsStore: (selector: (state: unknown) => unknown) =>
    selector({ requests: [], loadRequests: mocks.loadRequests }),
  useNotificationsStore: (selector: (state: unknown) => unknown) =>
    selector({ notifications: mocks.notifications }),
}));

vi.mock('../../hooks', () => ({
  formatShortcut: () => 'Ctrl+/',
  getShortcutPlatform: () => 'windows',
  HARBOR_SHORTCUT_EVENTS: {
    newPost: 'harbor:new-post',
    showShortcuts: 'harbor:show-shortcuts',
  },
  KEYBOARD_SHORTCUTS: [{ id: 'shortcuts', key: '/', modifier: 'mod' }],
  useKeyboardNavigation: vi.fn(),
}));

vi.mock('../../hooks/useMediaUrl', () => ({ useMediaUrl: () => null }));
vi.mock('../../utils/relayName', () => ({ safeIdentityLabel: () => 'Harbor Tester' }));
vi.mock('../identity', () => ({ requestIdentityVerification: vi.fn() }));
vi.mock('../onboarding', () => ({ OnboardingHero: () => null }));
vi.mock('../common', () => ({
  ComposePostModal: () => null,
  CustomizationPanel: () => null,
  KeyboardShortcutsModal: () => null,
  NotificationCenter: () => <button type="button">Notification center</button>,
}));

function renderLayout() {
  return render(
    <MemoryRouter initialEntries={['/feed']}>
      <MainLayout>
        <div>Page content</div>
      </MainLayout>
    </MemoryRouter>,
  );
}

describe('MainLayout responsive application menu', () => {
  beforeEach(() => {
    mocks.clearConversationSelection.mockClear();
    mocks.loadRequests.mockClear();
    mocks.lock.mockClear();
    mocks.notifications.length = 0;
  });

  it('collects compact utility actions behind an accessible hamburger menu', () => {
    renderLayout();

    const trigger = screen.getByRole('button', { name: 'Application menu' });
    expect(trigger).toHaveAttribute('aria-expanded', 'false');

    fireEvent.click(trigger);

    expect(trigger).toHaveAttribute('aria-expanded', 'true');
    const panel = screen.getByRole('dialog', { name: 'Application menu' });
    expect(panel).toBeInTheDocument();
    expect(within(panel).getByRole('button', { name: /Keyboard shortcuts/ })).toBeInTheDocument();
    expect(within(panel).getByRole('link', { name: 'Settings' })).toBeInTheDocument();
    expect(within(panel).getByRole('button', { name: 'Lock Account' })).toBeInTheDocument();
    expect(within(panel).getByText('2 peers connected')).toBeInTheDocument();
  });

  it('closes the compact menu with Escape and outside interaction', () => {
    renderLayout();
    const trigger = screen.getByRole('button', { name: 'Application menu' });

    fireEvent.click(trigger);
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(screen.queryByRole('dialog', { name: 'Application menu' })).not.toBeInTheDocument();

    fireEvent.click(trigger);
    fireEvent.pointerDown(document.body);
    expect(screen.queryByRole('dialog', { name: 'Application menu' })).not.toBeInTheDocument();
  });

  it('opens the existing lock warning from the compact menu', () => {
    renderLayout();

    fireEvent.click(screen.getByRole('button', { name: 'Application menu' }));
    const applicationMenu = screen.getByRole('dialog', { name: 'Application menu' });
    fireEvent.click(within(applicationMenu).getByRole('button', { name: 'Lock Account' }));

    expect(screen.getByRole('dialog', { name: 'Lock this account?' })).toBeInTheDocument();
  });

  it('surfaces unread notifications on the collapsed menu trigger', () => {
    mocks.notifications.push({ ownerPeerId: 'peer-local', read: false });
    renderLayout();

    expect(
      screen.getByRole('button', { name: 'Application menu, 1 unread notification' }),
    ).toHaveTextContent('1');
  });
});
