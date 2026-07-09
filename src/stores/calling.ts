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

interface CallingState {
  activeCalls: CallSession[];
  callHistory: CallSession[];
  isLoading: boolean;
  error: string | null;
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
  setCameraEnabled: (enabled: boolean) => Promise<void>;
  setGroupMuted: (muted: boolean) => Promise<void>;
  setGroupCameraEnabled: (enabled: boolean) => Promise<void>;
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
  lastEventPeerId: null as string | null,
  runtimeSnapshot: idleRuntimeSnapshot,
  groupRuntimeSnapshot: idleGroupRuntimeSnapshot,
};

function toErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

let runtime: AudioCallRuntime | null = null;
let groupRuntime: GroupMeshCallRuntime | null = null;
let pendingIncomingEnvelope: SignalingEnvelope | null = null;
let activeGroupCreatorPeerId: string | null = null;
let activeGroupRosterVersion = 0;
let pendingGroupInvite: GroupMembershipSignal | null = null;
let pendingGroupOffers = new Map<string, SignalingEnvelope>();

function getRuntime(set: (state: Partial<CallingState>) => void): AudioCallRuntime {
  if (!runtime) {
    const settings = useSettingsStore.getState();
    runtime = new AudioCallRuntime({
      iceServers: settings.iceServers,
      onStateChange: (runtimeSnapshot) => set({ runtimeSnapshot }),
    });
  }
  return runtime;
}

function getGroupRuntime(set: (state: Partial<CallingState>) => void): GroupMeshCallRuntime {
  if (!groupRuntime) {
    const settings = useSettingsStore.getState();
    groupRuntime = new GroupMeshCallRuntime({
      iceServers: settings.iceServers,
      onStateChange: (groupRuntimeSnapshot) => set({ groupRuntimeSnapshot }),
    });
  }
  return groupRuntime;
}

function disposeRuntime() {
  runtime?.dispose();
  groupRuntime?.dispose();
  runtime = null;
  groupRuntime = null;
  pendingIncomingEnvelope = null;
  activeGroupCreatorPeerId = null;
  activeGroupRosterVersion = 0;
  pendingGroupInvite = null;
  pendingGroupOffers.clear();
}

export const useCallingStore = create<CallingState>((set, get) => ({
  ...initialState,

  hydrateCalls: async () => {
    set({ isLoading: true, error: null });
    try {
      const [activeCalls, callHistory, groupRooms] = await Promise.all([
        callingService.getActiveCalls(),
        callingService.getCallHistory(),
        callingService.getActiveGroupCalls(),
      ]);
      set({ activeCalls, callHistory, isLoading: false });
      const identityState = useIdentityStore.getState().state;
      const room = groupRooms[0];
      if (room && identityState.status === 'unlocked' && !groupRuntime) {
        pendingGroupInvite = {
          roomId: room.roomId,
          creatorPeerId: room.creatorPeerId,
          senderPeerId: room.creatorPeerId,
          action: 'invite',
          topology: room.topology,
          rosterVersion: room.rosterVersion,
          participants: room.participants,
          mediaMode: room.mediaMode,
          nonce: 'persisted-room',
          timestamp: room.updatedAt,
          signature: [],
        };
        activeGroupCreatorPeerId = room.creatorPeerId;
        activeGroupRosterVersion = room.rosterVersion;
        getGroupRuntime(set).prepareIncomingGroupCall(
          pendingGroupInvite,
          identityState.identity.peerId,
        );
      }
    } catch (error) {
      set({ error: toErrorMessage(error), isLoading: false });
    }
  },

  refreshActiveCalls: async () => {
    try {
      const activeCalls = await callingService.getActiveCalls();
      set({ activeCalls, error: null });
    } catch (error) {
      set({ error: toErrorMessage(error) });
    }
  },

  refreshCallHistory: async (limit = 100) => {
    try {
      const callHistory = await callingService.getCallHistory(limit);
      set({ callHistory, error: null });
    } catch (error) {
      set({ error: toErrorMessage(error) });
    }
  },

  handleBackendEvent: async (event: NetworkEvent) => {
    if (event.type !== 'call_signaling_received') {
      return;
    }

    set({ lastEventPeerId: event.peer_id });
    if (event.message.payload.type === 'group_membership') {
      const membership = event.message.payload.payload;
      activeGroupCreatorPeerId = membership.creatorPeerId;
      activeGroupRosterVersion = membership.rosterVersion;
      if (membership.action === 'terminate') {
        groupRuntime?.dispose('ended');
        groupRuntime = null;
        set({ groupRuntimeSnapshot: idleGroupRuntimeSnapshot });
      } else if (membership.action === 'invite') {
        const identityState = useIdentityStore.getState().state;
        if (identityState.status !== 'unlocked') return;
        pendingGroupInvite = membership;
        getGroupRuntime(set).prepareIncomingGroupCall(
          membership,
          identityState.identity.peerId,
        );
      } else if (membership.action === 'leave') {
        pendingGroupOffers.delete(membership.senderPeerId);
        groupRuntime?.handleParticipantLeft(membership.senderPeerId);
      }
      return;
    }
    if (
      event.message.payload.type === 'offer' &&
      pendingGroupInvite?.participants.includes(event.peer_id)
    ) {
      if (groupRuntime && groupRuntime.getSnapshot().state !== 'ringing') {
        await groupRuntime.acceptParticipantOffer(event.message);
        await get().hydrateCalls();
      } else {
        pendingGroupOffers.set(event.peer_id, event.message);
      }
      return;
    }
    const callRuntime = getRuntime(set);
    const snapshot = callRuntime.getSnapshot();

    if (event.message.payload.type === 'offer') {
      if (!['idle', 'ended', 'failed'].includes(snapshot.state)) {
        await callingService.busyCall(
          event.message.payload.payload.callId,
          event.message.payload.payload.callerPeerId,
        );
        await get().hydrateCalls();
        return;
      }
      pendingIncomingEnvelope = event.message;
    }

    await callRuntime.handleSignalingEvent(event);
    await groupRuntime?.handleSignalingEnvelope(event.message);
    await get().hydrateCalls();
  },

  handlePeerDisconnected: (peerId: string) => {
    runtime?.handlePeerDisconnected(peerId);
    groupRuntime?.handlePeerDisconnected(peerId);
  },

  startOutgoingCall: async (peerId: string, options = {}) => {
    try {
      pendingIncomingEnvelope = null;
      await getRuntime(set).startOutgoingCall(peerId, options);
      await get().hydrateCalls();
    } catch (error) {
      set({ error: toErrorMessage(error) });
      throw error;
    }
  },

  startOutgoingGroupCall: async (peerIds: string[], options = {}) => {
    try {
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
      activeGroupCreatorPeerId = membership.creatorPeerId;
      activeGroupRosterVersion = membership.rosterVersion;
      await getGroupRuntime(set).startOutgoingGroupCall(peerIds, {
        ...options,
        roomId: membership.roomId,
        localPeerId,
      });
      await get().hydrateCalls();
    } catch (error) {
      set({ error: toErrorMessage(error) });
      throw error;
    }
  },

  acceptIncomingCall: async () => {
    if (!pendingIncomingEnvelope) {
      set({ error: 'No incoming call is available to answer.' });
      return;
    }

    try {
      await getRuntime(set).acceptIncomingCall(pendingIncomingEnvelope);
      pendingIncomingEnvelope = null;
      await get().hydrateCalls();
    } catch (error) {
      set({ error: toErrorMessage(error) });
      throw error;
    }
  },

  acceptIncomingGroupCall: async () => {
    const membership = pendingGroupInvite;
    if (!membership) {
      set({ error: 'No incoming group call is available to answer.' });
      return;
    }
    try {
      await callingService.sendGroupMembership({
        roomId: membership.roomId,
        creatorPeerId: membership.creatorPeerId,
        action: 'join',
        rosterVersion: membership.rosterVersion,
        participants: membership.participants,
        mediaMode: membership.mediaMode,
      });
      activeGroupRosterVersion = membership.rosterVersion;
      await getGroupRuntime(set).acceptIncomingGroupCall(
        membership,
        [...pendingGroupOffers.values()],
      );
      pendingGroupOffers.clear();
      await get().hydrateCalls();
    } catch (error) {
      set({ error: toErrorMessage(error) });
      throw error;
    }
  },

  declineIncomingCall: async () => {
    const envelope = pendingIncomingEnvelope;
    if (envelope?.payload.type === 'offer') {
      const offer = envelope.payload.payload;
      try {
        await callingService.declineCall(offer.callId, offer.callerPeerId);
      } catch (error) {
        set({ error: toErrorMessage(error) });
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
    pendingGroupInvite = null;
    pendingGroupOffers.clear();
    groupRuntime?.dispose();
    groupRuntime = null;
    set({ groupRuntimeSnapshot: idleGroupRuntimeSnapshot });
  },

  hangupActiveCall: async (reason = 'normal') => {
    try {
      await runtime?.hangup(reason);
      pendingIncomingEnvelope = null;
      await get().hydrateCalls();
    } catch (error) {
      set({ error: toErrorMessage(error) });
      throw error;
    }
  },

  leaveGroupCall: async (reason = 'normal') => {
    try {
      const snapshot = groupRuntime?.getSnapshot();
      if (snapshot?.roomId && snapshot.localPeerId && activeGroupCreatorPeerId) {
        const participants = [
          snapshot.localPeerId,
          ...snapshot.participants.map((participant) => participant.peerId),
        ].sort();
        await callingService.sendGroupMembership({
          roomId: snapshot.roomId,
          creatorPeerId: activeGroupCreatorPeerId,
          action: snapshot.localPeerId === activeGroupCreatorPeerId ? 'terminate' : 'leave',
          rosterVersion: activeGroupRosterVersion + 1,
          participants,
          mediaMode: snapshot.mediaMode,
        });
        activeGroupRosterVersion += 1;
      }
      await groupRuntime?.leave(reason);
      await get().hydrateCalls();
    } catch (error) {
      set({ error: toErrorMessage(error) });
      throw error;
    }
  },

  setCameraEnabled: async (enabled: boolean) => {
    try {
      await runtime?.setCameraEnabled(enabled);
    } catch (error) {
      set({ error: toErrorMessage(error) });
      throw error;
    }
  },

  setGroupMuted: async (muted: boolean) => {
    try {
      await groupRuntime?.setLocalMuted(muted);
    } catch (error) {
      set({ error: toErrorMessage(error) });
      throw error;
    }
  },

  setGroupCameraEnabled: async (enabled: boolean) => {
    try {
      await groupRuntime?.setCameraEnabled(enabled);
    } catch (error) {
      set({ error: toErrorMessage(error) });
      throw error;
    }
  },

  switchCamera: async (deviceId?: string) => {
    try {
      await runtime?.switchCamera(deviceId);
    } catch (error) {
      set({ error: toErrorMessage(error) });
      throw error;
    }
  },

  dismissCallUi: () => {
    disposeRuntime();
    set({
      runtimeSnapshot: idleRuntimeSnapshot,
      groupRuntimeSnapshot: idleGroupRuntimeSnapshot,
      error: null,
    });
  },

  reset: () => {
    disposeRuntime();
    set(initialState);
  },
}));
