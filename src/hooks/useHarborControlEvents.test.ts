import { renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useHarborControlEvents } from './useHarborControlEvents';

const mocks = vi.hoisted(() => {
  const identity = {
    state: { status: 'absent' } as Record<string, unknown>,
    initialize: vi.fn(),
  };
  return {
    listen: vi.fn(),
    emit: vi.fn(),
    identity,
    calling: {
      runtimeSnapshot: { state: 'idle' },
      groupRuntimeSnapshot: { state: 'idle' },
      error: null,
      startOutgoingCall: vi.fn(),
      acceptIncomingCall: vi.fn(),
      declineIncomingCall: vi.fn(),
      hangupActiveCall: vi.fn(),
      startOutgoingGroupCall: vi.fn(),
      acceptIncomingGroupCall: vi.fn(),
      declineIncomingGroupCall: vi.fn(),
      leaveGroupCall: vi.fn(),
    },
    contacts: {
      requests: [],
      loadRequests: vi.fn(),
      respondToRequest: vi.fn(),
    },
  };
});

vi.mock('@tauri-apps/api/event', () => ({ listen: mocks.listen, emit: mocks.emit }));
vi.mock('../stores', () => ({
  useIdentityStore: { getState: () => mocks.identity },
  useCallingStore: { getState: () => mocks.calling },
  useContactsStore: { getState: () => mocks.contacts },
}));

describe('useHarborControlEvents', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.identity.state = { status: 'absent' };
    mocks.identity.initialize.mockImplementation(async () => {
      mocks.identity.state = {
        status: 'unlocked',
        identity: { peerId: 'newly-created-profile' },
      };
    });
    mocks.listen.mockResolvedValue(vi.fn());
    mocks.emit.mockResolvedValue(undefined);
    mocks.calling.startOutgoingCall.mockResolvedValue(undefined);
    mocks.calling.startOutgoingGroupCall.mockResolvedValue(undefined);
  });

  it('refreshes a newly created identity before a profile bridge exists', async () => {
    const view = renderHook(() => useHarborControlEvents());

    await waitFor(() =>
      expect(mocks.listen).toHaveBeenCalledWith('harbor:control', expect.any(Function)),
    );
    const listener = mocks.listen.mock.calls[0][1] as (event: {
      payload: { id: string; action: string; payload: Record<string, unknown> };
    }) => void;
    listener({ payload: { id: 'refresh-1', action: 'identity.refresh', payload: {} } });

    await waitFor(() => expect(mocks.identity.initialize).toHaveBeenCalledOnce());
    await waitFor(() =>
      expect(mocks.emit).toHaveBeenCalledWith('harbor:control-result', {
        id: 'refresh-1',
        ok: true,
        result: mocks.identity.state,
      }),
    );

    view.unmount();
  });

  it('returns control failures without mounting profile-scoped listeners', async () => {
    const view = renderHook(() => useHarborControlEvents());

    await waitFor(() => expect(mocks.listen).toHaveBeenCalledOnce());
    expect(mocks.listen).toHaveBeenCalledWith('harbor:control', expect.any(Function));
    const listener = mocks.listen.mock.calls[0][1] as (event: {
      payload: { id: string; action: string; payload: Record<string, unknown> };
    }) => void;
    listener({ payload: { id: 'unknown-1', action: 'not-supported', payload: {} } });

    await waitFor(() =>
      expect(mocks.emit).toHaveBeenCalledWith('harbor:control-result', {
        id: 'unknown-1',
        ok: false,
        error: 'Unknown Harbor control action: not-supported',
      }),
    );

    view.unmount();
  });

  it('includes explicit profile-listener readiness in state snapshots', async () => {
    const view = renderHook(() => useHarborControlEvents());

    await waitFor(() => expect(mocks.listen).toHaveBeenCalledOnce());
    const listener = mocks.listen.mock.calls[0][1] as (event: {
      payload: { id: string; action: string; payload: Record<string, unknown> };
    }) => void;
    listener({ payload: { id: 'snapshot-1', action: 'state.snapshot', payload: {} } });

    await waitFor(() =>
      expect(mocks.emit).toHaveBeenCalledWith(
        'harbor:control-result',
        expect.objectContaining({
          id: 'snapshot-1',
          ok: true,
          result: expect.objectContaining({ profileEventsReady: false }),
        }),
      ),
    );

    view.unmount();
  });

  it('acknowledges group call startup while the mesh continues asynchronously', async () => {
    let finishGroupStart!: () => void;
    const groupStart = new Promise<void>((resolve) => {
      finishGroupStart = resolve;
    });
    mocks.calling.startOutgoingGroupCall.mockReturnValueOnce(groupStart);
    const view = renderHook(() => useHarborControlEvents());

    await waitFor(() => expect(mocks.listen).toHaveBeenCalledOnce());
    const listener = mocks.listen.mock.calls[0][1] as (event: {
      payload: { id: string; action: string; payload: Record<string, unknown> };
    }) => void;
    listener({
      payload: {
        id: 'group-start-1',
        action: 'group.start',
        payload: { peerIds: ['peer-a', 'peer-b'], video: true },
      },
    });

    await waitFor(() =>
      expect(mocks.emit).toHaveBeenCalledWith('harbor:control-result', {
        id: 'group-start-1',
        ok: true,
        result: mocks.calling.groupRuntimeSnapshot,
      }),
    );
    expect(mocks.calling.startOutgoingGroupCall).toHaveBeenCalledWith(
      ['peer-a', 'peer-b'],
      { video: true },
    );

    finishGroupStart();
    await groupStart;
    view.unmount();
  });

  it('acknowledges direct call startup while media setup continues asynchronously', async () => {
    let finishCallStart!: () => void;
    const callStart = new Promise<void>((resolve) => {
      finishCallStart = resolve;
    });
    mocks.calling.startOutgoingCall.mockReturnValueOnce(callStart);
    const view = renderHook(() => useHarborControlEvents());

    await waitFor(() => expect(mocks.listen).toHaveBeenCalledOnce());
    const listener = mocks.listen.mock.calls[0][1] as (event: {
      payload: { id: string; action: string; payload: Record<string, unknown> };
    }) => void;
    listener({
      payload: {
        id: 'call-start-1',
        action: 'call.start',
        payload: { peerId: 'peer-a', video: true },
      },
    });

    await waitFor(() =>
      expect(mocks.emit).toHaveBeenCalledWith('harbor:control-result', {
        id: 'call-start-1',
        ok: true,
        result: mocks.calling.runtimeSnapshot,
      }),
    );
    expect(mocks.calling.startOutgoingCall).toHaveBeenCalledWith('peer-a', { video: true });

    finishCallStart();
    await callStart;
    view.unmount();
  });

  it('disposes a listener that finishes registering after app unmount', async () => {
    let finishRegistration!: (dispose: () => void) => void;
    const registration = new Promise<() => void>((resolve) => {
      finishRegistration = resolve;
    });
    const dispose = vi.fn();
    mocks.listen.mockReturnValueOnce(registration);

    const view = renderHook(() => useHarborControlEvents());
    await waitFor(() => expect(mocks.listen).toHaveBeenCalledOnce());
    view.unmount();
    finishRegistration(dispose);

    await waitFor(() => expect(dispose).toHaveBeenCalledOnce());
  });
});
