import { create } from 'zustand';
import type { CallSession, HangupReason, NetworkEvent, SignalingEnvelope } from '../types';
import { callingService } from '../services/calling';
import {
  AudioCallRuntime,
  GROUP_CALL_MAX_REMOTE_PARTICIPANTS,
  GroupMeshCallRuntime,
  type AudioCallRuntimeSnapshot,
  type GroupCallRuntimeSnapshot,
} from '../services/callingRuntime';
import { useSettingsStore } from './settings';

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
  declineIncomingCall: () => Promise<void>;
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
}

export const useCallingStore = create<CallingState>((set, get) => ({
  ...initialState,

  hydrateCalls: async () => {
    set({ isLoading: true, error: null });
    try {
      const [activeCalls, callHistory] = await Promise.all([
        callingService.getActiveCalls(),
        callingService.getCallHistory(),
      ]);
      set({ activeCalls, callHistory, isLoading: false });
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
      await getGroupRuntime(set).startOutgoingGroupCall(peerIds, options);
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
