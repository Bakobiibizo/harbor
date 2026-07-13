import { beforeEach, describe, expect, it, vi } from 'vitest';
import { notifyHarborEvent } from './harborNotifications';
import { useNotificationsStore } from '../stores/notifications';

vi.mock('./nativeNotifications', () => ({ sendNativeHarborNotification: vi.fn() }));
vi.mock('../stores/identity', () => ({
  useIdentityStore: {
    getState: () => ({ state: { status: 'unlocked', identity: { peerId: 'peer-local' } } }),
  },
}));

describe('notifyHarborEvent', () => {
  beforeEach(() => {
    vi.stubGlobal('crypto', { randomUUID: vi.fn(() => 'notice-1') });
    useNotificationsStore.getState().reset();
  });

  it('keeps message content out of persisted and native notification copy', () => {
    const item = notifyHarborEvent({
      kind: 'message',
      peerId: 'peer-a',
      senderName: '@alice@harbor.social',
      eventId: 'event-1',
    });
    expect(item?.title).toContain('@alice@harbor.social');
    expect(item?.body).toBe('Open Harbor to read this private message.');
    expect(JSON.stringify(item)).not.toContain('peer-a payload');
  });

  it('deduplicates repeated call events by signed event identifier', () => {
    const input = {
      kind: 'incoming_call' as const,
      peerId: 'peer-a',
      senderName: '@alice@harbor.social',
      eventId: 'call-1',
      mediaMode: 'video' as const,
    };
    expect(notifyHarborEvent(input)?.title).toBe('Incoming video call');
    expect(notifyHarborEvent(input)).toBeNull();
  });
});
