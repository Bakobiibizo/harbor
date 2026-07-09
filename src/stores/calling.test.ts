import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useCallingStore } from './calling';
import { callingService } from '../services/calling';
import type { CallSession, NetworkEvent } from '../types';

vi.mock('../services/calling', () => ({
  callingService: {
    getActiveCalls: vi.fn(),
    getCallHistory: vi.fn(),
    getActiveGroupCalls: vi.fn(),
    busyCall: vi.fn(),
    declineCall: vi.fn(),
  },
}));

const activeCall: CallSession = {
  callId: 'call-active',
  peerId: 'peer-alice',
  callerPeerId: 'peer-local',
  calleePeerId: 'peer-alice',
  direction: 'outgoing',
  mediaKind: 'audio',
  state: 'ringing',
  startedAt: 100,
  endedAt: null,
  durationSeconds: null,
  terminalReason: null,
};

const endedCall: CallSession = {
  ...activeCall,
  callId: 'call-ended',
  state: 'ended',
  endedAt: 160,
  durationSeconds: 60,
  terminalReason: 'normal',
};

describe('useCallingStore', () => {
  beforeEach(() => {
    useCallingStore.getState().reset();
    vi.clearAllMocks();
    vi.mocked(callingService.getActiveGroupCalls).mockResolvedValue([]);
  });

  it('hydrates active calls and call history from backend persistence', async () => {
    vi.mocked(callingService.getActiveCalls).mockResolvedValue([activeCall]);
    vi.mocked(callingService.getCallHistory).mockResolvedValue([activeCall, endedCall]);

    await useCallingStore.getState().hydrateCalls();

    expect(callingService.getActiveCalls).toHaveBeenCalledTimes(1);
    expect(callingService.getCallHistory).toHaveBeenCalledWith();
    expect(useCallingStore.getState().activeCalls).toEqual([activeCall]);
    expect(useCallingStore.getState().callHistory).toEqual([activeCall, endedCall]);
    expect(useCallingStore.getState().error).toBeNull();
  });

  it('refreshes persisted calls after backend call signaling events', async () => {
    vi.mocked(callingService.getActiveCalls).mockResolvedValue([activeCall]);
    vi.mocked(callingService.getCallHistory).mockResolvedValue([activeCall]);

    const event: NetworkEvent = {
      type: 'call_signaling_received',
      peer_id: 'peer-alice',
      message: {
        senderPeerId: 'peer-alice',
        recipientPeerId: 'peer-local',
        payload: {
          type: 'offer',
          payload: {
            callId: 'call-active',
            callerPeerId: 'peer-alice',
            calleePeerId: 'peer-local',
            sdp: 'v=0',
            timestamp: 100,
            signature: [1, 2, 3],
          },
        },
      },
    };

    await useCallingStore.getState().handleBackendEvent(event);

    expect(useCallingStore.getState().lastEventPeerId).toBe('peer-alice');
    expect(useCallingStore.getState().runtimeSnapshot).toMatchObject({
      state: 'incoming',
      callId: 'call-active',
      peerId: 'peer-alice',
      direction: 'incoming',
    });
    expect(useCallingStore.getState().activeCalls).toEqual([activeCall]);
    expect(callingService.getActiveCalls).toHaveBeenCalledTimes(1);
    expect(callingService.getCallHistory).toHaveBeenCalledTimes(1);
  });

  it('sends busy when an offer arrives during an active call UI session', async () => {
    vi.mocked(callingService.getActiveCalls).mockResolvedValue([activeCall]);
    vi.mocked(callingService.getCallHistory).mockResolvedValue([activeCall]);
    vi.mocked(callingService.busyCall).mockResolvedValue({
      callId: 'call-second',
      senderPeerId: 'peer-local',
      targetPeerId: 'peer-bob',
      reason: 'busy',
      timestamp: 101,
      signature: [],
    });

    await useCallingStore.getState().handleBackendEvent({
      type: 'call_signaling_received',
      peer_id: 'peer-alice',
      message: {
        senderPeerId: 'peer-alice',
        recipientPeerId: 'peer-local',
        payload: {
          type: 'offer',
          payload: {
            callId: 'call-active',
            callerPeerId: 'peer-alice',
            calleePeerId: 'peer-local',
            sdp: 'v=0',
            timestamp: 100,
            signature: [1, 2, 3],
          },
        },
      },
    });

    await useCallingStore.getState().handleBackendEvent({
      type: 'call_signaling_received',
      peer_id: 'peer-bob',
      message: {
        senderPeerId: 'peer-bob',
        recipientPeerId: 'peer-local',
        payload: {
          type: 'offer',
          payload: {
            callId: 'call-second',
            callerPeerId: 'peer-bob',
            calleePeerId: 'peer-local',
            sdp: 'v=0',
            timestamp: 101,
            signature: [4, 5, 6],
          },
        },
      },
    });

    expect(callingService.busyCall).toHaveBeenCalledWith('call-second', 'peer-bob');
    expect(useCallingStore.getState().runtimeSnapshot.peerId).toBe('peer-alice');
  });
});
