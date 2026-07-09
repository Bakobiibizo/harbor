import type {
  CallIceStateSnapshot,
  HangupReason,
  IceServerConfig,
  GroupMembershipSignal,
  NetworkEvent,
  SignalingEnvelope,
  SignalingPayload,
} from '../types';
import { callingService } from './calling';
import { createCallPeerConnection, type CallPeerConnectionRuntime } from './callingIce';

export type AudioCallRuntimeState =
  | 'idle'
  | 'requesting_microphone'
  | 'ringing'
  | 'incoming'
  | 'connecting'
  | 'connected'
  | 'ended'
  | 'failed';

export type CallMediaMode = 'audio' | 'video';

export type AudioCallTerminalReason =
  | HangupReason
  | 'permission_denied'
  | 'missing_device'
  | 'peer_disconnected'
  | 'ice_failed'
  | 'timeout'
  | 'remote_hangup';

export interface AudioCallRuntimeSnapshot {
  state: AudioCallRuntimeState;
  callId: string | null;
  peerId: string | null;
  localPeerId: string | null;
  direction: 'outgoing' | 'incoming' | null;
  terminalReason: AudioCallTerminalReason | null;
  error: string | null;
  ice: CallIceStateSnapshot | null;
  mediaMode: CallMediaMode;
  videoRequested: boolean;
  localVideoEnabled: boolean;
  localVideoStream: MediaStream | null;
  remoteVideoStream: MediaStream | null;
  remoteVideoAvailable: boolean;
  cameraError: string | null;
}

export interface StartCallOptions {
  video?: boolean;
  videoDeviceId?: string;
}

export interface AudioCallRuntimeOptions {
  iceServers: IceServerConfig[];
  iceTransportPolicy?: RTCIceTransportPolicy;
  timeoutMs?: number;
  mediaDevices?: Pick<MediaDevices, 'getUserMedia'>;
  peerConnectionFactory?: (configuration: RTCConfiguration) => RTCPeerConnection;
  audioElementFactory?: () => HTMLAudioElement;
  onStateChange?: (snapshot: AudioCallRuntimeSnapshot) => void;
}

export type GroupCallRuntimeState =
  | 'idle'
  | 'starting'
  | 'ringing'
  | 'connecting'
  | 'connected'
  | 'degraded'
  | 'ended'
  | 'failed';

export type GroupCallParticipantState =
  | 'invited'
  | 'ringing'
  | 'connecting'
  | 'connected'
  | 'degraded'
  | 'left'
  | 'failed';

export interface GroupCallParticipantSnapshot {
  peerId: string;
  state: GroupCallParticipantState;
  callId: string | null;
  mediaMode: CallMediaMode;
  muted: boolean;
  cameraEnabled: boolean;
  localVideoStream: MediaStream | null;
  remoteVideoStream: MediaStream | null;
  remoteVideoAvailable: boolean;
  activeSpeaker: boolean;
  error: string | null;
  terminalReason: AudioCallTerminalReason | null;
  ice: CallIceStateSnapshot | null;
}

export interface GroupCallRuntimeSnapshot {
  state: GroupCallRuntimeState;
  roomId: string | null;
  topology: typeof GROUP_CALL_TOPOLOGY;
  maxParticipants: typeof GROUP_CALL_MAX_PARTICIPANTS;
  mediaMode: CallMediaMode;
  localPeerId: string | null;
  localMuted: boolean;
  localCameraEnabled: boolean;
  participantCount: number;
  participants: GroupCallParticipantSnapshot[];
  error: string | null;
}

export interface StartGroupCallOptions extends StartCallOptions {
  roomId?: string;
  localPeerId?: string;
}

export type GroupMeshCallRuntimeOptions = Omit<AudioCallRuntimeOptions, 'onStateChange'> & {
  onStateChange?: (snapshot: GroupCallRuntimeSnapshot) => void;
};

export const GROUP_CALL_TOPOLOGY = 'relay_assisted_mesh_v1';
export const GROUP_CALL_MAX_PARTICIPANTS = 4;
export const GROUP_CALL_MAX_REMOTE_PARTICIPANTS = GROUP_CALL_MAX_PARTICIPANTS - 1;

const DEFAULT_CALL_TIMEOUT_MS = 45_000;

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function microphoneFailureReason(error: unknown): AudioCallTerminalReason {
  if (error instanceof DOMException) {
    if (error.name === 'NotAllowedError' || error.name === 'SecurityError') {
      return 'permission_denied';
    }
    if (error.name === 'NotFoundError' || error.name === 'DevicesNotFoundError') {
      return 'missing_device';
    }
  }
  return 'error';
}

function candidateKey(candidate: RTCIceCandidateInit): string {
  return [candidate.candidate, candidate.sdpMid ?? '', candidate.sdpMLineIndex ?? ''].join('|');
}

function toIceCandidateInit(payload: {
  candidate: string;
  sdpMid?: string | null;
  sdpMlineIndex?: number | null;
}): RTCIceCandidateInit {
  return {
    candidate: payload.candidate,
    sdpMid: payload.sdpMid ?? undefined,
    sdpMLineIndex: payload.sdpMlineIndex ?? undefined,
  };
}

function getMediaDevices(
  configured?: Pick<MediaDevices, 'getUserMedia'>,
): Pick<MediaDevices, 'getUserMedia'> {
  const devices = configured ?? globalThis.navigator?.mediaDevices;
  if (!devices?.getUserMedia) {
    throw new DOMException('No microphone device API is available.', 'NotFoundError');
  }
  return devices;
}

function defaultAudioElementFactory(): HTMLAudioElement {
  const audio = new Audio();
  audio.autoplay = true;
  return audio;
}

function sdpHasVideo(sdp: string): boolean {
  return /^m=video\s/im.test(sdp);
}

function videoConstraints(deviceId?: string): MediaTrackConstraints | boolean {
  return deviceId ? { deviceId: { exact: deviceId } } : true;
}

function getVideoTracks(stream: MediaStream): MediaStreamTrack[] {
  return typeof stream.getVideoTracks === 'function'
    ? stream.getVideoTracks()
    : stream.getTracks().filter((track) => track.kind === 'video');
}

export class AudioCallRuntime {
  private readonly options: Required<
    Pick<AudioCallRuntimeOptions, 'iceServers' | 'timeoutMs' | 'audioElementFactory'>
  > &
    Omit<AudioCallRuntimeOptions, 'iceServers' | 'timeoutMs' | 'audioElementFactory'>;

  private connectionRuntime: CallPeerConnectionRuntime | null = null;
  private localStream: MediaStream | null = null;
  private remoteStream: MediaStream | null = null;
  private localVideoStream: MediaStream | null = null;
  private remoteVideoStream: MediaStream | null = null;
  private remoteAudio: HTMLAudioElement | null = null;
  private pendingRemoteIce: RTCIceCandidateInit[] = [];
  private seenRemoteIce = new Set<string>();
  private timeoutHandle: ReturnType<typeof setTimeout> | null = null;
  private preferredVideoDeviceId: string | undefined;
  private snapshot: AudioCallRuntimeSnapshot = {
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

  constructor(options: AudioCallRuntimeOptions) {
    this.options = {
      ...options,
      timeoutMs: options.timeoutMs ?? DEFAULT_CALL_TIMEOUT_MS,
      audioElementFactory: options.audioElementFactory ?? defaultAudioElementFactory,
    };
  }

  getSnapshot(): AudioCallRuntimeSnapshot {
    return { ...this.snapshot };
  }

  getRemoteAudioElement(): HTMLAudioElement | null {
    return this.remoteAudio;
  }

  async startOutgoingCall(
    calleePeerId: string,
    options: StartCallOptions = {},
  ): Promise<AudioCallRuntimeSnapshot> {
    this.ensureNoActiveCall();
    this.preferredVideoDeviceId = options.videoDeviceId;
    this.update({
      state: 'requesting_microphone',
      peerId: calleePeerId,
      direction: 'outgoing',
      videoRequested: Boolean(options.video),
      mediaMode: options.video ? 'video' : 'audio',
      cameraError: null,
    });

    try {
      const peerConnection = await this.prepareConnection(calleePeerId, Boolean(options.video));
      const offer = await peerConnection.createOffer({
        offerToReceiveAudio: true,
        offerToReceiveVideo: Boolean(options.video),
      });
      await peerConnection.setLocalDescription(offer);
      const localDescription = peerConnection.localDescription ?? offer;
      if (!localDescription.sdp) {
        throw new Error('WebRTC did not produce an SDP offer.');
      }

      const signedOffer = await callingService.startCall(calleePeerId, localDescription.sdp);
      this.update({
        state: 'ringing',
        callId: signedOffer.callId,
        peerId: signedOffer.calleePeerId,
        localPeerId: signedOffer.callerPeerId,
      });
      this.startCallTimeout();
      return this.getSnapshot();
    } catch (error) {
      this.fail(microphoneFailureReason(error), errorMessage(error));
      throw error;
    }
  }

  async acceptIncomingCall(envelope: SignalingEnvelope): Promise<AudioCallRuntimeSnapshot> {
    const offer = this.requireOfferPayload(envelope.payload);
    const remoteWantsVideo = sdpHasVideo(offer.sdp);
    if (this.snapshot.state === 'incoming' && this.snapshot.callId === offer.callId) {
      this.cleanup();
    } else {
      this.ensureNoActiveCall();
    }
    this.update({
      state: 'requesting_microphone',
      callId: offer.callId,
      peerId: offer.callerPeerId,
      localPeerId: offer.calleePeerId,
      direction: 'incoming',
      videoRequested: remoteWantsVideo,
      mediaMode: remoteWantsVideo ? 'video' : 'audio',
      cameraError: null,
    });

    try {
      const peerConnection = await this.prepareConnection(offer.callerPeerId, remoteWantsVideo);
      await peerConnection.setRemoteDescription({ type: 'offer', sdp: offer.sdp });
      await this.flushPendingRemoteIce();
      const answer = await peerConnection.createAnswer();
      await peerConnection.setLocalDescription(answer);
      const localDescription = peerConnection.localDescription ?? answer;
      if (!localDescription.sdp) {
        throw new Error('WebRTC did not produce an SDP answer.');
      }

      const signedAnswer = await callingService.answerCall(
        offer.callId,
        offer.callerPeerId,
        localDescription.sdp,
      );
      this.update({
        state: 'connecting',
        callId: signedAnswer.callId,
        peerId: signedAnswer.callerPeerId,
        localPeerId: signedAnswer.calleePeerId,
      });
      this.startCallTimeout();
      return this.getSnapshot();
    } catch (error) {
      this.fail(microphoneFailureReason(error), errorMessage(error));
      throw error;
    }
  }

  async handleSignalingEvent(event: NetworkEvent): Promise<void> {
    if (event.type !== 'call_signaling_received') return;
    await this.handleSignalingEnvelope(event.message);
  }

  async handleSignalingEnvelope(envelope: SignalingEnvelope): Promise<void> {
    switch (envelope.payload.type) {
      case 'offer':
        this.update({
          state: 'incoming',
          callId: envelope.payload.payload.callId,
          peerId: envelope.payload.payload.callerPeerId,
          localPeerId: envelope.payload.payload.calleePeerId,
          direction: 'incoming',
          videoRequested: sdpHasVideo(envelope.payload.payload.sdp),
          mediaMode: sdpHasVideo(envelope.payload.payload.sdp) ? 'video' : 'audio',
          cameraError: null,
        });
        return;
      case 'answer':
        await this.applyAnswer(envelope.payload.payload.sdp, envelope.payload.payload.callId);
        return;
      case 'ice':
        await this.addRemoteIce(toIceCandidateInit(envelope.payload.payload));
        return;
      case 'hangup':
      case 'decline':
      case 'busy':
        this.finish('remote_hangup');
        return;
    }
  }

  setMicrophoneMuted(muted: boolean): void {
    if (!this.localStream) return;
    const audioTracks =
      typeof this.localStream.getAudioTracks === 'function'
        ? this.localStream.getAudioTracks()
        : this.localStream.getTracks().filter((track) => track.kind === 'audio');
    for (const track of audioTracks) {
      track.enabled = !muted;
    }
  }

  async setCameraEnabled(enabled: boolean): Promise<void> {
    if (!this.localStream) return;
    const videoTracks = getVideoTracks(this.localStream);
    if (enabled && videoTracks.length === 0 && this.snapshot.videoRequested) {
      await this.switchCamera(this.preferredVideoDeviceId);
      return;
    }
    for (const track of videoTracks) {
      track.enabled = enabled;
    }
    this.update({ localVideoEnabled: enabled && videoTracks.length > 0 });
  }

  async switchCamera(deviceId?: string): Promise<void> {
    if (!this.connectionRuntime || !this.localStream) return;
    const devices = getMediaDevices(this.options.mediaDevices);
    const cameraStream = await devices.getUserMedia({
      audio: false,
      video: videoConstraints(deviceId),
    });
    const [newTrack] = getVideoTracks(cameraStream);
    if (!newTrack) {
      cameraStream.getTracks().forEach((track) => track.stop());
      throw new DOMException('No camera track is available.', 'NotFoundError');
    }

    const oldTracks = getVideoTracks(this.localStream);
    const sender = this.connectionRuntime.peerConnection
      .getSenders?.()
      .find((candidate) => candidate.track?.kind === 'video');
    if (sender) {
      await sender.replaceTrack(newTrack);
    } else {
      this.connectionRuntime.peerConnection.addTrack(newTrack, this.localStream);
    }
    oldTracks.forEach((track) => {
      this.localStream?.removeTrack?.(track);
      track.stop();
    });
    this.localStream.addTrack(newTrack);
    this.localVideoStream = new MediaStream([newTrack]);
    this.preferredVideoDeviceId = deviceId;
    this.update({
      mediaMode: 'video',
      localVideoEnabled: newTrack.enabled,
      localVideoStream: this.localVideoStream,
      cameraError: null,
    });
  }

  async hangup(reason: HangupReason = 'normal'): Promise<void> {
    const { callId, peerId } = this.snapshot;
    if (callId && peerId && this.snapshot.state !== 'ended' && this.snapshot.state !== 'failed') {
      await callingService.hangupCall(callId, peerId, reason);
    }
    this.finish(reason);
  }

  handlePeerDisconnected(peerId: string): void {
    if (this.snapshot.peerId === peerId && this.snapshot.state !== 'ended') {
      this.finish('peer_disconnected');
    }
  }

  dispose(): void {
    this.cleanup();
    this.update({ state: 'idle', terminalReason: null, error: null });
  }

  private async prepareConnection(
    peerId: string,
    requestVideo: boolean,
  ): Promise<RTCPeerConnection> {
    const stream = await this.captureInitialMedia(requestVideo);
    this.localStream = stream;
    const videoTracks = getVideoTracks(stream);
    this.localVideoStream = videoTracks.length > 0 ? new MediaStream(videoTracks) : null;

    this.remoteStream = new MediaStream();
    this.remoteVideoStream = new MediaStream();
    this.remoteAudio = this.options.audioElementFactory();
    this.remoteAudio.srcObject = this.remoteStream;

    this.connectionRuntime = createCallPeerConnection({
      iceServers: this.options.iceServers,
      iceTransportPolicy: this.options.iceTransportPolicy,
      peerConnectionFactory: this.options.peerConnectionFactory,
      onStateChange: (ice) => {
        this.update({ ice });
        if (ice.connectionState === 'connected' || ice.iceConnectionState === 'connected') {
          this.clearCallTimeout();
          this.update({ state: 'connected' });
        }
        if (
          ice.error &&
          (ice.connectionState === 'failed' || ice.iceConnectionState === 'failed')
        ) {
          this.finish('ice_failed', ice.error.message);
        }
      },
      onIceCandidate: (candidate) => {
        if (!this.snapshot.callId || !this.snapshot.peerId) return;
        callingService
          .sendIceCandidate(
            this.snapshot.callId,
            this.snapshot.peerId,
            candidate.candidate,
            candidate.sdpMid ?? undefined,
            candidate.sdpMLineIndex ?? undefined,
          )
          .catch((error) => this.fail('error', errorMessage(error)));
      },
    });

    const peerConnection = this.connectionRuntime.peerConnection;
    for (const track of stream.getTracks()) {
      peerConnection.addTrack(track, stream);
    }
    peerConnection.addEventListener('track', (event) => {
      const [remote] = event.streams;
      const track = event.track;
      if (track.kind === 'audio') {
        if (remote) {
          this.remoteAudio!.srcObject = remote;
          this.remoteStream = remote;
        } else {
          this.remoteStream?.addTrack(track);
        }
        void this.remoteAudio?.play?.();
      }
      if (track.kind === 'video') {
        if (remote) {
          this.remoteVideoStream = remote;
        } else {
          this.remoteVideoStream?.addTrack(track);
        }
        this.update({
          remoteVideoStream: this.remoteVideoStream,
          remoteVideoAvailable: true,
          mediaMode: 'video',
        });
      }
    });

    this.update({
      state: 'connecting',
      peerId,
      localVideoEnabled: videoTracks.some((track) => track.enabled),
      localVideoStream: this.localVideoStream,
      remoteVideoStream: this.remoteVideoStream,
    });
    return peerConnection;
  }

  private async captureInitialMedia(requestVideo: boolean): Promise<MediaStream> {
    const devices = getMediaDevices(this.options.mediaDevices);
    if (!requestVideo) {
      return devices.getUserMedia({ audio: true, video: false });
    }

    try {
      return await devices.getUserMedia({
        audio: true,
        video: videoConstraints(this.preferredVideoDeviceId),
      });
    } catch (cameraError) {
      const audioOnly = await devices.getUserMedia({ audio: true, video: false });
      this.update({
        mediaMode: 'audio',
        localVideoEnabled: false,
        localVideoStream: null,
        cameraError: errorMessage(cameraError),
      });
      return audioOnly;
    }
  }

  private async applyAnswer(sdp: string, callId: string): Promise<void> {
    if (this.snapshot.callId !== callId || !this.connectionRuntime) return;
    await this.connectionRuntime.peerConnection.setRemoteDescription({ type: 'answer', sdp });
    await this.flushPendingRemoteIce();
    this.update({
      state: 'connecting',
      remoteVideoAvailable: this.snapshot.remoteVideoAvailable || sdpHasVideo(sdp),
    });
  }

  private async addRemoteIce(candidate: RTCIceCandidateInit): Promise<void> {
    const key = candidateKey(candidate);
    if (this.seenRemoteIce.has(key)) return;
    this.seenRemoteIce.add(key);

    const peerConnection = this.connectionRuntime?.peerConnection;
    if (!peerConnection?.remoteDescription) {
      this.pendingRemoteIce.push(candidate);
      return;
    }
    await peerConnection.addIceCandidate(candidate);
  }

  private async flushPendingRemoteIce(): Promise<void> {
    const pending = this.pendingRemoteIce.splice(0);
    for (const candidate of pending) {
      await this.connectionRuntime?.peerConnection.addIceCandidate(candidate);
    }
  }

  private startCallTimeout(): void {
    this.clearCallTimeout();
    this.timeoutHandle = setTimeout(() => {
      this.finish('timeout', 'Call timed out before media connected.');
    }, this.options.timeoutMs);
  }

  private clearCallTimeout(): void {
    if (this.timeoutHandle) {
      clearTimeout(this.timeoutHandle);
      this.timeoutHandle = null;
    }
  }

  private ensureNoActiveCall(): void {
    if (!['idle', 'ended', 'failed'].includes(this.snapshot.state)) {
      throw new Error('A call is already active.');
    }
    this.cleanup();
  }

  private requireOfferPayload(
    payload: SignalingPayload,
  ): Extract<SignalingPayload, { type: 'offer' }>['payload'] {
    if (payload.type !== 'offer') {
      throw new Error('Expected offer signaling payload.');
    }
    return payload.payload;
  }

  private fail(reason: AudioCallTerminalReason, message: string): void {
    this.finish(reason, message, 'failed');
  }

  private finish(
    reason: AudioCallTerminalReason,
    message: string | null = null,
    state: AudioCallRuntimeState = 'ended',
  ): void {
    this.cleanup();
    this.update({ state, terminalReason: reason, error: message });
  }

  private cleanup(): void {
    this.clearCallTimeout();
    this.localStream?.getTracks().forEach((track) => track.stop());
    this.localStream = null;
    this.localVideoStream = null;
    this.remoteStream?.getTracks().forEach((track) => track.stop());
    this.remoteStream = null;
    this.remoteVideoStream?.getTracks().forEach((track) => track.stop());
    this.remoteVideoStream = null;
    if (this.remoteAudio) {
      this.remoteAudio.pause?.();
      this.remoteAudio.srcObject = null;
    }
    this.remoteAudio = null;
    this.connectionRuntime?.close();
    this.connectionRuntime = null;
    this.pendingRemoteIce = [];
    this.seenRemoteIce.clear();
    this.update({
      localVideoEnabled: false,
      localVideoStream: null,
      remoteVideoStream: null,
      remoteVideoAvailable: false,
    });
  }

  private update(update: Partial<AudioCallRuntimeSnapshot>): void {
    this.snapshot = { ...this.snapshot, ...update };
    this.options.onStateChange?.(this.getSnapshot());
  }
}

function groupParticipantState(snapshot: AudioCallRuntimeSnapshot): GroupCallParticipantState {
  switch (snapshot.state) {
    case 'ringing':
      return 'ringing';
    case 'requesting_microphone':
    case 'incoming':
    case 'connecting':
      return 'connecting';
    case 'connected':
      return snapshot.ice?.error ? 'degraded' : 'connected';
    case 'ended':
      return 'left';
    case 'failed':
      return 'failed';
    default:
      return 'invited';
  }
}

function groupOverallState(
  participants: GroupCallParticipantSnapshot[],
  current: GroupCallRuntimeState,
): GroupCallRuntimeState {
  if (participants.length === 0) return current === 'failed' ? 'failed' : 'idle';
  const active = participants.filter((participant) =>
    ['ringing', 'connecting', 'connected', 'degraded'].includes(participant.state),
  );
  if (active.length === 0) {
    if (current === 'starting' || current === 'ringing' || current === 'connecting') return current;
    return participants.every((participant) => participant.state === 'failed') ? 'failed' : 'ended';
  }
  if (
    participants.some(
      (participant) => participant.state === 'failed' || participant.state === 'degraded',
    )
  ) {
    return 'degraded';
  }
  if (participants.some((participant) => participant.state === 'connected')) return 'connected';
  if (participants.some((participant) => participant.state === 'connecting')) return 'connecting';
  return 'ringing';
}

/**
 * Small-group relay-assisted full-mesh runtime from ADR-0001.
 *
 * Each remote participant gets an independent WebRTC peer connection and signed
 * Harbor signaling path. One participant failing is represented on that tile and
 * does not tear down the remaining mesh legs.
 */
export class GroupMeshCallRuntime {
  private readonly childOptions: Omit<AudioCallRuntimeOptions, 'onStateChange'>;
  private readonly onStateChange?: (snapshot: GroupCallRuntimeSnapshot) => void;
  private runtimes = new Map<string, AudioCallRuntime>();
  private participantSnapshots = new Map<string, AudioCallRuntimeSnapshot>();
  private failedParticipants = new Map<string, string>();
  private snapshot: GroupCallRuntimeSnapshot = {
    state: 'idle',
    roomId: null,
    topology: GROUP_CALL_TOPOLOGY,
    maxParticipants: GROUP_CALL_MAX_PARTICIPANTS,
    mediaMode: 'audio',
    localPeerId: null,
    localMuted: false,
    localCameraEnabled: false,
    participantCount: 1,
    participants: [],
    error: null,
  };

  constructor(options: GroupMeshCallRuntimeOptions) {
    const { onStateChange, ...childOptions } = options;
    this.childOptions = childOptions;
    this.onStateChange = onStateChange;
  }

  getSnapshot(): GroupCallRuntimeSnapshot {
    return {
      ...this.snapshot,
      participants: this.snapshot.participants.map((participant) => ({ ...participant })),
    };
  }

  prepareIncomingGroupCall(
    membership: GroupMembershipSignal,
    localPeerId: string,
  ): GroupCallRuntimeSnapshot {
    this.ensureIdle();
    const peers = membership.participants.filter((peerId) => peerId !== localPeerId);
    this.update({
      state: 'ringing',
      roomId: membership.roomId,
      mediaMode: membership.mediaMode,
      localPeerId,
      participantCount: membership.participants.length,
      participants: peers.map((peerId) =>
        this.placeholderParticipant(peerId, membership.mediaMode),
      ),
      error: null,
    });
    return this.getSnapshot();
  }

  async acceptIncomingGroupCall(
    membership: GroupMembershipSignal,
    pendingOffers: SignalingEnvelope[],
    options: StartCallOptions = {},
  ): Promise<GroupCallRuntimeSnapshot> {
    const localPeerId = this.snapshot.localPeerId;
    if (!localPeerId || this.snapshot.roomId !== membership.roomId) {
      throw new Error('The pending group invitation is no longer active.');
    }
    this.update({ state: 'connecting' });
    const offeredPeers = new Set<string>();
    for (const envelope of pendingOffers) {
      if (envelope.payload.type !== 'offer') continue;
      const peerId = envelope.senderPeerId;
      offeredPeers.add(peerId);
      const participantRuntime = this.createParticipantRuntime(peerId);
      this.runtimes.set(peerId, participantRuntime);
      await participantRuntime.acceptIncomingCall(envelope);
      this.participantSnapshots.set(peerId, participantRuntime.getSnapshot());
    }

    const outgoingPeers = membership.participants.filter(
      (peerId) =>
        peerId !== localPeerId &&
        peerId !== membership.creatorPeerId &&
        localPeerId.localeCompare(peerId) < 0 &&
        !offeredPeers.has(peerId),
    );
    await Promise.all(
      outgoingPeers.map(async (peerId) => {
        const participantRuntime = this.createParticipantRuntime(peerId);
        this.runtimes.set(peerId, participantRuntime);
        const participantSnapshot = await participantRuntime.startOutgoingCall(peerId, {
          ...options,
          video: membership.mediaMode === 'video',
        });
        this.participantSnapshots.set(peerId, participantSnapshot);
      }),
    );
    this.recompute();
    return this.getSnapshot();
  }

  async acceptParticipantOffer(envelope: SignalingEnvelope): Promise<void> {
    if (envelope.payload.type !== 'offer') return;
    const peerId = envelope.senderPeerId;
    if (!this.snapshot.participants.some((participant) => participant.peerId === peerId)) return;
    if (this.runtimes.has(peerId)) {
      await this.runtimes.get(peerId)?.handleSignalingEnvelope(envelope);
      return;
    }
    const participantRuntime = this.createParticipantRuntime(peerId);
    this.runtimes.set(peerId, participantRuntime);
    await participantRuntime.acceptIncomingCall(envelope);
    this.participantSnapshots.set(peerId, participantRuntime.getSnapshot());
    this.recompute();
  }

  async startOutgoingGroupCall(
    peerIds: string[],
    options: StartGroupCallOptions = {},
  ): Promise<GroupCallRuntimeSnapshot> {
    this.ensureIdle();
    const uniquePeerIds = [...new Set(peerIds.filter(Boolean))];
    if (uniquePeerIds.length === 0) {
      throw new Error('Select at least one participant for a group call.');
    }
    if (uniquePeerIds.length > GROUP_CALL_MAX_REMOTE_PARTICIPANTS) {
      throw new Error(
        `Group calls use ${GROUP_CALL_TOPOLOGY} and support at most ${GROUP_CALL_MAX_PARTICIPANTS} total participants (${GROUP_CALL_MAX_REMOTE_PARTICIPANTS} remote peers).`,
      );
    }

    this.update({
      state: 'starting',
      roomId:
        options.roomId ?? `mesh-${globalThis.crypto?.randomUUID?.() ?? Date.now().toString(36)}`,
      mediaMode: options.video ? 'video' : 'audio',
      localPeerId: options.localPeerId ?? null,
      participantCount: uniquePeerIds.length + 1,
      participants: uniquePeerIds.map((peerId) =>
        this.placeholderParticipant(peerId, options.video ? 'video' : 'audio'),
      ),
      error: null,
    });

    await Promise.all(
      uniquePeerIds.map(async (peerId) => {
        const runtime = this.createParticipantRuntime(peerId);
        this.runtimes.set(peerId, runtime);
        try {
          const snapshot = await runtime.startOutgoingCall(peerId, options);
          this.participantSnapshots.set(peerId, snapshot);
          this.recompute();
        } catch (error) {
          this.failedParticipants.set(peerId, errorMessage(error));
          this.recompute();
        }
      }),
    );

    const snapshot = this.getSnapshot();
    if (snapshot.participants.every((participant) => participant.state === 'failed')) {
      const error = 'Could not establish media with any group-call participant.';
      this.update({ state: 'failed', error });
      throw new Error(error);
    }
    return this.getSnapshot();
  }

  async handleSignalingEnvelope(envelope: SignalingEnvelope): Promise<void> {
    const peerId = envelope.senderPeerId;
    const runtime = this.runtimes.get(peerId);
    if (!runtime) return;
    try {
      await runtime.handleSignalingEnvelope(envelope);
      this.participantSnapshots.set(peerId, runtime.getSnapshot());
      this.recompute();
    } catch (error) {
      this.failedParticipants.set(peerId, errorMessage(error));
      this.recompute();
    }
  }

  handlePeerDisconnected(peerId: string): void {
    const runtime = this.runtimes.get(peerId);
    runtime?.handlePeerDisconnected(peerId);
    this.participantSnapshots.set(
      peerId,
      runtime?.getSnapshot() ?? this.placeholderAudioSnapshot(peerId),
    );
    this.failedParticipants.set(peerId, 'Peer disconnected.');
    this.recompute();
  }

  async setLocalMuted(muted: boolean): Promise<void> {
    for (const runtime of this.runtimes.values()) {
      runtime.setMicrophoneMuted(muted);
    }
    this.update({ localMuted: muted });
  }

  async setCameraEnabled(enabled: boolean): Promise<void> {
    await Promise.all(
      [...this.runtimes.values()].map((runtime) => runtime.setCameraEnabled(enabled)),
    );
    this.update({ localCameraEnabled: enabled });
    this.recompute();
  }

  async leave(reason: HangupReason = 'normal'): Promise<void> {
    await Promise.allSettled([...this.runtimes.values()].map((runtime) => runtime.hangup(reason)));
    this.dispose('ended');
  }

  dispose(state: GroupCallRuntimeState = 'idle'): void {
    for (const runtime of this.runtimes.values()) runtime.dispose();
    this.runtimes.clear();
    this.participantSnapshots.clear();
    this.failedParticipants.clear();
    this.update({
      state,
      roomId: state === 'idle' ? null : this.snapshot.roomId,
      participantCount: state === 'idle' ? 1 : this.snapshot.participantCount,
      participants: [],
      localMuted: false,
      localCameraEnabled: false,
      error: null,
    });
  }

  private createParticipantRuntime(peerId: string): AudioCallRuntime {
    return new AudioCallRuntime({
      ...this.childOptions,
      onStateChange: (snapshot) => {
        this.participantSnapshots.set(peerId, snapshot);
        this.recompute();
      },
    });
  }

  private placeholderParticipant(
    peerId: string,
    mediaMode: CallMediaMode,
  ): GroupCallParticipantSnapshot {
    return {
      peerId,
      state: 'invited',
      callId: null,
      mediaMode,
      muted: false,
      cameraEnabled: false,
      localVideoStream: null,
      remoteVideoStream: null,
      remoteVideoAvailable: false,
      activeSpeaker: false,
      error: null,
      terminalReason: null,
      ice: null,
    };
  }

  private placeholderAudioSnapshot(peerId: string): AudioCallRuntimeSnapshot {
    return {
      state: 'failed',
      callId: null,
      peerId,
      localPeerId: this.snapshot.localPeerId,
      direction: 'outgoing',
      terminalReason: 'peer_disconnected',
      error: 'Peer disconnected.',
      ice: null,
      mediaMode: this.snapshot.mediaMode,
      videoRequested: this.snapshot.mediaMode === 'video',
      localVideoEnabled: false,
      localVideoStream: null,
      remoteVideoStream: null,
      remoteVideoAvailable: false,
      cameraError: null,
    };
  }

  private recompute(): void {
    const peers = new Set([
      ...this.snapshot.participants.map((participant) => participant.peerId),
      ...this.participantSnapshots.keys(),
      ...this.failedParticipants.keys(),
    ]);
    const participants = [...peers].map((peerId) => {
      const snapshot = this.participantSnapshots.get(peerId);
      const failure = this.failedParticipants.get(peerId);
      if (!snapshot) {
        return {
          ...this.placeholderParticipant(peerId, this.snapshot.mediaMode),
          error: failure ?? null,
          state: failure ? ('failed' as const) : ('invited' as const),
        };
      }
      return {
        peerId,
        state: failure ? 'failed' : groupParticipantState(snapshot),
        callId: snapshot.callId,
        mediaMode: snapshot.mediaMode,
        muted: false,
        cameraEnabled: snapshot.localVideoEnabled,
        localVideoStream: snapshot.localVideoStream,
        remoteVideoStream: snapshot.remoteVideoStream,
        remoteVideoAvailable: snapshot.remoteVideoAvailable,
        activeSpeaker: snapshot.state === 'connected',
        error: failure ?? snapshot.error ?? snapshot.cameraError,
        terminalReason: snapshot.terminalReason,
        ice: snapshot.ice,
      } satisfies GroupCallParticipantSnapshot;
    });
    const activeCamera = participants.some((participant) => participant.cameraEnabled);
    this.update({
      state: groupOverallState(participants, this.snapshot.state),
      participants,
      localCameraEnabled: activeCamera,
      participantCount: participants.length + 1,
    });
  }

  private ensureIdle(): void {
    if (!['idle', 'ended', 'failed'].includes(this.snapshot.state)) {
      throw new Error('A group call is already active.');
    }
    this.dispose('idle');
  }

  private update(update: Partial<GroupCallRuntimeSnapshot>): void {
    this.snapshot = { ...this.snapshot, ...update };
    this.onStateChange?.(this.getSnapshot());
  }
}
