import { renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useTauriEvents } from './useTauriEvents';
import { activateProfile, suspendProfile, type ProfileToken } from '../services/profileSession';
import { getProfileEventsReady } from '../services/profileEventReadiness';

const mocks = vi.hoisted(() => {
  const createStore = (state: Record<string, unknown>) => {
    const store = vi.fn(() => state) as ReturnType<typeof vi.fn> & {
      getState: ReturnType<typeof vi.fn>;
      setState: ReturnType<typeof vi.fn>;
      subscribe: ReturnType<typeof vi.fn>;
    };
    store.getState = vi.fn(() => state);
    store.setState = vi.fn();
    store.subscribe = vi.fn(() => vi.fn());
    return store;
  };

  const networkState = {
    isRunning: false,
    status: 'disconnected',
    refreshPeers: vi.fn(),
    refreshStats: vi.fn(),
    setPendingDeepLinkContact: vi.fn(),
  };
  const contactsState = {
    contacts: [],
    requests: [],
    refreshContacts: vi.fn(),
    loadRequests: vi.fn(),
  };

  return {
    listen: vi.fn(),
    emit: vi.fn(),
    coordinator: { start: vi.fn(), stop: vi.fn(), enqueue: vi.fn() },
    poller: { update: vi.fn(), stop: vi.fn() },
    unregisterProfileReset: vi.fn(),
    stores: {
      useNetworkStore: createStore(networkState),
      useContactsStore: createStore(contactsState),
      useMessagingStore: createStore({
        activeConversation: null,
        loadConversations: vi.fn(),
        loadMessages: vi.fn(),
      }),
      useFeedStore: createStore({ loadFeed: vi.fn() }),
      useContactWallStore: createStore({ authorPeerId: null, reconcileWall: vi.fn() }),
      useWallStore: createStore({}),
      useCallingStore: createStore({
        runtimeSnapshot: { peerId: null, state: 'idle' },
        handleBackendEvent: vi.fn(() => Promise.resolve()),
        hydrateCalls: vi.fn(() => Promise.resolve()),
      }),
      useIdentityStore: createStore({ state: { status: 'locked' } }),
      useMediaTransfersStore: createStore({ reset: vi.fn(), apply: vi.fn() }),
      useSettingsStore: createStore({
        contactFeedPollingEnabled: true,
        contactFeedPollIntervalMinutes: 5,
      }),
    },
  };
});

vi.mock('@tauri-apps/api/event', () => ({ listen: mocks.listen, emit: mocks.emit }));
vi.mock('react-hot-toast', () => ({
  default: Object.assign(vi.fn(), {
    success: vi.fn(),
    error: vi.fn(),
    dismiss: vi.fn(),
  }),
}));
vi.mock('../stores', () => mocks.stores);
vi.mock('../services/media', () => ({
  mediaService: { preloadMissingMedia: vi.fn() },
}));
vi.mock('../services/feed', () => ({
  feedService: { fetchContactWall: vi.fn() },
}));
vi.mock('../services/reactiveRefresh', () => ({
  ReactiveRefreshCoordinator: vi.fn(function ReactiveRefreshCoordinator() {
    return mocks.coordinator;
  }),
}));
vi.mock('../services/contactFeedPoller', () => ({
  ContactFeedPoller: vi.fn(function ContactFeedPoller() {
    return mocks.poller;
  }),
}));
vi.mock('../services/harborNotifications', () => ({ notifyHarborEvent: vi.fn() }));
vi.mock('../services/mediaTransferEvents', () => ({
  isMediaTransferEventForIdentity: vi.fn(() => false),
}));
vi.mock('../services/profileRuntimeLifecycle', () => ({
  registerProfileRuntimeReset: vi.fn(() => mocks.unregisterProfileReset),
}));
vi.mock('../utils/relayName', () => ({ safePeerLabel: vi.fn() }));
vi.mock('../utils/errors', () => ({ getErrorMessage: (error: unknown) => String(error) }));
vi.mock('../stores/wall', () => ({ applyPostRelayStatusEvent: vi.fn() }));

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

describe('useTauriEvents listener lifecycle', () => {
  let profileToken: ProfileToken;
  let profileSequence = 0;

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.listen.mockReset();
    suspendProfile();
    profileToken = activateProfile(`listener-test-${++profileSequence}`);
  });

  afterEach(() => suspendProfile());

  it('reports ready only after the full asynchronous listener group commits', async () => {
    const networkRegistration = deferred<() => void>();
    const deepLinkRegistration = deferred<() => void>();
    mocks.listen
      .mockReturnValueOnce(networkRegistration.promise)
      .mockReturnValueOnce(deepLinkRegistration.promise);

    const view = renderHook(() => useTauriEvents(profileToken));
    expect(getProfileEventsReady()).toBe(false);

    networkRegistration.resolve(vi.fn());
    await waitFor(() => expect(mocks.listen).toHaveBeenCalledTimes(2));
    expect(getProfileEventsReady()).toBe(false);

    deepLinkRegistration.resolve(vi.fn());
    await waitFor(() => expect(getProfileEventsReady()).toBe(true));

    view.unmount();
    expect(getProfileEventsReady()).toBe(false);
  });

  it('atomically tears down earlier Tauri listeners when later registration fails', async () => {
    const firstRegistration = deferred<() => void>();
    const disposeFirst = vi.fn();
    const registrationFailure = new Error('deep-link listener failed');
    mocks.listen
      .mockReturnValueOnce(firstRegistration.promise)
      .mockRejectedValueOnce(registrationFailure);
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);

    const { unmount } = renderHook(() => useTauriEvents(profileToken));
    await waitFor(() => expect(mocks.listen).toHaveBeenCalledTimes(1));
    firstRegistration.resolve(disposeFirst);

    await waitFor(() => expect(mocks.listen).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(disposeFirst).toHaveBeenCalledTimes(1));
    expect(mocks.listen).toHaveBeenCalledWith('harbor:network', expect.any(Function));
    expect(mocks.listen).toHaveBeenCalledWith('deep_link_contact', expect.any(Function));
    expect(warn).toHaveBeenCalledWith(
      '[TauriEvent] Listener lifecycle failure:',
      registrationFailure,
    );
    expect(getProfileEventsReady()).toBe(false);

    unmount();
    expect(disposeFirst).toHaveBeenCalledTimes(1);
    warn.mockRestore();
  });

  it('disposes a Tauri listener that resolves after the hook unmounts', async () => {
    const registration = deferred<() => void>();
    const dispose = vi.fn();
    mocks.listen.mockReturnValueOnce(registration.promise);

    const { unmount } = renderHook(() => useTauriEvents(profileToken));
    await waitFor(() => expect(mocks.listen).toHaveBeenCalledTimes(1));
    unmount();
    registration.resolve(dispose);

    await waitFor(() => expect(dispose).toHaveBeenCalledTimes(1));
    expect(mocks.listen).toHaveBeenCalledTimes(1);
  });

  it('keeps a remount independent from an older delayed registration', async () => {
    const oldRegistration = deferred<() => void>();
    const disposeOld = vi.fn();
    const disposeCurrentNetwork = vi.fn();
    const disposeCurrentDeepLink = vi.fn();
    mocks.listen
      .mockReturnValueOnce(oldRegistration.promise)
      .mockResolvedValueOnce(disposeCurrentNetwork)
      .mockResolvedValueOnce(disposeCurrentDeepLink);

    const view = renderHook(({ token }: { token: ProfileToken }) => useTauriEvents(token), {
      initialProps: { token: profileToken },
    });
    await waitFor(() => expect(mocks.listen).toHaveBeenCalledTimes(1));

    const currentToken = activateProfile(`listener-test-${++profileSequence}`);
    expect(getProfileEventsReady()).toBe(false);
    view.rerender({ token: currentToken });
    await waitFor(() => expect(mocks.listen).toHaveBeenCalledTimes(3));
    await waitFor(() => expect(getProfileEventsReady()).toBe(true));
    oldRegistration.resolve(disposeOld);
    await waitFor(() => expect(disposeOld).toHaveBeenCalledTimes(1));
    expect(getProfileEventsReady()).toBe(true);

    expect(disposeCurrentNetwork).not.toHaveBeenCalled();
    expect(disposeCurrentDeepLink).not.toHaveBeenCalled();

    view.unmount();
    expect(disposeCurrentNetwork).toHaveBeenCalledTimes(1);
    expect(disposeCurrentDeepLink).toHaveBeenCalledTimes(1);
    expect(getProfileEventsReady()).toBe(false);
  });

  it('logs received call signaling as a privacy-safe structured summary', async () => {
    mocks.listen.mockResolvedValue(vi.fn());
    const identityState = mocks.stores.useIdentityStore.getState() as {
      state: Record<string, unknown>;
    };
    identityState.state = {
      status: 'unlocked',
      identity: { peerId: 'recipient-peer' },
    };
    const log = vi.spyOn(console, 'log').mockImplementation(() => undefined);
    const view = renderHook(() => useTauriEvents(profileToken));

    await waitFor(() => expect(mocks.listen).toHaveBeenCalledTimes(2));
    const networkListener = mocks.listen.mock.calls.find(
      ([eventName]) => eventName === 'harbor:network',
    )?.[1] as ((event: { payload: Record<string, unknown> }) => void) | undefined;
    expect(networkListener).toBeTypeOf('function');

    networkListener?.({
      payload: {
        type: 'call_signaling_received',
        peer_id: 'sender-peer',
        message: {
          senderPeerId: 'sender-peer',
          recipientPeerId: 'recipient-peer',
          payload: {
            type: 'offer',
            payload: {
              callId: 'call-private-log',
              callerPeerId: 'sender-peer',
              calleePeerId: 'recipient-peer',
              sdp: 'a=ice-pwd:super-secret\r\na=fingerprint:private-fingerprint',
              timestamp: 1,
              signature: [91, 92, 93, 94],
            },
          },
        },
      },
    });

    const logged = JSON.stringify(log.mock.calls);
    expect(logged).toContain('offer');
    expect(logged).toContain('call-private-log');
    expect(logged).toContain('inbound');
    expect(logged).toContain('received');
    expect(logged).toContain('dispatching');
    expect(logged).not.toContain('ice-pwd');
    expect(logged).not.toContain('super-secret');
    expect(logged).not.toContain('fingerprint');
    expect(logged).not.toContain('signature');

    view.unmount();
    identityState.state = { status: 'locked' };
    log.mockRestore();
  });
});
