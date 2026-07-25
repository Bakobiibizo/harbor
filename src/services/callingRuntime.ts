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
import { callFailureFrom } from '../utils/callErrors';

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
  | 'missing_media_api'
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
  /** Browser autoplay policy blocked remote audio until a user gesture retries it. */
  remoteAudioBlocked: boolean;
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
  /** Group meshes own and stop their shared local tracks at room teardown. */
  stopLocalTracksOnCleanup?: boolean;
  onStateChange?: (snapshot: AudioCallRuntimeSnapshot) => void;
}

export type GroupCallRuntimeState =
  'idle' | 'starting' | 'ringing' | 'connecting' | 'connected' | 'degraded' | 'ended' | 'failed';

export type GroupCallParticipantState =
  | 'invited'
  | 'ringing'
  | 'connecting'
  | 'connected'
  | 'degraded'
  | 'left'
  | 'declined'
  | 'timed_out'
  | 'disconnected'
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
  remoteAudioBlocked: boolean;
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
  return callFailureFrom(error, 'call-media-runtime').message;
}

function microphoneFailureReason(error: unknown): AudioCallTerminalReason {
  if (error instanceof DOMException) {
    if (error.name === 'NotAllowedError' || error.name === 'SecurityError') {
      return 'permission_denied';
    }
    if (error.name === 'NotSupportedError') {
      return 'missing_media_api';
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
  if (
    !configured &&
    (globalThis as typeof globalThis & { __HARBOR_HEADLESS_MEDIA_CAPTURE__?: boolean })
      .__HARBOR_HEADLESS_MEDIA_CAPTURE__
  ) {
    return { getUserMedia: createHeadlessMediaStream };
  }
  const devices = configured ?? globalThis.navigator?.mediaDevices;
  if (!devices?.getUserMedia) {
    throw new DOMException('The media capture API is not available.', 'NotSupportedError');
  }
  return devices;
}

const syntheticTrackCleanup = new WeakMap<MediaStreamTrack, () => void>();

function stopMediaTrack(track: MediaStreamTrack): void {
  syntheticTrackCleanup.get(track)?.();
  syntheticTrackCleanup.delete(track);
  track.stop();
}

function ownSyntheticTrack(track: MediaStreamTrack, cleanup: () => void): void {
  let cleaned = false;
  const runCleanup = () => {
    if (cleaned) return;
    cleaned = true;
    syntheticTrackCleanup.delete(track);
    cleanup();
  };
  syntheticTrackCleanup.set(track, runCleanup);
  track.addEventListener('ended', runCleanup, { once: true });
}

async function createHeadlessMediaStream(
  constraints: MediaStreamConstraints,
): Promise<MediaStream> {
  const stream = new MediaStream();
  if (constraints.audio) {
    const constructors = globalThis as typeof globalThis & {
      webkitAudioContext?: typeof AudioContext;
    };
    const AudioContextCtor = globalThis.AudioContext ?? constructors.webkitAudioContext;
    if (!AudioContextCtor) {
      throw new DOMException('Synthetic audio is unavailable.', 'NotSupportedError');
    }
    const context = new AudioContextCtor();
    const oscillator = context.createOscillator();
    const destination = context.createMediaStreamDestination();
    oscillator.frequency.value = 440;
    oscillator.connect(destination);
    oscillator.start();
    const [track] = destination.stream.getAudioTracks();
    if (!track) {
      try {
        oscillator.stop();
      } catch {}
      void context.close();
      throw new DOMException('Synthetic audio track creation failed.', 'NotSupportedError');
    }
    ownSyntheticTrack(track, () => {
      try {
        oscillator.stop();
      } catch {}
      void context.close();
    });
    stream.addTrack(track);
  }
  if (constraints.video) {
    const canvas = document.createElement('canvas');
    canvas.width = 320;
    canvas.height = 180;
    const context = canvas.getContext('2d');
    let frame = 0;
    const paint = () => {
      if (!context) return;
      context.fillStyle = frame++ % 2 ? '#0b1f3a' : '#ffffff';
      context.fillRect(0, 0, canvas.width, canvas.height);
    };
    paint();
    const timer = setInterval(paint, 200);
    const [track] = canvas.captureStream(5).getVideoTracks();
    if (!track) {
      clearInterval(timer);
      throw new DOMException('Synthetic video track creation failed.', 'NotSupportedError');
    }
    ownSyntheticTrack(track, () => clearInterval(timer));
    stream.addTrack(track);
  }
  return stream;
}

function defaultAudioElementFactory(): HTMLAudioElement {
  const audio = new Audio();
  audio.autoplay = true;
  return audio;
}

async function waitForInitialIceGathering(
  peerConnection: RTCPeerConnection,
  timeoutMs = 3_000,
): Promise<void> {
  if (peerConnection.iceGatheringState === 'complete') return;
  await new Promise<void>((resolve) => {
    let settled = false;
    const finish = () => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      peerConnection.removeEventListener('icegatheringstatechange', onStateChange);
      resolve();
    };
    const onStateChange = () => {
      if (peerConnection.iceGatheringState === 'complete') finish();
    };
    const timer = setTimeout(finish, timeoutMs);
    peerConnection.addEventListener('icegatheringstatechange', onStateChange);
    onStateChange();
  });
}

function sdpHasVideo(sdp: string): boolean {
  return /^m=video\s/im.test(sdp);
}

/**
 * WebKit/GStreamer can reuse a dynamic RTP payload number for different codecs
 * in separate media sections. Chromium treats payload numbers as globally
 * unique and rejects that otherwise valid SDP. Remap only the conflicting
 * section and keep its m-line, codec attributes, and RTX apt references aligned.
 */
export function normalizeSdpPayloadTypes(sdp: string): string {
  const newline = sdp.includes('\r\n') ? '\r\n' : '\n';
  const trailingNewline = sdp.endsWith(newline);
  let lines = sdp.split(/\r?\n/);
  if (trailingNewline) lines.pop();

  // Some WebKit builds also emit two codec declarations for one payload in a
  // single m-section. A single payload cannot represent both codecs, so retain
  // the primary codec and drop the duplicate RTX declaration. Retransmission
  // is optional; keeping the primary codec preserves the actual media path.
  const removeLines = new Set<number>();
  const droppedRtx = new Map<number, Set<number>>();
  const sectionCodecs = new Map<number, Map<number, { codec: string; line: number }>>();
  let duplicateSection = -1;
  for (const [index, line] of lines.entries()) {
    if (line.startsWith('m=')) duplicateSection += 1;
    const match = line.match(/^a=rtpmap:(\d+)\s+([^/\s]+)\//i);
    if (!match || duplicateSection < 0) continue;
    const payloadType = Number(match[1]);
    const codec = match[2].toLowerCase();
    const codecs = sectionCodecs.get(duplicateSection) ?? new Map();
    const existing = codecs.get(payloadType);
    if (!existing) {
      codecs.set(payloadType, { codec, line: index });
      sectionCodecs.set(duplicateSection, codecs);
      continue;
    }
    if (existing.codec === codec) continue;

    const dropped = droppedRtx.get(duplicateSection) ?? new Set<number>();
    dropped.add(payloadType);
    droppedRtx.set(duplicateSection, dropped);
    if (codec === 'rtx') {
      removeLines.add(index);
    } else if (existing.codec === 'rtx') {
      removeLines.add(existing.line);
      codecs.set(payloadType, { codec, line: index });
    } else {
      // Prefer the first non-RTX declaration when a broken producer assigns
      // one payload to two primary codecs.
      removeLines.add(index);
    }
  }
  duplicateSection = -1;
  lines = lines.filter((line, index) => {
    if (line.startsWith('m=')) duplicateSection += 1;
    if (removeLines.has(index)) return false;
    const fmtp = line.match(/^a=fmtp:(\d+)\s+.*\bapt=\d+\b/i);
    return !(fmtp && droppedRtx.get(duplicateSection)?.has(Number(fmtp[1])));
  });

  const used = new Set<number>();
  for (const line of lines) {
    const media = line.match(/^m=\S+\s+\S+\s+\S+\s+(.+)$/);
    if (media) {
      for (const value of media[1].trim().split(/\s+/)) {
        if (/^\d+$/.test(value)) used.add(Number(value));
      }
    }
    const rtpmap = line.match(/^a=rtpmap:(\d+)\s+/i);
    if (rtpmap) used.add(Number(rtpmap[1]));
  }

  const signatures = new Map<number, string>();
  const remaps = new Map<number, Map<number, number>>();
  let mediaSection = -1;
  for (const line of lines) {
    if (line.startsWith('m=')) mediaSection += 1;
    const match = line.match(/^a=rtpmap:(\d+)\s+(.+)$/i);
    if (!match || mediaSection < 0) continue;
    const payloadType = Number(match[1]);
    const signature = match[2].trim().toLowerCase();
    const existing = signatures.get(payloadType);
    if (!existing || existing === signature) {
      signatures.set(payloadType, signature);
      continue;
    }

    let replacement = 127;
    while (replacement >= 96 && used.has(replacement)) replacement -= 1;
    if (replacement < 96) {
      throw new Error('WebRTC SDP has no free dynamic payload type for codec interoperability.');
    }
    used.add(replacement);
    signatures.set(replacement, signature);
    const sectionRemaps = remaps.get(mediaSection) ?? new Map<number, number>();
    sectionRemaps.set(payloadType, replacement);
    remaps.set(mediaSection, sectionRemaps);
  }

  mediaSection = -1;
  const normalized = lines.map((line) => {
    if (line.startsWith('m=')) mediaSection += 1;
    const sectionRemaps = remaps.get(mediaSection);
    if (!sectionRemaps?.size) return line;

    if (line.startsWith('m=')) {
      const fields = line.split(/\s+/);
      return fields
        .map((field, index) =>
          index >= 3 && /^\d+$/.test(field)
            ? String(sectionRemaps.get(Number(field)) ?? Number(field))
            : field,
        )
        .join(' ');
    }

    const attribute = line.match(/^(a=(?:rtpmap|fmtp|rtcp-fb):)(\d+)(.*)$/i);
    let value = line;
    if (attribute) {
      value = `${attribute[1]}${sectionRemaps.get(Number(attribute[2])) ?? attribute[2]}${attribute[3]}`;
    }
    return value.replace(/\bapt=(\d+)\b/gi, (_match, payload: string) => {
      return `apt=${sectionRemaps.get(Number(payload)) ?? payload}`;
    });
  });

  return `${normalized.join(newline)}${trailingNewline ? newline : ''}`;
}

function videoConstraints(deviceId?: string): MediaTrackConstraints | boolean {
  return deviceId ? { deviceId: { exact: deviceId } } : true;
}

function getVideoTracks(stream: MediaStream): MediaStreamTrack[] {
  return typeof stream.getVideoTracks === 'function'
    ? stream.getVideoTracks()
    : stream.getTracks().filter((track) => track.kind === 'video');
}

function isLiveVideoTrack(track: MediaStreamTrack): boolean {
  return track.kind === 'video' && track.enabled && track.readyState !== 'ended' && !track.muted;
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
  private pendingLocalIce: RTCIceCandidateInit[] = [];
  private pendingRemoteIce: RTCIceCandidateInit[] = [];
  private remoteAnswerApplication: Promise<void> | null = null;
  private seenRemoteIce = new Set<string>();
  private timeoutHandle: ReturnType<typeof setTimeout> | null = null;
  private lifecycleGeneration = 0;
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
    remoteAudioBlocked: false,
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

  /**
   * Retry remote playback from an explicit user gesture after autoplay was
   * denied. The media session stays alive while blocked, so this does not
   * renegotiate or replace the peer connection.
   */
  async enableRemoteAudio(): Promise<boolean> {
    const audio = this.remoteAudio;
    if (!audio?.play || !audio.srcObject) return false;
    try {
      await audio.play();
      this.update({ remoteAudioBlocked: false });
      return true;
    } catch {
      this.update({ remoteAudioBlocked: true });
      return false;
    }
  }

  async startOutgoingCall(
    calleePeerId: string,
    options: StartCallOptions = {},
  ): Promise<AudioCallRuntimeSnapshot> {
    this.ensureNoActiveCall();
    const generation = this.lifecycleGeneration;
    this.preferredVideoDeviceId = options.videoDeviceId;
    this.update({
      state: 'requesting_microphone',
      callId: null,
      peerId: calleePeerId,
      localPeerId: null,
      direction: 'outgoing',
      terminalReason: null,
      error: null,
      ice: null,
      videoRequested: Boolean(options.video),
      mediaMode: options.video ? 'video' : 'audio',
      cameraError: null,
    });

    try {
      const peerConnection = await this.prepareConnection(
        calleePeerId,
        Boolean(options.video),
        generation,
      );
      const offer = await peerConnection.createOffer({
        offerToReceiveAudio: true,
        offerToReceiveVideo: Boolean(options.video),
      });
      const normalizedOffer = {
        ...offer,
        sdp: offer.sdp ? normalizeSdpPayloadTypes(offer.sdp) : offer.sdp,
      };
      await peerConnection.setLocalDescription(normalizedOffer);
      await waitForInitialIceGathering(peerConnection);
      // Preserve any synchronously gathered ICE candidates from the browser's
      // local description, then normalize again because WebKit may regenerate
      // its globally conflicting payload numbers while exposing that value.
      const exposedOffer = peerConnection.localDescription ?? normalizedOffer;
      const localDescription = {
        ...exposedOffer,
        sdp: exposedOffer.sdp ? normalizeSdpPayloadTypes(exposedOffer.sdp) : exposedOffer.sdp,
      };
      if (!localDescription.sdp) {
        throw new Error('WebRTC did not produce an SDP offer.');
      }

      const signedOffer = await callingService.startCall(calleePeerId, localDescription.sdp);
      if (generation !== this.lifecycleGeneration) {
        throw new DOMException('The call ended while signaling was pending.', 'AbortError');
      }
      this.update({
        state: 'ringing',
        callId: signedOffer.callId,
        peerId: signedOffer.calleePeerId,
        localPeerId: signedOffer.callerPeerId,
      });
      await this.flushPendingLocalIce(generation);
      this.startCallTimeout();
      return this.getSnapshot();
    } catch (error) {
      if (generation === this.lifecycleGeneration) {
        const { callId, peerId } = this.snapshot;
        this.fail(microphoneFailureReason(error), errorMessage(error));
        if (callId && peerId) {
          void callingService.hangupCall(callId, peerId, 'error').catch((signalError) => {
            console.warn(
              '[Call] Failed to notify the peer about local media failure:',
              errorMessage(signalError),
            );
          });
        }
      }
      throw error;
    }
  }

  async acceptIncomingCall(envelope: SignalingEnvelope): Promise<AudioCallRuntimeSnapshot> {
    const offer = this.requireOfferPayload(envelope.payload);
    if (
      envelope.senderPeerId !== offer.callerPeerId ||
      envelope.recipientPeerId !== offer.calleePeerId
    ) {
      throw new Error('Incoming call signaling does not match its envelope.');
    }
    const remoteWantsVideo = sdpHasVideo(offer.sdp);
    if (this.snapshot.state === 'incoming' && this.snapshot.callId === offer.callId) {
      this.cleanup();
    } else {
      this.ensureNoActiveCall();
    }
    const generation = this.lifecycleGeneration;
    this.update({
      state: 'requesting_microphone',
      callId: offer.callId,
      peerId: offer.callerPeerId,
      localPeerId: offer.calleePeerId,
      direction: 'incoming',
      terminalReason: null,
      error: null,
      ice: null,
      videoRequested: remoteWantsVideo,
      mediaMode: remoteWantsVideo ? 'video' : 'audio',
      cameraError: null,
    });

    try {
      const peerConnection = await this.prepareConnection(
        offer.callerPeerId,
        remoteWantsVideo,
        generation,
      );
      await peerConnection.setRemoteDescription({ type: 'offer', sdp: offer.sdp });
      await this.flushPendingRemoteIce();
      const answer = await peerConnection.createAnswer();
      const normalizedAnswer = {
        ...answer,
        sdp: answer.sdp ? normalizeSdpPayloadTypes(answer.sdp) : answer.sdp,
      };
      await peerConnection.setLocalDescription(normalizedAnswer);
      await waitForInitialIceGathering(peerConnection);
      const exposedAnswer = peerConnection.localDescription ?? normalizedAnswer;
      const localDescription = {
        ...exposedAnswer,
        sdp: exposedAnswer.sdp ? normalizeSdpPayloadTypes(exposedAnswer.sdp) : exposedAnswer.sdp,
      };
      if (!localDescription.sdp) {
        throw new Error('WebRTC did not produce an SDP answer.');
      }

      const signedAnswer = await callingService.answerCall(
        offer.callId,
        offer.callerPeerId,
        localDescription.sdp,
      );
      if (generation !== this.lifecycleGeneration) {
        throw new DOMException('The call ended while signaling was pending.', 'AbortError');
      }
      this.update({
        state: 'connecting',
        callId: signedAnswer.callId,
        peerId: signedAnswer.callerPeerId,
        localPeerId: signedAnswer.calleePeerId,
      });
      this.startCallTimeout();
      return this.getSnapshot();
    } catch (error) {
      if (generation === this.lifecycleGeneration) {
        const { callId, peerId } = this.snapshot;
        this.fail(microphoneFailureReason(error), errorMessage(error));
        if (callId && peerId) {
          void callingService.hangupCall(callId, peerId, 'error').catch((signalError) => {
            console.warn(
              '[Call] Failed to notify the peer about local media failure:',
              errorMessage(signalError),
            );
          });
        }
      }
      throw error;
    }
  }

  async handleSignalingEvent(event: NetworkEvent): Promise<void> {
    if (event.type !== 'call_signaling_received') return;
    await this.handleSignalingEnvelope(event.message);
  }

  async handleSignalingEnvelope(envelope: SignalingEnvelope): Promise<void> {
    switch (envelope.payload.type) {
      case 'offer': {
        const offer = envelope.payload.payload;
        if (
          envelope.senderPeerId !== offer.callerPeerId ||
          envelope.recipientPeerId !== offer.calleePeerId
        ) {
          return;
        }
        if (
          !['idle', 'ended', 'failed'].includes(this.snapshot.state) &&
          !(
            this.snapshot.state === 'incoming' &&
            this.snapshot.callId === offer.callId &&
            this.snapshot.peerId === offer.callerPeerId
          )
        ) {
          return;
        }
        this.update({
          state: 'incoming',
          callId: offer.callId,
          peerId: offer.callerPeerId,
          localPeerId: offer.calleePeerId,
          direction: 'incoming',
          videoRequested: sdpHasVideo(offer.sdp),
          mediaMode: sdpHasVideo(offer.sdp) ? 'video' : 'audio',
          cameraError: null,
        });
        return;
      }
      case 'answer':
        if (!this.matchesCurrentSignal(envelope)) return;
        await this.applyAnswer(envelope.payload.payload.sdp, envelope.payload.payload.callId);
        return;
      case 'ice':
        if (!this.matchesCurrentSignal(envelope)) return;
        await this.addRemoteIce(toIceCandidateInit(envelope.payload.payload));
        return;
      case 'hangup':
        if (!this.matchesCurrentSignal(envelope)) return;
        if (envelope.payload.payload.reason === 'error') {
          this.fail('error', 'The other participant could not start call media.');
        } else if (envelope.payload.payload.reason === 'timeout') {
          this.finish('timeout', 'The other participant ended the call after it timed out.');
        } else {
          this.finish('remote_hangup');
        }
        return;
      case 'decline':
        if (!this.matchesCurrentSignal(envelope)) return;
        this.finish('declined');
        return;
      case 'busy':
        if (!this.matchesCurrentSignal(envelope)) return;
        this.finish('busy');
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
      try {
        await this.switchCamera(this.preferredVideoDeviceId);
      } catch (error) {
        this.update({ cameraError: errorMessage(error), localVideoEnabled: false });
        throw error;
      }
      return;
    }
    for (const track of videoTracks) {
      track.enabled = enabled;
    }
    this.update({ localVideoEnabled: enabled && videoTracks.some(isLiveVideoTrack) });
  }

  async switchCamera(deviceId?: string): Promise<void> {
    if (!this.connectionRuntime || !this.localStream) return;
    const devices = getMediaDevices(this.options.mediaDevices);
    const generation = this.lifecycleGeneration;
    const cameraStream = await devices.getUserMedia({
      audio: false,
      video: videoConstraints(deviceId),
    });
    if (generation !== this.lifecycleGeneration) {
      cameraStream.getTracks().forEach(stopMediaTrack);
      throw new DOMException('The call ended while camera access was pending.', 'AbortError');
    }
    const [newTrack] = getVideoTracks(cameraStream);
    if (!newTrack) {
      cameraStream.getTracks().forEach(stopMediaTrack);
      throw new DOMException('No camera track is available.', 'NotFoundError');
    }

    const oldTracks = getVideoTracks(this.localStream);
    const peerConnection = this.connectionRuntime.peerConnection;
    const sender =
      peerConnection.getSenders?.().find((candidate) => candidate.track?.kind === 'video') ??
      peerConnection
        .getTransceivers?.()
        .find((candidate) => candidate.receiver.track.kind === 'video')?.sender;
    try {
      if (sender) {
        await sender.replaceTrack(newTrack);
      } else {
        peerConnection.addTrack(newTrack, this.localStream);
      }
    } catch (error) {
      cameraStream.getTracks().forEach(stopMediaTrack);
      throw error;
    }
    oldTracks.forEach((track) => {
      this.localStream?.removeTrack?.(track);
      if (this.options.stopLocalTracksOnCleanup !== false) stopMediaTrack(track);
    });
    this.localStream.addTrack(newTrack);
    this.localVideoStream = new MediaStream([newTrack]);
    this.preferredVideoDeviceId = deviceId;
    this.bindLocalVideoTrack(newTrack, generation);
    this.update({
      mediaMode: 'video',
      localVideoEnabled: isLiveVideoTrack(newTrack),
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
    this.lifecycleGeneration += 1;
    this.cleanup();
    this.update({
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
      cameraError: null,
    });
  }

  private async prepareConnection(
    peerId: string,
    requestVideo: boolean,
    generation: number,
  ): Promise<RTCPeerConnection> {
    if (!this.options.peerConnectionFactory && typeof globalThis.RTCPeerConnection !== 'function') {
      throw new DOMException(
        'This Harbor build does not provide the WebRTC peer connection API required for calls.',
        'NotSupportedError',
      );
    }
    const stream = await this.captureInitialMedia(requestVideo);
    if (generation !== this.lifecycleGeneration) {
      stream.getTracks().forEach(stopMediaTrack);
      throw new DOMException('The call ended while media access was pending.', 'AbortError');
    }
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
        if (generation !== this.lifecycleGeneration) return;
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
        if (generation !== this.lifecycleGeneration) return;
        const value = {
          candidate: candidate.candidate,
          sdpMid: candidate.sdpMid ?? undefined,
          sdpMLineIndex: candidate.sdpMLineIndex ?? undefined,
        };
        if (!this.snapshot.callId || !this.snapshot.peerId) {
          this.pendingLocalIce.push(value);
          return;
        }
        void this.sendLocalIce(value, generation).catch((error) => {
          if (generation === this.lifecycleGeneration) this.fail('error', errorMessage(error));
        });
      },
    });

    const peerConnection = this.connectionRuntime.peerConnection;
    if (requestVideo && videoTracks.length === 0) {
      // Reserve a negotiated video sender while camera capture is unavailable.
      // Replacing this sender's null track later does not require a second SDP
      // exchange, so camera permission or hardware recovery is immediately
      // visible to the remote peer without interrupting the audio session.
      peerConnection.addTransceiver?.('video', { direction: 'sendrecv' });
    }
    for (const track of stream.getTracks()) {
      peerConnection.addTrack(track, stream);
    }
    peerConnection.addEventListener('track', (event) => {
      if (generation !== this.lifecycleGeneration) return;
      const [remote] = event.streams;
      const track = event.track;
      if (track.kind === 'audio') {
        if (remote) {
          this.remoteAudio!.srcObject = remote;
          this.remoteStream = remote;
        } else {
          this.remoteStream?.addTrack(track);
        }
        void this.enableRemoteAudio();
      }
      if (track.kind === 'video') {
        if (remote) {
          this.remoteVideoStream = remote;
          if (!getVideoTracks(remote).includes(track)) {
            remote.addTrack(track);
          }
        } else {
          this.remoteVideoStream?.addTrack(track);
        }
        this.bindRemoteVideoTrack(track, generation);
        this.refreshRemoteVideoState();
      }
    });

    for (const track of videoTracks) {
      this.bindLocalVideoTrack(track, generation);
    }

    this.update({
      state: 'connecting',
      peerId,
      localVideoEnabled: videoTracks.some(isLiveVideoTrack),
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
    const generation = this.lifecycleGeneration;
    const peerConnection = this.connectionRuntime.peerConnection;
    // Rust deliberately repeats the signed answer over a short bounded window.
    // Applying the same answer twice is invalid in some WebRTC engines, so make
    // the frontend transition idempotent just like incoming offers.
    if (peerConnection.remoteDescription?.type === 'answer') return;
    if (this.remoteAnswerApplication) {
      await this.remoteAnswerApplication;
      return;
    }
    const application = peerConnection.setRemoteDescription({ type: 'answer', sdp });
    this.remoteAnswerApplication = application;
    try {
      await application;
    } finally {
      if (this.remoteAnswerApplication === application) this.remoteAnswerApplication = null;
    }
    if (generation !== this.lifecycleGeneration) return;
    await this.flushPendingRemoteIce();
    if (generation !== this.lifecycleGeneration) return;
    this.update({
      state: 'connecting',
    });
  }

  private matchesCurrentSignal(envelope: SignalingEnvelope): boolean {
    if (['idle', 'ended', 'failed'].includes(this.snapshot.state)) return false;
    if (!this.snapshot.callId || !this.snapshot.peerId || !this.snapshot.localPeerId) return false;
    const payload = envelope.payload.payload;
    if (!('callId' in payload) || payload.callId !== this.snapshot.callId) return false;
    if (
      envelope.senderPeerId !== this.snapshot.peerId ||
      envelope.recipientPeerId !== this.snapshot.localPeerId
    ) {
      return false;
    }
    if ('senderPeerId' in payload && payload.senderPeerId !== this.snapshot.peerId) return false;
    if (envelope.payload.type === 'answer') {
      const answer = envelope.payload.payload;
      return (
        answer.callerPeerId === this.snapshot.localPeerId &&
        answer.calleePeerId === this.snapshot.peerId
      );
    }
    return true;
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

  private async sendLocalIce(candidate: RTCIceCandidateInit, generation: number): Promise<void> {
    if (generation !== this.lifecycleGeneration) return;
    const { callId, peerId } = this.snapshot;
    if (!callId || !peerId) {
      this.pendingLocalIce.push(candidate);
      return;
    }
    await callingService.sendIceCandidate(
      callId,
      peerId,
      candidate.candidate ?? '',
      candidate.sdpMid ?? undefined,
      candidate.sdpMLineIndex ?? undefined,
    );
  }

  private async flushPendingLocalIce(generation: number): Promise<void> {
    const pending = this.pendingLocalIce.splice(0);
    for (const candidate of pending) {
      await this.sendLocalIce(candidate, generation);
    }
  }

  private async flushPendingRemoteIce(): Promise<void> {
    const pending = this.pendingRemoteIce.splice(0);
    for (const candidate of pending) {
      await this.connectionRuntime?.peerConnection.addIceCandidate(candidate);
    }
  }

  private startCallTimeout(): void {
    this.clearCallTimeout();
    const generation = this.lifecycleGeneration;
    this.timeoutHandle = setTimeout(() => {
      if (generation !== this.lifecycleGeneration) return;
      const { callId, peerId } = this.snapshot;
      this.finish('timeout', 'Call timed out before media connected.');
      if (callId && peerId) {
        void callingService.hangupCall(callId, peerId, 'timeout').catch((error) => {
          console.warn(
            '[Call] Failed to notify the peer about a call timeout:',
            errorMessage(error),
          );
        });
      }
    }, this.options.timeoutMs);
  }

  private bindLocalVideoTrack(track: MediaStreamTrack, generation: number): void {
    const refresh = () => {
      if (generation !== this.lifecycleGeneration) return;
      const tracks = this.localStream ? getVideoTracks(this.localStream) : [];
      const available = tracks.some(isLiveVideoTrack);
      this.update({
        localVideoEnabled: available,
        localVideoStream: available ? this.localVideoStream : null,
        mediaMode: available || this.snapshot.remoteVideoAvailable ? 'video' : 'audio',
      });
    };
    track.addEventListener?.('ended', refresh);
    track.addEventListener?.('mute', refresh);
    track.addEventListener?.('unmute', refresh);
  }

  private bindRemoteVideoTrack(track: MediaStreamTrack, generation: number): void {
    const refresh = () => {
      if (generation !== this.lifecycleGeneration) return;
      this.refreshRemoteVideoState();
    };
    track.addEventListener?.('ended', refresh);
    track.addEventListener?.('mute', refresh);
    track.addEventListener?.('unmute', refresh);
  }

  private refreshRemoteVideoState(): void {
    const tracks = this.remoteVideoStream ? getVideoTracks(this.remoteVideoStream) : [];
    const available = tracks.some(isLiveVideoTrack);
    this.update({
      remoteVideoStream: available ? this.remoteVideoStream : null,
      remoteVideoAvailable: available,
      mediaMode: available || this.snapshot.localVideoEnabled ? 'video' : 'audio',
    });
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
    this.lifecycleGeneration += 1;
    this.cleanup();
    this.update({ state, terminalReason: reason, error: message });
  }

  private cleanup(): void {
    this.clearCallTimeout();
    if (this.options.stopLocalTracksOnCleanup !== false) {
      this.localStream?.getTracks().forEach(stopMediaTrack);
    }
    this.localStream = null;
    this.localVideoStream = null;
    this.remoteStream?.getTracks().forEach(stopMediaTrack);
    this.remoteStream = null;
    this.remoteVideoStream?.getTracks().forEach(stopMediaTrack);
    this.remoteVideoStream = null;
    if (this.remoteAudio) {
      this.remoteAudio.pause?.();
      this.remoteAudio.srcObject = null;
    }
    this.remoteAudio = null;
    this.connectionRuntime?.close();
    this.connectionRuntime = null;
    this.pendingLocalIce = [];
    this.pendingRemoteIce = [];
    this.remoteAnswerApplication = null;
    this.seenRemoteIce.clear();
    this.update({
      localVideoEnabled: false,
      localVideoStream: null,
      remoteVideoStream: null,
      remoteVideoAvailable: false,
      remoteAudioBlocked: false,
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
      if (snapshot.terminalReason === 'declined') return 'declined';
      if (snapshot.terminalReason === 'timeout') return 'timed_out';
      if (snapshot.terminalReason === 'peer_disconnected') return 'disconnected';
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
    if (participants.every((participant) => participant.state === 'failed')) return 'failed';
    if (participants.some((participant) => participant.state === 'invited')) return current;
    return 'ended';
  }
  if (
    participants.some(
      (participant) =>
        participant.state === 'failed' ||
        participant.state === 'degraded' ||
        participant.state === 'declined' ||
        participant.state === 'timed_out' ||
        participant.state === 'disconnected',
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
  private readonly captureMediaDevices?: Pick<MediaDevices, 'getUserMedia'>;
  private readonly onStateChange?: (snapshot: GroupCallRuntimeSnapshot) => void;
  private runtimes = new Map<string, AudioCallRuntime>();
  private participantSnapshots = new Map<string, AudioCallRuntimeSnapshot>();
  private failedParticipants = new Map<string, string>();
  private declinedParticipants = new Set<string>();
  private sharedMediaStream: MediaStream | null = null;
  private sharedMediaRequest: Promise<void> | null = null;
  private unavailableMediaKinds = new Set<'audio' | 'video'>();
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
    this.captureMediaDevices = childOptions.mediaDevices;
    this.childOptions = {
      ...childOptions,
      stopLocalTracksOnCleanup: false,
      mediaDevices: {
        getUserMedia: (constraints) => this.cloneSharedMedia(constraints),
      },
    };
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
    this.validateMembership(membership, localPeerId);
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
      if (!membership.participants.includes(peerId) || peerId === localPeerId) continue;
      offeredPeers.add(peerId);
      const participantRuntime = this.createParticipantRuntime(peerId);
      this.runtimes.set(peerId, participantRuntime);
      try {
        await participantRuntime.acceptIncomingCall(envelope);
        this.participantSnapshots.set(peerId, participantRuntime.getSnapshot());
      } catch (error) {
        this.failedParticipants.set(peerId, errorMessage(error));
      }
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
        try {
          const participantSnapshot = await participantRuntime.startOutgoingCall(peerId, {
            ...options,
            video: membership.mediaMode === 'video',
          });
          this.participantSnapshots.set(peerId, participantSnapshot);
        } catch (error) {
          this.failedParticipants.set(peerId, errorMessage(error));
        }
      }),
    );
    this.recompute();
    if (this.snapshot.participants.every((participant) => participant.state === 'failed')) {
      const error = 'Could not establish media with any group-call participant.';
      this.update({ state: 'failed', error });
      throw new Error(error);
    }
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

  async enableRemoteAudio(): Promise<boolean> {
    const results = await Promise.all(
      [...this.runtimes.entries()].map(async ([peerId, runtime]) => {
        const enabled = await runtime.enableRemoteAudio();
        this.participantSnapshots.set(peerId, runtime.getSnapshot());
        return enabled;
      }),
    );
    this.recompute();
    return results.some(Boolean);
  }

  handleParticipantLeft(peerId: string): void {
    this.runtimes.get(peerId)?.dispose();
    this.runtimes.delete(peerId);
    this.participantSnapshots.delete(peerId);
    this.failedParticipants.delete(peerId);
    this.update({
      participantCount: Math.max(1, this.snapshot.participantCount - 1),
      participants: this.snapshot.participants.filter(
        (participant) => participant.peerId !== peerId,
      ),
    });
    this.recompute();
  }

  async handleParticipantFailed(
    peerId: string,
    error = 'Participant could not establish call media.',
  ): Promise<void> {
    const runtime = this.runtimes.get(peerId);
    if (runtime) {
      await runtime.hangup('error').catch(() => runtime.dispose());
    }
    this.runtimes.delete(peerId);
    this.participantSnapshots.delete(peerId);
    this.failedParticipants.set(peerId, error);
    this.recompute();
  }

  async handleParticipantDeclined(peerId: string): Promise<void> {
    const runtime = this.runtimes.get(peerId);
    if (runtime) {
      await runtime.hangup('declined').catch(() => runtime.dispose());
    }
    this.runtimes.delete(peerId);
    this.participantSnapshots.delete(peerId);
    this.failedParticipants.delete(peerId);
    this.declinedParticipants.add(peerId);
    this.recompute();
  }

  async retryParticipant(
    peerId: string,
    options: StartCallOptions = {},
  ): Promise<GroupCallRuntimeSnapshot> {
    if (!this.snapshot.participants.some((participant) => participant.peerId === peerId)) {
      throw new Error('This participant is not part of the active group call.');
    }
    this.runtimes.get(peerId)?.dispose();
    this.runtimes.delete(peerId);
    this.participantSnapshots.delete(peerId);
    this.failedParticipants.delete(peerId);
    this.declinedParticipants.delete(peerId);

    const runtime = this.createParticipantRuntime(peerId);
    this.runtimes.set(peerId, runtime);
    try {
      const snapshot = await runtime.startOutgoingCall(peerId, {
        ...options,
        video: options.video ?? this.snapshot.mediaMode === 'video',
      });
      this.participantSnapshots.set(peerId, snapshot);
    } catch (error) {
      this.failedParticipants.set(peerId, errorMessage(error));
      this.recompute();
      throw error;
    }
    this.recompute();
    return this.getSnapshot();
  }

  async startOutgoingGroupCall(
    peerIds: string[],
    options: StartGroupCallOptions = {},
  ): Promise<GroupCallRuntimeSnapshot> {
    this.ensureIdle();
    const uniquePeerIds = [
      ...new Set(peerIds.filter((peerId) => Boolean(peerId) && peerId !== options.localPeerId)),
    ];
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

    // Bring up one leg at a time. Concurrent WebRTC offer creation can stall a
    // shared WebKit/GStreamer process and hold the frontend control request
    // beyond its bounded timeout. A four-person mesh has at most three remote
    // legs, so deterministic sequential setup remains fast and avoids that
    // process-level contention.
    for (const peerId of uniquePeerIds) {
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
    }

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
      [...this.runtimes.entries()].map(async ([peerId, runtime]) => {
        try {
          await runtime.setCameraEnabled(enabled);
          this.participantSnapshots.set(peerId, runtime.getSnapshot());
        } catch {
          // Camera denial is a per-leg video degradation, not a failed audio
          // session. AudioCallRuntime records the actionable camera error.
          this.participantSnapshots.set(peerId, runtime.getSnapshot());
        }
      }),
    );
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
    this.declinedParticipants.clear();
    this.sharedMediaStream?.getTracks().forEach(stopMediaTrack);
    this.sharedMediaStream = null;
    this.sharedMediaRequest = null;
    this.unavailableMediaKinds.clear();
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

  /**
   * Capture each local device once per group session, then clone tracks for
   * each independent peer connection. This avoids repeated permission/device
   * acquisition and keeps one participant runtime from stopping another leg's
   * local media during isolated teardown.
   */
  private async cloneSharedMedia(
    constraints: MediaStreamConstraints = { audio: true },
  ): Promise<MediaStream> {
    const needsAudio = Boolean(constraints.audio);
    const needsVideo = Boolean(constraints.video);
    const hasRequiredTracks = () => {
      const tracks = this.sharedMediaStream?.getTracks() ?? [];
      const hasAudio = tracks.some(
        (track) => track.kind === 'audio' && track.readyState !== 'ended',
      );
      const hasVideo = tracks.some(
        (track) => track.kind === 'video' && track.readyState !== 'ended',
      );
      return (
        (!needsAudio || hasAudio || this.unavailableMediaKinds.has('audio')) &&
        (!needsVideo || hasVideo || this.unavailableMediaKinds.has('video'))
      );
    };

    if (!hasRequiredTracks()) {
      if (!this.sharedMediaRequest) {
        const currentTracks = this.sharedMediaStream?.getTracks() ?? [];
        const requestAudio =
          needsAudio &&
          !this.unavailableMediaKinds.has('audio') &&
          !currentTracks.some((track) => track.kind === 'audio');
        const requestVideo =
          needsVideo &&
          !this.unavailableMediaKinds.has('video') &&
          !currentTracks.some((track) => track.kind === 'video');
        this.sharedMediaRequest = getMediaDevices(this.captureMediaDevices)
          .getUserMedia({
            audio: requestAudio ? constraints.audio : false,
            video: requestVideo ? constraints.video : false,
          })
          .then((captured) => {
            if (requestAudio && captured.getAudioTracks().length === 0) {
              this.unavailableMediaKinds.add('audio');
            }
            if (requestVideo && captured.getVideoTracks().length === 0) {
              this.unavailableMediaKinds.add('video');
            }
            if (!this.sharedMediaStream) {
              this.sharedMediaStream = captured;
              return;
            }
            if (captured !== this.sharedMediaStream) {
              for (const track of captured.getTracks()) this.sharedMediaStream.addTrack(track);
            }
          })
          .finally(() => {
            this.sharedMediaRequest = null;
          });
      }
      await this.sharedMediaRequest;
    }

    const stream = new MediaStream();
    for (const track of this.sharedMediaStream?.getTracks() ?? []) {
      if ((track.kind === 'audio' && !needsAudio) || (track.kind === 'video' && !needsVideo)) {
        continue;
      }
      stream.addTrack(track);
    }
    return stream;
  }

  private validateMembership(membership: GroupMembershipSignal, localPeerId: string): void {
    const canonical = [...new Set(membership.participants)].sort();
    if (
      membership.topology !== GROUP_CALL_TOPOLOGY ||
      canonical.length < 2 ||
      canonical.length > GROUP_CALL_MAX_PARTICIPANTS ||
      canonical.some((peerId, index) => peerId !== membership.participants[index]) ||
      !canonical.includes(localPeerId) ||
      !canonical.includes(membership.creatorPeerId)
    ) {
      throw new Error('Invalid group-call membership roster.');
    }
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
      remoteAudioBlocked: false,
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
      remoteAudioBlocked: false,
      cameraError: null,
    };
  }

  private recompute(): void {
    const peers = new Set([
      ...this.snapshot.participants.map((participant) => participant.peerId),
      ...this.participantSnapshots.keys(),
      ...this.failedParticipants.keys(),
      ...this.declinedParticipants,
    ]);
    const participants = [...peers].map((peerId) => {
      const snapshot = this.participantSnapshots.get(peerId);
      const failure = this.failedParticipants.get(peerId);
      const declined = this.declinedParticipants.has(peerId);
      if (!snapshot) {
        return {
          ...this.placeholderParticipant(peerId, this.snapshot.mediaMode),
          error: failure ?? (declined ? 'Participant declined the group call.' : null),
          state: failure
            ? ('failed' as const)
            : declined
              ? ('declined' as const)
              : ('invited' as const),
        };
      }
      return {
        peerId,
        state: failure ? 'failed' : declined ? 'declined' : groupParticipantState(snapshot),
        callId: snapshot.callId,
        mediaMode: snapshot.mediaMode,
        muted: false,
        cameraEnabled: snapshot.localVideoEnabled,
        localVideoStream: snapshot.localVideoStream,
        remoteVideoStream: snapshot.remoteVideoStream,
        remoteVideoAvailable: snapshot.remoteVideoAvailable,
        remoteAudioBlocked: snapshot.remoteAudioBlocked,
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
