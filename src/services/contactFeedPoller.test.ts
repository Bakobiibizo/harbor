import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Contact, ContactRequest } from '../types';
import { ContactFeedPoller, eligibleContactIds } from './contactFeedPoller';

function contact(peerId: string, isBlocked = false): Contact {
  return {
    id: 1,
    peerId,
    publicKey: '',
    x25519Public: '',
    displayName: peerId,
    avatarHash: null,
    bio: null,
    isBlocked,
    trustLevel: 0,
    lastSeenAt: null,
    addedAt: 1,
    updatedAt: 1,
  };
}

function request(peerId: string, status: ContactRequest['status'], updatedAt = 1): ContactRequest {
  return {
    requestId: `${peerId}-${updatedAt}`,
    peerId,
    direction: 'outgoing',
    displayName: peerId,
    status,
    error: null,
    createdAt: 1,
    updatedAt,
  };
}

function context(
  contacts: Contact[],
  requests: ContactRequest[] = [],
  profileId: string | null = 'profile-a',
) {
  return {
    profileId,
    online: true,
    enabled: true,
    intervalMs: 100,
    contacts,
    requests,
  };
}

function options() {
  return {
    concurrency: 2,
    startupJitterMs: 0,
    jitterRatio: 0,
    backoffBaseMs: 10,
    maxBackoffMs: 100,
    minIntervalMs: 1,
  };
}

describe('ContactFeedPoller', () => {
  afterEach(() => {
    vi.restoreAllMocks();
    vi.useRealTimers();
  });

  it('publishes through the refresh path when the durable cursor advances', async () => {
    vi.useFakeTimers();
    const fetchContact = vi
      .fn()
      .mockResolvedValueOnce({ changed: false })
      .mockResolvedValueOnce({ changed: true });
    const publishRefresh = vi.fn();
    const poller = new ContactFeedPoller({ fetchContact, publishRefresh }, options());

    poller.update(context([contact('alice')], [request('alice', 'accepted')]));
    await vi.advanceTimersByTimeAsync(0);
    expect(fetchContact).toHaveBeenCalledTimes(1);
    expect(publishRefresh).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(100);
    expect(fetchContact).toHaveBeenCalledTimes(2);
    expect(publishRefresh).toHaveBeenCalledWith('alice');
    poller.stop();
  });

  it('coalesces repeated updates and bounds a contact burst', async () => {
    vi.useFakeTimers();
    let active = 0;
    let maximumActive = 0;
    const releases: Array<() => void> = [];
    const fetchContact = vi.fn(async () => {
      active += 1;
      maximumActive = Math.max(maximumActive, active);
      await new Promise<void>((resolve) => releases.push(resolve));
      active -= 1;
    });
    const poller = new ContactFeedPoller({ fetchContact, publishRefresh: vi.fn() }, options());
    const state = context(['a', 'b', 'c', 'd'].map((id) => contact(id)));
    poller.update(state);
    poller.update(state);
    poller.update(state);
    await vi.advanceTimersByTimeAsync(0);
    expect(fetchContact).toHaveBeenCalledTimes(2);
    expect(maximumActive).toBe(2);

    releases.splice(0).forEach((release) => release());
    await vi.advanceTimersByTimeAsync(0);
    expect(fetchContact).toHaveBeenCalledTimes(4);
    releases.splice(0).forEach((release) => release());
    await vi.advanceTimersByTimeAsync(0);
    expect(maximumActive).toBe(2);
    poller.stop();
  });

  it('backs off exponentially and returns to the normal interval after recovery', async () => {
    vi.useFakeTimers();
    vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    const fetchContact = vi
      .fn()
      .mockRejectedValueOnce(new Error('offline'))
      .mockRejectedValueOnce(new Error('still offline'))
      .mockResolvedValue(undefined);
    const poller = new ContactFeedPoller({ fetchContact, publishRefresh: vi.fn() }, options());
    poller.update(context([contact('alice')]));

    await vi.advanceTimersByTimeAsync(0);
    expect(fetchContact).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(9);
    expect(fetchContact).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(fetchContact).toHaveBeenCalledTimes(2);
    await vi.advanceTimersByTimeAsync(19);
    expect(fetchContact).toHaveBeenCalledTimes(2);
    await vi.advanceTimersByTimeAsync(1);
    expect(fetchContact).toHaveBeenCalledTimes(3);
    await vi.advanceTimersByTimeAsync(99);
    expect(fetchContact).toHaveBeenCalledTimes(3);
    await vi.advanceTimersByTimeAsync(1);
    expect(fetchContact).toHaveBeenCalledTimes(4);
    poller.stop();
  });

  it('cancels stale publication across lock and profile changes', async () => {
    vi.useFakeTimers();
    const releases = new Map<string, () => void>();
    const fetchContact = vi.fn(
      (peerId: string) =>
        new Promise<void>((resolve) => {
          releases.set(peerId, resolve);
        }),
    );
    const publishRefresh = vi.fn();
    const poller = new ContactFeedPoller({ fetchContact, publishRefresh }, options());
    poller.update(context([contact('alice')]));
    await vi.advanceTimersByTimeAsync(0);

    poller.update(context([contact('alice')], [], null));
    poller.update(context([contact('bob')], [], 'profile-b'));
    await vi.advanceTimersByTimeAsync(0);
    expect(fetchContact).toHaveBeenLastCalledWith('bob');
    releases.get('alice')?.();
    await vi.advanceTimersByTimeAsync(0);
    expect(publishRefresh).not.toHaveBeenCalled();
    releases.get('bob')?.();
    await vi.advanceTimersByTimeAsync(0);
    expect(publishRefresh).toHaveBeenCalledWith('bob');
    poller.stop();
  });

  it('polls only accepted, unblocked contacts and reacts to authorization changes', () => {
    const contacts = [
      contact('accepted'),
      contact('legacy'),
      contact('blocked', true),
      contact('revoked'),
    ];
    const requests = [
      request('accepted', 'pending', 1),
      request('accepted', 'accepted', 2),
      request('blocked', 'accepted'),
      request('revoked', 'revoked'),
    ];
    expect(eligibleContactIds(contacts, requests)).toEqual(['accepted', 'legacy']);
  });
});
