import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useCallingStore } from './calling';
import { useIdentityStore } from './identity';
import { callingService } from '../services/calling';
import type { CallSession, NetworkEvent } from '../types';

vi.mock('../services/calling', () => ({
  callingService: {
    getActiveCalls: vi.fn(),
    getCallHistory: vi.fn(),
    getActiveGroupCalls: vi.fn(),
    busyCall: vi.fn(),
    declineCall: vi.fn(),
    sendGroupMembership: vi.fn(),
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

const localIdentity = {
  peerId: 'peer-local',
  publicKey: 'public-local',
  x25519Public: 'exchange-local',
  displayName: 'Local caller',
  avatarHash: null,
  bio: null,
  passphraseHint: null,
  createdAt: 1,
  updatedAt: 1,
};

function groupMembershipEvent(
  action: 'invite' | 'leave' | 'decline' | 'failed' | 'terminate',
  rosterVersion: number,
): NetworkEvent {
  return {
    type: 'call_signaling_received',
    peer_id: 'peer-alice',
    message: {
      senderPeerId: 'peer-alice',
      recipientPeerId: 'peer-local',
      payload: {
        type: 'group_membership',
        payload: {
          roomId: 'room-one',
          creatorPeerId: 'peer-alice',
          senderPeerId: 'peer-alice',
          action,
          topology: 'relay_assisted_mesh_v1',
          rosterVersion,
          participants: ['peer-alice', 'peer-local'],
          mediaMode: 'audio',
          nonce: `membership-${rosterVersion}`,
          timestamp: 100 + rosterVersion,
          signature: [],
        },
      },
    },
  };
}

function offerEvent(callId: string): NetworkEvent {
  return {
    type: 'call_signaling_received',
    peer_id: 'peer-alice',
    message: {
      senderPeerId: 'peer-alice',
      recipientPeerId: 'peer-local',
      payload: {
        type: 'offer',
        payload: {
          callId,
          callerPeerId: 'peer-alice',
          calleePeerId: 'peer-local',
          sdp: 'v=0',
          timestamp: 200,
          signature: [],
        },
      },
    },
  };
}

function hangupEvent(callId: string): NetworkEvent {
  return {
    type: 'call_signaling_received',
    peer_id: 'peer-alice',
    message: {
      senderPeerId: 'peer-alice',
      recipientPeerId: 'peer-local',
      payload: {
        type: 'hangup',
        payload: {
          callId,
          senderPeerId: 'peer-alice',
          reason: 'normal',
          timestamp: 201,
          signature: [],
        },
      },
    },
  };
}

describe('useCallingStore', () => {
  beforeEach(() => {
    useCallingStore.getState().reset();
    vi.clearAllMocks();
    useIdentityStore.setState({
      state: { status: 'unlocked', identity: localIdentity },
      error: null,
    });
    vi.mocked(callingService.getActiveCalls).mockResolvedValue([]);
    vi.mocked(callingService.getCallHistory).mockResolvedValue([]);
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

  it('does not revive a persisted group room after a profile restart', async () => {
    vi.mocked(callingService.getActiveGroupCalls).mockResolvedValue([
      {
        roomId: 'stale-room',
        creatorPeerId: 'peer-alice',
        topology: 'relay_assisted_mesh_v1',
        mediaMode: 'audio',
        rosterVersion: 2,
        participants: ['peer-alice', 'peer-local'],
        state: 'active',
        createdAt: 100,
        updatedAt: 200,
      },
    ]);

    await useCallingStore.getState().hydrateCalls();
    await useCallingStore.getState().handleBackendEvent(offerEvent('post-restart-one-to-one'));

    expect(callingService.getActiveGroupCalls).not.toHaveBeenCalled();
    expect(useCallingStore.getState().groupRuntimeSnapshot).toEqual(
      expect.objectContaining({ state: 'idle', roomId: null, participants: [] }),
    );
    expect(useCallingStore.getState().runtimeSnapshot.callId).toBe('post-restart-one-to-one');
  });

  it('turns structured backend failures into safe actionable state', async () => {
    vi.mocked(callingService.getActiveCalls).mockRejectedValue({
      code: 'NETWORK_PEER_UNREACHABLE',
      message: 'Could not reach peer',
      details: 'Network signaling failed',
      privateKey: 'must-not-leak',
    });
    vi.mocked(callingService.getCallHistory).mockResolvedValue([]);

    await useCallingStore.getState().hydrateCalls();

    const state = useCallingStore.getState();
    expect(state.failure?.code).toBe('signaling_failed');
    expect(state.error).toBe('Harbor could not reach this contact to set up the call.');
    expect(JSON.stringify(state.failure)).not.toContain('must-not-leak');
    expect(state.error).not.toContain('[object Object]');
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

  it('treats a repeated signed offer for the current call as idempotent', async () => {
    const duplicate = offerEvent('call-duplicate');

    await useCallingStore.getState().handleBackendEvent(duplicate);
    await useCallingStore.getState().handleBackendEvent(duplicate);

    expect(callingService.busyCall).not.toHaveBeenCalled();
    expect(useCallingStore.getState().runtimeSnapshot).toMatchObject({
      state: 'incoming',
      callId: 'call-duplicate',
      peerId: 'peer-alice',
    });
  });

  it('routes a later one-to-one offer normally after reordered group terminate and hangup', async () => {
    await useCallingStore.getState().handleBackendEvent(groupMembershipEvent('invite', 1));
    await useCallingStore.getState().handleBackendEvent(offerEvent('group-leg'));

    expect(useCallingStore.getState().groupRuntimeSnapshot).toMatchObject({
      state: 'ringing',
      roomId: 'room-one',
    });
    expect(useCallingStore.getState().runtimeSnapshot.state).toBe('idle');

    await useCallingStore.getState().handleBackendEvent(groupMembershipEvent('terminate', 2));
    await useCallingStore.getState().handleBackendEvent(hangupEvent('group-leg'));
    await useCallingStore.getState().handleBackendEvent(offerEvent('one-to-one-after-group'));

    expect(useCallingStore.getState().groupRuntimeSnapshot.state).toBe('idle');
    expect(useCallingStore.getState().runtimeSnapshot).toMatchObject({
      state: 'incoming',
      callId: 'one-to-one-after-group',
      peerId: 'peer-alice',
      direction: 'incoming',
    });
    expect(callingService.busyCall).not.toHaveBeenCalled();
  });

  it('forgets pending group routing after declining an invite', async () => {
    await useCallingStore.getState().handleBackendEvent(groupMembershipEvent('invite', 1));
    await useCallingStore.getState().handleBackendEvent(offerEvent('pending-group-leg'));

    await useCallingStore.getState().declineIncomingGroupCall();
    await useCallingStore.getState().handleBackendEvent(offerEvent('one-to-one-after-decline'));

    expect(useCallingStore.getState().groupRuntimeSnapshot.state).toBe('idle');
    expect(callingService.sendGroupMembership).toHaveBeenCalledWith(
      expect.objectContaining({
        roomId: 'room-one',
        creatorPeerId: 'peer-alice',
        action: 'decline',
        rosterVersion: 1,
      }),
    );
    expect(useCallingStore.getState().runtimeSnapshot).toMatchObject({
      state: 'incoming',
      callId: 'one-to-one-after-decline',
      peerId: 'peer-alice',
    });
  });

  it('forgets group routing when the last remote participant leaves', async () => {
    await useCallingStore.getState().handleBackendEvent(groupMembershipEvent('invite', 1));
    await useCallingStore.getState().handleBackendEvent(offerEvent('pending-group-leg'));

    await useCallingStore.getState().handleBackendEvent(groupMembershipEvent('leave', 2));
    await useCallingStore.getState().handleBackendEvent(offerEvent('one-to-one-after-leave'));

    expect(useCallingStore.getState().groupRuntimeSnapshot.state).toBe('idle');
    expect(useCallingStore.getState().runtimeSnapshot).toMatchObject({
      state: 'incoming',
      callId: 'one-to-one-after-leave',
      peerId: 'peer-alice',
    });
  });

  it('records a terminal participant failure and releases later one-to-one routing', async () => {
    await useCallingStore.getState().handleBackendEvent(groupMembershipEvent('invite', 1));
    await useCallingStore.getState().handleBackendEvent(offerEvent('pending-group-leg'));

    await useCallingStore.getState().handleBackendEvent(groupMembershipEvent('failed', 1));
    await useCallingStore.getState().handleBackendEvent(offerEvent('one-to-one-after-failure'));

    expect(useCallingStore.getState().groupRuntimeSnapshot.state).toBe('idle');
    expect(useCallingStore.getState().runtimeSnapshot).toMatchObject({
      state: 'incoming',
      callId: 'one-to-one-after-failure',
      peerId: 'peer-alice',
    });
  });

  it('turns a signed remote group decline into terminal cleanup', async () => {
    await useCallingStore.getState().handleBackendEvent(groupMembershipEvent('invite', 1));
    await useCallingStore.getState().handleBackendEvent(offerEvent('pending-group-leg'));

    await useCallingStore.getState().handleBackendEvent(groupMembershipEvent('decline', 1));

    expect(useCallingStore.getState().groupRuntimeSnapshot).toEqual(
      expect.objectContaining({ state: 'idle', roomId: null, participants: [] }),
    );
  });

  it('releases failed group setup before routing a later one-to-one offer', async () => {
    const inviteEvent = groupMembershipEvent('invite', 1);
    const invite =
      inviteEvent.type === 'call_signaling_received' &&
      inviteEvent.message.payload.type === 'group_membership'
        ? inviteEvent.message.payload.payload
        : null;
    expect(invite).not.toBeNull();
    vi.mocked(callingService.sendGroupMembership).mockResolvedValue({
      ...invite!,
      action: 'join',
      rosterVersion: 2,
    });

    await useCallingStore.getState().handleBackendEvent(inviteEvent);
    await useCallingStore.getState().handleBackendEvent(offerEvent('failed-group-leg'));
    await expect(useCallingStore.getState().acceptIncomingGroupCall()).rejects.toThrow();

    await useCallingStore.getState().handleBackendEvent(offerEvent('one-to-one-after-failure'));

    expect(useCallingStore.getState().runtimeSnapshot).toMatchObject({
      state: 'incoming',
      callId: 'one-to-one-after-failure',
      peerId: 'peer-alice',
    });
  });

  it('forgets pending group routing across a profile reset', async () => {
    await useCallingStore.getState().handleBackendEvent(groupMembershipEvent('invite', 1));
    await useCallingStore.getState().handleBackendEvent(offerEvent('old-profile-group-leg'));

    useCallingStore.getState().reset();
    await useCallingStore.getState().handleBackendEvent(offerEvent('new-profile-one-to-one'));

    expect(useCallingStore.getState().groupRuntimeSnapshot.state).toBe('idle');
    expect(useCallingStore.getState().runtimeSnapshot).toMatchObject({
      state: 'incoming',
      callId: 'new-profile-one-to-one',
      peerId: 'peer-alice',
    });
  });
});
