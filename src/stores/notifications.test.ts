import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useNotificationsStore } from './notifications';

const notification = (overrides = {}) => ({
  dedupeKey: 'message:peer-a:1',
  kind: 'message' as const,
  ownerPeerId: 'peer-local',
  peerId: 'peer-a',
  senderName: '@alice@harbor.social',
  title: 'Message from @alice@harbor.social',
  body: 'Open Harbor to read this message.',
  route: '/chat',
  createdAt: 1_000,
  ...overrides,
});

describe('useNotificationsStore', () => {
  beforeEach(() => {
    vi.stubGlobal('crypto', { randomUUID: vi.fn(() => `notice-${Date.now()}`) });
    useNotificationsStore.getState().reset();
  });

  it('persists read state and deduplicates repeated backend events', () => {
    const first = useNotificationsStore.getState().add(notification());
    expect(first).not.toBeNull();
    expect(useNotificationsStore.getState().add(notification({ createdAt: 2_000 }))).toBeNull();
    useNotificationsStore.getState().markRead(first!.id);
    expect(useNotificationsStore.getState().notifications[0].read).toBe(true);
  });

  it('suppresses muted peers without affecting other contacts', () => {
    useNotificationsStore.getState().setPeerMuted('peer-local', 'peer-a', true);
    expect(useNotificationsStore.getState().add(notification())).toBeNull();
    expect(
      useNotificationsStore
        .getState()
        .add(notification({ peerId: 'peer-b', dedupeKey: 'message:peer-b:1' })),
    ).not.toBeNull();
  });

  it('keeps mute state isolated between local accounts', () => {
    useNotificationsStore.getState().setPeerMuted('peer-local', 'peer-a', true);
    expect(
      useNotificationsStore.getState().add(notification({ ownerPeerId: 'peer-other' })),
    ).not.toBeNull();
  });
});
