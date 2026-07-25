import { create } from 'zustand';
import type {
  CallSession,
  GroupMembershipSignal,
  HangupReason,
  NetworkEvent,
  SignalingEnvelope,
} from '../types';
import { callingService } from '../services/calling';
import {
  AudioCallRuntime,
  GROUP_CALL_MAX_REMOTE_PARTICIPANTS,
  GroupMeshCallRuntime,
  type AudioCallRuntimeSnapshot,
  type GroupCallRuntimeSnapshot,
} from '../services/callingRuntime';
import { useSettingsStore } from './settings';
import { useIdentityStore } from './identity';
import { callFailureFrom, type CallFailure } from '../utils/callErrors';

interface CallingState {
  activeCalls: CallSession[];
  callHistory: CallSession[];
  isLoading: boolean;
  error: string | null;
  failure: CallFailure | null;
  lastEventPeerId: string | null;
  runtimeSnapshot: AudioCallRuntimeSnapshot;
  groupRuntimeSnapshot: GroupCallRuntimeSnapshot;

  hydrateCalls: () => Promise<void>;
  refreshActiveCalls: () => Promise<void>;
  refreshCallHistory: (limit?: number) => Promise<void>;
  handleBackendEvent: (event: NetworkEvent) => Promise<void>;
  handlePeerDisconnected: (peerId: string) => void;
  startOutgoingCall: (
    peerId: string,
    options?: { video?: boolean; videoDeviceId?: string },
  ) => Promise<void>;
  startOutgoingGroupCall: (
    peerIds: string[],
    options?: { video?: boolean; videoDeviceId?: string; roomId?: string },
  ) => Promise<void>;
  acceptIncomingCall: () => Promise<void>;
  acceptIncomingGroupCall: () => Promise<void>;
  declineIncomingCall: () => Promise<void>;
  declineIncomingGroupCall: () => Promise<void>;
  hangupActiveCall: (reason?: HangupReason) => Promise<void>;
  leaveGroupCall: (reason?: HangupReason) => Promise<void>;
  retryGroupParticipant: (peerId: string) => Promise<void>;
  setCameraEnabled: (enabled: boolean) => Promise<void>;
  setGroupMuted: (muted: boolean) => Promise<void>;
  setGroupCameraEnabled: (enabled: boolean) => Promise<void>;
  enableCallAudio: () => Promise<void>;
  switchCamera: (deviceId?: string) => Promise<void>;
  dismissCallUi: () => void;
  reset: () => void;
}

const idleRuntimeSnapshot: AudioCallRuntimeSnapshot = {
  state: 'idle',
  callId: null,
  peerId: null,
  localPeerId: null,
  direction: null,
  terminalReason: null,
  error: null,
  ice: null,
  mediaMode: 'audio',
  videoRequested: false,
  localVideoEnabled: false,
  localVideoStream: null,
  remoteVideoStream: null,
  remoteVideoAvailable: false,
  remoteAudioBlocked: false,
  cameraError: null,
};

const idleGroupRuntimeSnapshot: GroupCallRuntimeSnapshot = {
  state: 'idle',
  roomId: null,
  topology: 'relay_assisted_mesh_v1',
  maxParticipants: 4,
  mediaMode: 'audio',
  localPeerId: null,
  localMuted: false,
  localCameraEnabled: false,
  participantCount: 1,
  participants: [],
  error: null,
};

const initialState = {
  activeCalls: [] as CallSession[],
  callHistory: [] as CallSession[],
  isLoading: false,
  error: null as string | null,
  failure: null as CallFailure | null,
  lastEventPeerId: null as string | null,
  runtimeSnapshot: idleRuntimeSnapshot,
  groupRuntimeSnapshot: idleGroupRuntimeSnapshot,
};

function failureState(error: unknown, context: string): Pick<CallingState, 'error' | 'failure'> {
  const failure = callFailureFrom(error, context);
  return { error: failure.message, failure };
}

let runtime: AudioCallRuntime | null = null;
let groupRuntime: GroupMeshCallRuntime | null = null;
let pendingIncomingEnvelope: SignalingEnvelope | null = null;
let activeGroupCreatorPeerId: string | null = null;
let activeGroupRosterVersion = 0;
let pendingGroupInvite: GroupMembershipSignal | null = null;
let pendingGroupOffers = new Map<string, SignalingEnvelope>();
let lifecycleGeneration = 0;

function clearGroupLifecycle(disposeState: 'idle' | 'ended' | 'failed' = 'idle') {
  const currentGroupRuntime = groupRuntime;
  groupRuntime = null;
  pendingGroupInvite = null;
  pendingGroupOffers.clear();
  activeGroupCreatorPeerId = null;
  activeGroupRosterVersion = 0;
  currentGroupRuntime?.dispose(disposeState);
}

function getRuntime(set: (state: Partial<CallingState>) => void): AudioCallRuntime {
  if (!runtime) {
    const settings = useSettingsStore.getState();
    const generation = lifecycleGeneration;
    runtime = new AudioCallRuntime({
      iceServers: settings.iceServers,
      onStateChange: (runtimeSnapshot) => {
        if (generation !== lifecycleGeneration) return;
        set({
          runtimeSnapshot,
          ...(runtimeSnapshot.error
            ? failureState(runtimeSnapshot.error, 'voice-video-call-runtime')
            : {}),
        });
      },
    });
  }
  return runtime;
}

function getGroupRuntime(set: (state: Partial<CallingState>) => void): GroupMeshCallRuntime {
  if (!groupRuntime) {
    const settings = useSettingsStore.getState();
    const generation = lifecycleGeneration;
    groupRuntime = new GroupMeshCallRuntime({
      iceServers: settings.iceServers,
      onStateChange: (groupRuntimeSnapshot) => {
        if (generation !== lifecycleGeneration) return;
        set({
          groupRuntimeSnapshot,
          ...(groupRuntimeSnapshot.error
            ? failureState(groupRuntimeSnapshot.error, 'group-call-runtime')
            : {}),
        });
        if (groupRuntimeSnapshot.state === 'failed') {
          clearGroupLifecycle();
          set({ groupRuntimeSnapshot: idleGroupRuntimeSnapshot });
        } else if (
          groupRuntimeSnapshot.state === 'ended' ||
          (groupRuntimeSnapshot.state === 'idle' && groupRuntimeSnapshot.roomId !== null)
        ) {
          clearGroupLifecycle();
          set({ groupRuntimeSnapshot: idleGroupRuntimeSnapshot });
        }
      },
    });
  }
  return groupRuntime;
}

function disposeRuntime() {
  runtime?.dispose();
  runtime = null;
  pendingIncomingEnvelope = null;
  clearGroupLifecycle();
}

export const useCallingStore = create<CallingState>((set, get) => ({
  ...initialState,

  hydrateCalls: async () => {
    const generation = lifecycleGeneration;
    set({ isLoading: true, error: null, failure: null });
    try {
      const [activeCalls, callHistory] = await Promise.all([
        callingService.getActiveCalls(),
        callingService.getCallHistory(),
      ]);
      if (generation !== lifecycleGeneration) return;
      set({ activeCalls, callHistory, isLoading: false });
    } catch (error) {
      if (generation !== lifecycleGeneration) return;
      set({ ...failureState(error, 'hydrate-calls'), isLoading: false });
    }
  },

  refreshActiveCalls: async () => {
    const generation = lifecycleGeneration;
    try {
      const activeCalls = await callingService.getActiveCalls();
      if (generation === lifecycleGeneration) set({ activeCalls, error: null, failure: null });
    } catch (error) {
      if (generation === lifecycleGeneration) set(failureState(error, 'refresh-active-calls'));
    }
  },

  refreshCallHistory: async (limit = 100) => {
    const generation = lifecycleGeneration;
    try {
      const callHistory = await callingService.getCallHistory(limit);
      if (generation === lifecycleGeneration) set({ callHistory, error: null, failure: null });
    } catch (error) {
      if (generation === lifecycleGeneration) set(failureState(error, 'refresh-call-history'));
    }
  },

  handleBackendEvent: async (event: NetworkEvent) => {
    const generation = lifecycleGeneration;
    if (event.type !== 'call_signaling_received') {
      return;
    }

    set({ lastEventPeerId: event.peer_id });
    if (event.message.payload.type === 'group_membership') {
      const membership = event.message.payload.payload;
      if (membership.action === 'terminate') {
        clearGroupLifecycle('ended');
        set({ groupRuntimeSnapshot: idleGroupRuntimeSnapshot });
      } else if (membership.action === 'invite') {
        const identityState = useIdentityStore.getState().state;
        if (identityState.status !== 'unlocked') return;
        clearGroupLifecycle();
        pendingGroupInvite = membership;
        activeGroupCreatorPeerId = membership.creatorPeerId;
        activeGroupRosterVersion = membership.rosterVersion;
        getGroupRuntime(set).prepareIncomingGroupCall(membership, identityState.identity.peerId);
      } else if (membership.action === 'leave') {
        const identityState = useIdentityStore.getState().state;
        const localPeerId =
          identityState.status === 'unlocked' ? identityState.identity.peerId : null;
        if (membership.senderPeerId === localPeerId) {
          clearGroupLifecycle('ended');
          set({ groupRuntimeSnapshot: idleGroupRuntimeSnapshot });
        } else {
          activeGroupCreatorPeerId = membership.creatorPeerId;
          activeGroupRosterVersion = membership.rosterVersion;
          pendingGroupOffers.delete(membership.senderPeerId);
          groupRuntime?.handleParticipantLeft(membership.senderPeerId);
        }
      } else if (membership.action === 'failed') {
        activeGroupCreatorPeerId = membership.creatorPeerId;
        activeGroupRosterVersion = membership.rosterVersion;
        pendingGroupOffers.delete(membership.senderPeerId);
        await groupRuntime?.handleParticipantFailed(
          membership.senderPeerId,
          'Participant could not establish call media.',
        );
      } else if (membership.action === 'decline') {
        activeGroupCreatorPeerId = membership.creatorPeerId;
        activeGroupRosterVersion = membership.rosterVersion;
        pendingGroupOffers.delete(membership.senderPeerId);
        await groupRuntime?.handleParticipantDeclined(membership.senderPeerId);
      } else {
        activeGroupCreatorPeerId = membership.creatorPeerId;
        activeGroupRosterVersion = membership.rosterVersion;
      }
      return;
    }
    if (
      event.message.payload.type === 'offer' &&
      pendingGroupInvite?.participants.includes(event.peer_id)
    ) {
      if (groupRuntime && groupRuntime.getSnapshot().state !== 'ringing') {
        await groupRuntime.acceptParticipantOffer(event.message);
        if (generation !== lifecycleGeneration) return;
        await get().hydrateCalls();
      } else {
        pendingGroupOffers.set(event.peer_id, event.message);
      }
      return;
    }
    const callRuntime = getRuntime(set);
    const snapshot = callRuntime.getSnapshot();

    if (event.message.payload.type === 'offer') {
      const offer = event.message.payload.payload;
      const repeatsCurrentOffer =
        snapshot.callId === offer.callId && snapshot.peerId === offer.callerPeerId;
      if (repeatsCurrentOffer && snapshot.state !== 'incoming') {
        return;
      }
      if (!repeatsCurrentOffer && !['idle', 'ended', 'failed'].includes(snapshot.state)) {
        await callingService.busyCall(offer.callId, offer.callerPeerId);
        if (generation !== lifecycleGeneration) return;
        await get().hydrateCalls();
        return;
      }
      pendingIncomingEnvelope = event.message;
    }

    await callRuntime.handleSignalingEvent(event);
    if (generation !== lifecycleGeneration) return;
    await groupRuntime?.handleSignalingEnvelope(event.message);
    if (generation !== lifecycleGeneration) return;
    await get().hydrateCalls();
  },

  handlePeerDisconnected: (peerId: string) => {
    runtime?.handlePeerDisconnected(peerId);
    groupRuntime?.handlePeerDisconnected(peerId);
  },

  startOutgoingCall: async (peerId: string, options = {}) => {
    const generation = lifecycleGeneration;
    try {
      set({ error: null, failure: null });
      pendingIncomingEnvelope = null;
      await getRuntime(set).startOutgoingCall(peerId, options);
      if (generation !== lifecycleGeneration) return;
      await get().hydrateCalls();
    } catch (error) {
      if (generation === lifecycleGeneration) set(failureState(error, 'start-outgoing-call'));
      throw error;
    }
  },

  startOutgoingGroupCall: async (peerIds: string[], options = {}) => {
    const generation = lifecycleGeneration;
    try {
      set({ error: null, failure: null });
      pendingIncomingEnvelope = null;
      if (peerIds.length > GROUP_CALL_MAX_REMOTE_PARTICIPANTS) {
        throw new Error(
          'Group calls support up to 4 total participants in the selected mesh topology.',
        );
      }
      const identityState = useIdentityStore.getState().state;
      if (identityState.status !== 'unlocked') {
        throw new Error('Unlock your identity before starting a group call.');
      }
      const localPeerId = identityState.identity.peerId;
      const participants = [...new Set([localPeerId, ...peerIds])].sort();
      const membership = await callingService.sendGroupMembership({
        roomId: options.roomId,
        action: 'invite',
        rosterVersion: 1,
        participants,
        mediaMode: options.video ? 'video' : 'audio',
      });
      if (generation !== lifecycleGeneration) return;
      activeGroupCreatorPeerId = membership.creatorPeerId;
      activeGroupRosterVersion = membership.rosterVersion;
      await getGroupRuntime(set).startOutgoingGroupCall(peerIds, {
        ...options,
        roomId: membership.roomId,
        localPeerId,
      });
      if (generation !== lifecycleGeneration) return;
      await get().hydrateCalls();
    } catch (error) {
      if (generation === lifecycleGeneration) {
        clearGroupLifecycle();
        set({ groupRuntimeSnapshot: idleGroupRuntimeSnapshot });
        set(failureState(error, 'start-outgoing-group-call'));
      }
      throw error;
    }
  },

  acceptIncomingCall: async () => {
    const generation = lifecycleGeneration;
    if (!pendingIncomingEnvelope) {
      const error = new Error('No incoming call is available to answer.');
      set(failureState(error, 'accept-call'));
      throw error;
    }

    try {
      set({ error: null, failure: null });
      await getRuntime(set).acceptIncomingCall(pendingIncomingEnvelope);
      if (generation !== lifecycleGeneration) return;
      pendingIncomingEnvelope = null;
      await get().hydrateCalls();
    } catch (error) {
      if (generation === lifecycleGeneration) set(failureState(error, 'accept-incoming-call'));
      throw error;
    }
  },

  acceptIncomingGroupCall: async () => {
    const generation = lifecycleGeneration;
    const membership = pendingGroupInvite;
    if (!membership) {
      const error = new Error('No incoming group call is available to answer.');
      set(failureState(error, 'accept-group-call'));
      throw error;
    }
    try {
      set({ error: null, failure: null });
      await callingService.sendGroupMembership({
        roomId: membership.roomId,
        creatorPeerId: membership.creatorPeerId,
        action: 'join',
        rosterVersion: membership.rosterVersion,
        participants: membership.participants,
        mediaMode: membership.mediaMode,
      });
      if (generation !== lifecycleGeneration) return;
      activeGroupRosterVersion = membership.rosterVersion;
      await getGroupRuntime(set).acceptIncomingGroupCall(membership, [
        ...pendingGroupOffers.values(),
      ]);
      if (generation !== lifecycleGeneration) return;
      pendingGroupOffers.clear();
      await get().hydrateCalls();
    } catch (error) {
      if (generation === lifecycleGeneration) {
        try {
          await callingService.sendGroupMembership({
            roomId: membership.roomId,
            creatorPeerId: membership.creatorPeerId,
            action: 'failed',
            rosterVersion: membership.rosterVersion,
            participants: membership.participants,
            mediaMode: membership.mediaMode,
          });
        } catch (signalError) {
          console.warn(
            '[Call] Failed to publish terminal group participant state:',
            callFailureFrom(signalError, 'group-failure-signal').message,
          );
        }
        clearGroupLifecycle();
        set({ groupRuntimeSnapshot: idleGroupRuntimeSnapshot });
        set(failureState(error, 'accept-incoming-group-call'));
      }
      throw error;
    }
  },

  declineIncomingCall: async () => {
    const generation = lifecycleGeneration;
    const envelope = pendingIncomingEnvelope;
    if (envelope?.payload.type === 'offer') {
      const offer = envelope.payload.payload;
      try {
        await callingService.declineCall(offer.callId, offer.callerPeerId);
        if (generation !== lifecycleGeneration) return;
      } catch (error) {
        if (generation === lifecycleGeneration) set(failureState(error, 'decline-incoming-call'));
        throw error;
      }
    }
    pendingIncomingEnvelope = null;
    runtime?.dispose();
    runtime = null;
    set({ runtimeSnapshot: idleRuntimeSnapshot });
    await get().hydrateCalls();
  },

  declineIncomingGroupCall: async () => {
    const generation = lifecycleGeneration;
    const membership = pendingGroupInvite;
    if (membership) {
      try {
        await callingService.sendGroupMembership({
          roomId: membership.roomId,
          creatorPeerId: membership.creatorPeerId,
          action: 'decline',
          rosterVersion: membership.rosterVersion,
          participants: membership.participants,
          mediaMode: membership.mediaMode,
        });
        if (generation !== lifecycleGeneration) return;
      } catch (error) {
        if (generation === lifecycleGeneration) {
          set(failureState(error, 'decline-incoming-group-call'));
        }
        throw error;
      }
    }
    clearGroupLifecycle();
    set({ groupRuntimeSnapshot: idleGroupRuntimeSnapshot });
  },

  hangupActiveCall: async (reason = 'normal') => {
    const generation = lifecycleGeneration;
    try {
      await runtime?.hangup(reason);
      if (generation !== lifecycleGeneration) return;
      pendingIncomingEnvelope = null;
      await get().hydrateCalls();
    } catch (error) {
      if (generation === lifecycleGeneration) set(failureState(error, 'hangup-call'));
      throw error;
    }
  },

  leaveGroupCall: async (reason = 'normal') => {
    const generation = lifecycleGeneration;
    try {
      const snapshot = groupRuntime?.getSnapshot();
      if (snapshot?.roomId && snapshot.localPeerId && activeGroupCreatorPeerId) {
        const participants = [
          snapshot.localPeerId,
          ...snapshot.participants.map((participant) => participant.peerId),
        ].sort();
        try {
          await callingService.sendGroupMembership({
            roomId: snapshot.roomId,
            creatorPeerId: activeGroupCreatorPeerId,
            action: snapshot.localPeerId === activeGroupCreatorPeerId ? 'terminate' : 'leave',
            rosterVersion: activeGroupRosterVersion + 1,
            participants,
            mediaMode: snapshot.mediaMode,
          });
        } catch (error) {
          console.warn(
            '[Call] Group leave notification was only partially delivered:',
            callFailureFrom(error, 'group-leave-signal').message,
          );
        }
        if (generation !== lifecycleGeneration) return;
        activeGroupRosterVersion += 1;
      }
      await groupRuntime?.leave(reason);
      if (generation !== lifecycleGeneration) return;
      clearGroupLifecycle('ended');
      await get().hydrateCalls();
      if (generation !== lifecycleGeneration) return;
      clearGroupLifecycle();
      set({ groupRuntimeSnapshot: idleGroupRuntimeSnapshot });
    } catch (error) {
      if (generation === lifecycleGeneration) {
        clearGroupLifecycle();
        set({ groupRuntimeSnapshot: idleGroupRuntimeSnapshot });
        set(failureState(error, 'leave-group-call'));
      }
      throw error;
    }
  },

  retryGroupParticipant: async (peerId: string) => {
    const generation = lifecycleGeneration;
    if (!groupRuntime) {
      const error = new Error('No active group call is available to retry.');
      set(failureState(error, 'retry-group-participant'));
      throw error;
    }
    try {
      set({ error: null, failure: null });
      await groupRuntime.retryParticipant(peerId);
      if (generation !== lifecycleGeneration) return;
      await get().hydrateCalls();
    } catch (error) {
      if (generation === lifecycleGeneration) {
        set(failureState(error, 'retry-group-participant'));
      }
      throw error;
    }
  },

  setCameraEnabled: async (enabled: boolean) => {
    const generation = lifecycleGeneration;
    try {
      await runtime?.setCameraEnabled(enabled);
    } catch (error) {
      if (generation === lifecycleGeneration) set(failureState(error, 'set-camera'));
      throw error;
    }
  },

  setGroupMuted: async (muted: boolean) => {
    const generation = lifecycleGeneration;
    try {
      await groupRuntime?.setLocalMuted(muted);
    } catch (error) {
      if (generation === lifecycleGeneration) set(failureState(error, 'set-group-muted'));
      throw error;
    }
  },

  setGroupCameraEnabled: async (enabled: boolean) => {
    const generation = lifecycleGeneration;
    try {
      await groupRuntime?.setCameraEnabled(enabled);
    } catch (error) {
      if (generation === lifecycleGeneration) set(failureState(error, 'set-group-camera'));
      throw error;
    }
  },

  enableCallAudio: async () => {
    const generation = lifecycleGeneration;
    try {
      const groupBlocked = groupRuntime
        ?.getSnapshot()
        .participants.some((participant) => participant.remoteAudioBlocked);
      const enabled = groupBlocked
        ? await groupRuntime?.enableRemoteAudio()
        : await runtime?.enableRemoteAudio();
      if (generation !== lifecycleGeneration) return;
      if (!enabled) {
        throw new Error(
          'Audio is still blocked. Check the app volume and Windows or macOS audio permissions, then try again.',
        );
      }
      set({ error: null, failure: null });
    } catch (error) {
      if (generation === lifecycleGeneration) set(failureState(error, 'enable-call-audio'));
      throw error;
    }
  },

  switchCamera: async (deviceId?: string) => {
    const generation = lifecycleGeneration;
    try {
      await runtime?.switchCamera(deviceId);
    } catch (error) {
      if (generation === lifecycleGeneration) set(failureState(error, 'switch-camera'));
      throw error;
    }
  },

  dismissCallUi: () => {
    disposeRuntime();
    set({
      runtimeSnapshot: idleRuntimeSnapshot,
      groupRuntimeSnapshot: idleGroupRuntimeSnapshot,
      error: null,
      failure: null,
    });
  },

  reset: () => {
    lifecycleGeneration += 1;
    disposeRuntime();
    set(initialState);
  },
}));
