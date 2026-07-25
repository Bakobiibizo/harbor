import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  ReactiveRefreshCoordinator,
  type ReactiveRefreshHandlers,
} from './reactiveRefresh';

function handlers(): ReactiveRefreshHandlers {
  return {
    contacts: vi.fn(),
    requests: vi.fn(),
    messages: vi.fn(),
    posts: vi.fn(),
    media: vi.fn(),
  };
}

describe('ReactiveRefreshCoordinator', () => {
  afterEach(() => vi.useRealTimers());

  it('deduplicates an event burst into one refresh per affected domain', async () => {
    vi.useFakeTimers();
    const refresh = handlers();
    const coordinator = new ReactiveRefreshCoordinator(refresh, { debounceMs: 25 });

    coordinator.enqueue({ domains: ['posts', 'media'], peerId: 'alice' });
    coordinator.enqueue({ domains: ['posts'], peerId: 'alice' });
    coordinator.enqueue({ domains: ['posts'], peerId: 'bob' });
    await vi.advanceTimersByTimeAsync(25);

    expect(refresh.posts).toHaveBeenCalledTimes(1);
    expect([...(refresh.posts as ReturnType<typeof vi.fn>).mock.calls[0][0]]).toEqual([
      'alice',
      'bob',
    ]);
    expect(refresh.media).toHaveBeenCalledTimes(1);
    coordinator.stop();
  });

  it('runs at most one trailing reconciliation when events arrive in flight', async () => {
    vi.useFakeTimers();
    let release!: () => void;
    const firstPass = new Promise<void>((resolve) => {
      release = resolve;
    });
    const refresh = handlers();
    refresh.posts = vi.fn().mockReturnValueOnce(firstPass).mockResolvedValue(undefined);
    const coordinator = new ReactiveRefreshCoordinator(refresh, { debounceMs: 10 });

    coordinator.enqueue({ domains: ['posts'] });
    await vi.advanceTimersByTimeAsync(10);
    coordinator.enqueue({ domains: ['posts'] });
    coordinator.enqueue({ domains: ['posts'] });
    await vi.advanceTimersByTimeAsync(100);
    expect(refresh.posts).toHaveBeenCalledTimes(1);

    release();
    await Promise.resolve();
    await vi.advanceTimersByTimeAsync(10);
    expect(refresh.posts).toHaveBeenCalledTimes(2);
    coordinator.stop();
  });

  it('uses a bounded local reconciliation fallback without overlapping', async () => {
    vi.useFakeTimers();
    const refresh = handlers();
    const coordinator = new ReactiveRefreshCoordinator(refresh, {
      debounceMs: 0,
      fallbackMs: 5_000,
    });
    coordinator.start();

    await vi.advanceTimersByTimeAsync(5_001);
    for (const domain of ['contacts', 'requests', 'messages', 'posts'] as const) {
      expect(refresh[domain]).toHaveBeenCalledTimes(1);
    }
    expect(refresh.media).not.toHaveBeenCalled();
    coordinator.stop();
  });

  it('can restart cleanly after profile suspension', async () => {
    vi.useFakeTimers();
    const refresh = handlers();
    const coordinator = new ReactiveRefreshCoordinator(refresh, {
      debounceMs: 25,
      fallbackMs: 5_000,
    });

    coordinator.start();
    coordinator.enqueue({ domains: ['posts'], peerId: 'profile-a' });
    coordinator.stop();
    await vi.advanceTimersByTimeAsync(5_001);
    expect(refresh.posts).not.toHaveBeenCalled();

    coordinator.start();
    await vi.advanceTimersByTimeAsync(5_026);
    expect(refresh.posts).toHaveBeenCalledTimes(1);
    coordinator.stop();
  });
});
