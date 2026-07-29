import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  AudioCallRuntime,
  GROUP_CALL_MAX_PARTICIPANTS,
  GroupMeshCallRuntime,
  normalizeSdpPayloadTypes,
} from './callingRuntime';
import { callingService } from './calling';
import type { SignalingEnvelope } from '../types';

vi.mock('./calling', () => ({
  callingService: {
    startCall: vi.fn(),
    answerCall: vi.fn(),
    sendIceCandidate: vi.fn(),
    hangupCall: vi.fn(),
  },
}));

describe('normalizeSdpPayloadTypes', () => {
  it('remaps a codec payload reused with a conflicting signature across media sections', () => {
    const input = [
      'v=0',
      'm=audio 9 UDP/TLS/RTP/SAVPF 97 111',
      'a=rtpmap:97 red/48000/2',
      'a=rtpmap:111 opus/48000/2',
      'a=rtpmap:127 ulpfec/90000',
      'm=video 9 UDP/TLS/RTP/SAVPF 96 97',
      'a=rtpmap:96 VP8/90000',
      'a=rtpmap:97 rtx/90000',
      'a=fmtp:97 apt=96',
      'a=rtcp-fb:97 nack',
      '',
    ].join('\r\n');

    const output = normalizeSdpPayloadTypes(input);

    expect(output).toContain('m=audio 9 UDP/TLS/RTP/SAVPF 97 111');
    expect(output).toContain('a=rtpmap:97 red/48000/2');
    expect(output).toContain('m=video 9 UDP/TLS/RTP/SAVPF 96 126');
    expect(output).toContain('a=rtpmap:126 rtx/90000');
    expect(output).toContain('a=fmtp:126 apt=96');
    expect(output).toContain('a=rtcp-fb:126 nack');
    expect(output.endsWith('\r\n')).toBe(true);
  });

  it('drops duplicate RTX assigned to a primary codec payload in one media section', () => {
    const input = [
      'v=0',
      'm=video 9 UDP/TLS/RTP/SAVPF 97',
      'a=rtpmap:97 VP8/90000',
      'a=rtpmap:97 rtx/90000',
      'a=fmtp:97 apt=97',
      '',
    ].join('\r\n');

    const output = normalizeSdpPayloadTypes(input);

    expect(output).toContain('a=rtpmap:97 VP8/90000');
    expect(output).not.toContain('rtx/90000');
    expect(output).not.toContain('apt=97');
  });
});

class MockTrack {
  stopped = false;
  enabled = true;
  muted = false;
  readyState: MediaStreamTrackState = 'live';
  kind: 'audio' | 'video';
  listeners = new Map<string, Array<() => void>>();

  constructor(kind: 'audio' | 'video' = 'audio') {
    this.kind = kind;
  }

  stop = vi.fn(() => {
    this.stopped = true;
    this.readyState = 'ended';
  });

  addEventListener(type: string, listener: () => void) {
    this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener]);
  }

  emit(type: 'ended' | 'mute' | 'unmute') {
    if (type === 'ended') this.readyState = 'ended';
    if (type === 'mute') this.muted = true;
    if (type === 'unmute') this.muted = false;
    for (const listener of this.listeners.get(type) ?? []) listener();
  }
}

class MockMediaStream {
  tracks: MockTrack[];

  constructor(tracks: MockTrack[] = []) {
    this.tracks = tracks;
  }

  getAudioTracks() {
    return this.tracks.filter((track) => track.kind === 'audio');
  }

  getVideoTracks() {
    return this.tracks.filter((track) => track.kind === 'video');
  }

  getTracks() {
    return this.tracks;
  }

  addTrack(track: MockTrack) {
    this.tracks.push(track);
  }

  removeTrack(track: MockTrack) {
    this.tracks = this.tracks.filter((candidate) => candidate !== track);
  }
}

class MockPeerConnection {
  localDescription: RTCSessionDescriptionInit | null = null;
  remoteDescription: RTCSessionDescriptionInit | null = null;
  iceGatheringState: RTCIceGatheringState = 'new';
  iceConnectionState: RTCIceConnectionState = 'new';
  connectionState: RTCPeerConnectionState = 'new';
  senders: Array<{ track: MockTrack; replaceTrack: ReturnType<typeof vi.fn> }> = [];
  transceivers: Array<{
    sender: { track: MockTrack | null; replaceTrack: ReturnType<typeof vi.fn> };
    receiver: { track: MockTrack };
  }> = [];
  addTrack = vi.fn((track: MockTrack) => {
    const sender = {
      track,
      replaceTrack: vi.fn(async (replacement: MockTrack | null) => {
        sender.track = replacement ?? track;
      }),
    };
    this.senders.push(sender);
    return sender;
  });
  getSenders = vi.fn(() => this.senders);
  addTransceiver = vi.fn((kind: string) => {
    const sender = {
      track: null as MockTrack | null,
      replaceTrack: vi.fn(async (replacement: MockTrack | null) => {
        sender.track = replacement;
      }),
    };
    const transceiver = { sender, receiver: { track: new MockTrack(kind as 'video') } };
    this.transceivers.push(transceiver);
    return transceiver;
  });
  getTransceivers = vi.fn(() => this.transceivers);
  addIceCandidate = vi.fn(async () => undefined);
  close = vi.fn();
  listeners = new Map<string, Array<(event: any) => void>>();

  addEventListener(type: string, listener: (event: any) => void) {
    this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener]);
  }

  removeEventListener(type: string, listener: (event: any) => void) {
    this.listeners.set(
      type,
      (this.listeners.get(type) ?? []).filter((candidate) => candidate !== listener),
    );
  }

  async createOffer(): Promise<RTCSessionDescriptionInit> {
    return { type: 'offer', sdp: 'v=0\r\noffer' };
  }

  async createAnswer(): Promise<RTCSessionDescriptionInit> {
    return { type: 'answer', sdp: 'v=0\r\nanswer' };
  }

  async setLocalDescription(description: RTCSessionDescriptionInit) {
    this.localDescription = description;
    this.iceGatheringState = 'complete';
    this.emit('icegatheringstatechange');
  }

  async setRemoteDescription(description: RTCSessionDescriptionInit) {
    this.remoteDescription = description;
  }

  emit(type: string, event: any = {}) {
    for (const listener of this.listeners.get(type) ?? []) {
      listener(event);
    }
  }
}

function audioElement(): HTMLAudioElement {
  return {
    autoplay: false,
    srcObject: null,
    play: vi.fn(async () => undefined),
    pause: vi.fn(),
  } as unknown as HTMLAudioElement;
}

function offerEnvelope(): SignalingEnvelope {
  return {
    senderPeerId: 'peer-alice',
    recipientPeerId: 'peer-local',
    payload: {
      type: 'offer',
      payload: {
        callId: 'call-1',
        callerPeerId: 'peer-alice',
        calleePeerId: 'peer-local',
        sdp: 'v=0\r\nremote-offer',
        timestamp: 100,
        signature: [1],
      },
    },
  };
}

function answerEnvelope(sdp = 'v=0\r\nremote-answer'): SignalingEnvelope {
  return {
    senderPeerId: 'peer-alice',
    recipientPeerId: 'peer-local',
    payload: {
      type: 'answer',
      payload: {
        callId: 'call-1',
        callerPeerId: 'peer-local',
        calleePeerId: 'peer-alice',
        sdp,
        timestamp: 100,
        signature: [1],
      },
    },
  };
}

function hangupEnvelope(reason = 'timeout'): SignalingEnvelope {
  return {
    senderPeerId: 'peer-alice',
    recipientPeerId: 'peer-local',
    payload: {
      type: 'hangup',
      payload: {
        callId: 'call-1',
        senderPeerId: 'peer-alice',
        reason,
        timestamp: 100,
        signature: [1],
      },
    },
  };
}

function iceEnvelope(): SignalingEnvelope {
  return {
    senderPeerId: 'peer-alice',
    recipientPeerId: 'peer-local',
    payload: {
      type: 'ice',
      payload: {
        callId: 'call-1',
        senderPeerId: 'peer-alice',
        candidate: 'candidate:late',
        sdpMid: '0',
        sdpMlineIndex: 0,
        timestamp: 100,
        signature: [1],
      },
    },
  };
}

function runtimeWith(
  pc: MockPeerConnection,
  getUserMedia?: ReturnType<typeof vi.fn>,
  audioElementFactory: () => HTMLAudioElement = audioElement,
) {
  const track = new MockTrack();
  const getUserMediaMock = getUserMedia ?? vi.fn().mockResolvedValue(new MockMediaStream([track]));
  return {
    track,
    runtime: new AudioCallRuntime({
      iceServers: [],
      timeoutMs: 1_000,
      mediaDevices: { getUserMedia: getUserMediaMock },
      audioElementFactory,
      peerConnectionFactory: () => pc as unknown as RTCPeerConnection,
    }),
    getUserMedia: getUserMediaMock,
  };
}

describe('AudioCallRuntime', () => {
  beforeEach(() => {
    vi.useRealTimers();
    vi.resetAllMocks();
    vi.stubGlobal('MediaStream', MockMediaStream);
    vi.mocked(callingService.sendIceCandidate).mockResolvedValue({
      callId: 'call-1',
      senderPeerId: 'peer-local',
      targetPeerId: 'peer-alice',
      candidate: 'candidate:local',
      sdpMid: '0',
      sdpMlineIndex: 0,
      timestamp: 100,
      signature: [1],
    });
    vi.mocked(callingService.hangupCall).mockResolvedValue({
      callId: 'call-1',
      senderPeerId: 'peer-local',
      targetPeerId: 'peer-alice',
      reason: 'normal',
      timestamp: 100,
      signature: [1],
    });
  });

  it('creates an outgoing audio offer through the signed Tauri signaling command', async () => {
    const pc = new MockPeerConnection();
    const { runtime, getUserMedia } = runtimeWith(pc);
    vi.mocked(callingService.startCall).mockResolvedValue({
      callId: 'call-1',
      callerPeerId: 'peer-local',
      calleePeerId: 'peer-alice',
      sdp: 'v=0\r\noffer',
      timestamp: 100,
      signature: [1],
    });

    const snapshot = await runtime.startOutgoingCall('peer-alice');

    expect(getUserMedia).toHaveBeenCalledWith({ audio: true, video: false });
    expect(pc.addTrack).toHaveBeenCalledTimes(1);
    expect(callingService.startCall).toHaveBeenCalledWith('peer-alice', 'v=0\r\noffer');
    expect(snapshot).toMatchObject({ state: 'ringing', callId: 'call-1', peerId: 'peer-alice' });
  });

  it('keeps remote media alive and exposes a gesture retry when autoplay is blocked', async () => {
    const pc = new MockPeerConnection();
    const play = vi
      .fn<() => Promise<void>>()
      .mockRejectedValueOnce(new DOMException('gesture required', 'NotAllowedError'))
      .mockResolvedValueOnce(undefined);
    const remoteAudio = {
      autoplay: false,
      srcObject: null,
      play,
      pause: vi.fn(),
    } as unknown as HTMLAudioElement;
    const { runtime } = runtimeWith(pc, undefined, () => remoteAudio);
    vi.mocked(callingService.startCall).mockResolvedValue({
      callId: 'call-1',
      callerPeerId: 'peer-local',
      calleePeerId: 'peer-alice',
      sdp: 'v=0\r\noffer',
      timestamp: 100,
      signature: [1],
    });
    await runtime.startOutgoingCall('peer-alice');

    const remoteTrack = new MockTrack('audio');
    const remoteStream = new MockMediaStream([remoteTrack]);
    pc.emit('track', { track: remoteTrack, streams: [remoteStream] });
    await vi.waitFor(() => expect(runtime.getSnapshot().remoteAudioBlocked).toBe(true));

    await expect(runtime.enableRemoteAudio()).resolves.toBe(true);
    expect(runtime.getSnapshot().remoteAudioBlocked).toBe(false);
    expect(remoteAudio.srcObject).toBe(remoteStream);
    expect(remoteTrack.stop).not.toHaveBeenCalled();
    expect(play).toHaveBeenCalledTimes(2);
  });

  it('stops media acquired after runtime disposal and cannot revive the call', async () => {
    const pc = new MockPeerConnection();
    const lateTrack = new MockTrack('audio');
    let release!: (stream: MockMediaStream) => void;
    const getUserMedia = vi.fn(
      () =>
        new Promise<MockMediaStream>((resolve) => {
          release = resolve;
        }),
    );
    const { runtime } = runtimeWith(pc, getUserMedia);

    const pending = runtime.startOutgoingCall('peer-alice');
    runtime.dispose();
    release(new MockMediaStream([lateTrack]));

    await expect(pending).rejects.toMatchObject({ name: 'AbortError' });
    expect(lateTrack.stop).toHaveBeenCalledOnce();
    expect(pc.addTrack).not.toHaveBeenCalled();
    expect(callingService.startCall).not.toHaveBeenCalled();
    expect(runtime.getSnapshot()).toMatchObject({ state: 'idle', callId: null, peerId: null });
  });

  it('answers an incoming offer with local audio and signed signaling', async () => {
    const pc = new MockPeerConnection();
    const { runtime } = runtimeWith(pc);
    vi.mocked(callingService.answerCall).mockResolvedValue({
      callId: 'call-1',
      callerPeerId: 'peer-alice',
      calleePeerId: 'peer-local',
      sdp: 'v=0\r\nanswer',
      timestamp: 100,
      signature: [1],
    });

    const snapshot = await runtime.acceptIncomingCall(offerEnvelope());

    expect(pc.remoteDescription).toEqual({ type: 'offer', sdp: 'v=0\r\nremote-offer' });
    expect(callingService.answerCall).toHaveBeenCalledWith('call-1', 'peer-alice', 'v=0\r\nanswer');
    expect(snapshot).toMatchObject({ state: 'connecting', direction: 'incoming' });
  });

  it('applies remote answers and forwards local ICE candidates through Tauri', async () => {
    const pc = new MockPeerConnection();
    const setRemoteDescription = vi.spyOn(pc, 'setRemoteDescription');
    const { runtime } = runtimeWith(pc);
    vi.mocked(callingService.startCall).mockResolvedValue({
      callId: 'call-1',
      callerPeerId: 'peer-local',
      calleePeerId: 'peer-alice',
      sdp: 'v=0\r\noffer',
      timestamp: 100,
      signature: [1],
    });
    await runtime.startOutgoingCall('peer-alice');

    await runtime.handleSignalingEnvelope(answerEnvelope());
    await runtime.handleSignalingEnvelope(answerEnvelope());
    pc.emit('icecandidate', {
      candidate: { candidate: 'candidate:local', sdpMid: '0', sdpMLineIndex: 0 },
    });

    expect(pc.remoteDescription).toEqual({ type: 'answer', sdp: 'v=0\r\nremote-answer' });
    expect(setRemoteDescription).toHaveBeenCalledTimes(1);
    expect(callingService.sendIceCandidate).toHaveBeenCalledWith(
      'call-1',
      'peer-alice',
      'candidate:local',
      '0',
      0,
    );
  });

  it('buffers local ICE gathered before the signed call ID is available', async () => {
    const pc = new MockPeerConnection();
    const { runtime } = runtimeWith(pc);
    let resolveStart!: (result: Awaited<ReturnType<typeof callingService.startCall>>) => void;
    vi.mocked(callingService.startCall).mockReturnValueOnce(
      new Promise((resolve) => {
        resolveStart = resolve;
      }),
    );

    const pendingStart = runtime.startOutgoingCall('peer-alice');
    await vi.waitFor(() => expect(callingService.startCall).toHaveBeenCalledOnce());
    pc.emit('icecandidate', {
      candidate: { candidate: 'candidate:early', sdpMid: '0', sdpMLineIndex: 0 },
    });
    expect(callingService.sendIceCandidate).not.toHaveBeenCalled();

    resolveStart({
      callId: 'call-1',
      callerPeerId: 'peer-local',
      calleePeerId: 'peer-alice',
      sdp: 'v=0\r\noffer',
      timestamp: 100,
      signature: [1],
    });
    await pendingStart;

    expect(callingService.sendIceCandidate).toHaveBeenCalledWith(
      'call-1',
      'peer-alice',
      'candidate:early',
      '0',
      0,
    );
  });

  it('maps a signed remote media failure to a failed call leg', async () => {
    const pc = new MockPeerConnection();
    const { runtime } = runtimeWith(pc);
    vi.mocked(callingService.startCall).mockResolvedValue({
      callId: 'call-1',
      callerPeerId: 'peer-local',
      calleePeerId: 'peer-alice',
      sdp: 'v=0\r\noffer',
      timestamp: 100,
      signature: [1],
    });
    await runtime.startOutgoingCall('peer-alice');

    await runtime.handleSignalingEnvelope(hangupEnvelope('error'));

    expect(runtime.getSnapshot()).toMatchObject({
      state: 'failed',
      terminalReason: 'error',
      error: 'The other participant could not start call media.',
    });
  });

  it('deduplicates remote ICE candidates before adding them to the peer connection', async () => {
    const pc = new MockPeerConnection();
    const { runtime } = runtimeWith(pc);
    vi.mocked(callingService.answerCall).mockResolvedValue({
      callId: 'call-1',
      callerPeerId: 'peer-alice',
      calleePeerId: 'peer-local',
      sdp: 'v=0\r\nanswer',
      timestamp: 100,
      signature: [1],
    });
    await runtime.acceptIncomingCall(offerEnvelope());
    const iceEnvelope: SignalingEnvelope = {
      senderPeerId: 'peer-alice',
      recipientPeerId: 'peer-local',
      payload: {
        type: 'ice',
        payload: {
          callId: 'call-1',
          senderPeerId: 'peer-alice',
          candidate: 'candidate:remote',
          sdpMid: '0',
          sdpMlineIndex: 0,
          timestamp: 100,
          signature: [1],
        },
      },
    };

    await runtime.handleSignalingEnvelope(iceEnvelope);
    await runtime.handleSignalingEnvelope(iceEnvelope);

    expect(pc.addIceCandidate).toHaveBeenCalledTimes(1);
  });

  it('creates an outgoing video offer with camera capture and local preview state', async () => {
    const pc = new MockPeerConnection();
    const audioTrack = new MockTrack('audio');
    const videoTrack = new MockTrack('video');
    const getUserMedia = vi.fn().mockResolvedValue(new MockMediaStream([audioTrack, videoTrack]));
    const { runtime } = runtimeWith(pc, getUserMedia);
    vi.mocked(callingService.startCall).mockResolvedValue({
      callId: 'call-1',
      callerPeerId: 'peer-local',
      calleePeerId: 'peer-alice',
      sdp: 'v=0\r\noffer\r\nm=video 9 UDP/TLS/RTP/SAVPF 96',
      timestamp: 100,
      signature: [1],
    });

    const snapshot = await runtime.startOutgoingCall('peer-alice', { video: true });

    expect(getUserMedia).toHaveBeenCalledWith({ audio: true, video: true });
    expect(pc.addTrack).toHaveBeenCalledTimes(2);
    expect(snapshot).toMatchObject({
      mediaMode: 'video',
      videoRequested: true,
      localVideoEnabled: true,
    });
    expect(snapshot.localVideoStream?.getVideoTracks()).toEqual([videoTrack]);
  });

  it('reports remote video only while a live remote track is present', async () => {
    const pc = new MockPeerConnection();
    const audioTrack = new MockTrack('audio');
    const videoTrack = new MockTrack('video');
    const { runtime } = runtimeWith(
      pc,
      vi.fn().mockResolvedValue(new MockMediaStream([audioTrack, videoTrack])),
    );
    vi.mocked(callingService.startCall).mockResolvedValue({
      callId: 'call-1',
      callerPeerId: 'peer-local',
      calleePeerId: 'peer-alice',
      sdp: 'v=0\r\noffer\r\nm=video 9 UDP/TLS/RTP/SAVPF 96',
      timestamp: 100,
      signature: [1],
    });
    await runtime.startOutgoingCall('peer-alice', { video: true });

    await runtime.handleSignalingEnvelope(
      answerEnvelope('v=0\r\nanswer\r\nm=video 9 UDP/TLS/RTP/SAVPF 96'),
    );
    expect(runtime.getSnapshot().remoteVideoAvailable).toBe(false);

    const remoteVideo = new MockTrack('video');
    const remoteStream = new MockMediaStream([remoteVideo]);
    pc.emit('track', { track: remoteVideo, streams: [remoteStream] });
    expect(runtime.getSnapshot()).toMatchObject({
      remoteVideoAvailable: true,
      mediaMode: 'video',
    });

    remoteVideo.emit('mute');
    expect(runtime.getSnapshot().remoteVideoAvailable).toBe(false);
    remoteVideo.emit('unmute');
    expect(runtime.getSnapshot().remoteVideoAvailable).toBe(true);
    remoteVideo.emit('ended');
    expect(runtime.getSnapshot().remoteVideoAvailable).toBe(false);

    videoTrack.emit('ended');
    expect(runtime.getSnapshot()).toMatchObject({
      localVideoEnabled: false,
      mediaMode: 'audio',
    });
  });

  it('falls back to audio when camera capture fails during a video call', async () => {
    const pc = new MockPeerConnection();
    const audioTrack = new MockTrack('audio');
    const getUserMedia = vi
      .fn()
      .mockRejectedValueOnce(new DOMException('camera denied', 'NotAllowedError'))
      .mockResolvedValueOnce(new MockMediaStream([audioTrack]));
    const { runtime } = runtimeWith(pc, getUserMedia);
    vi.mocked(callingService.startCall).mockResolvedValue({
      callId: 'call-1',
      callerPeerId: 'peer-local',
      calleePeerId: 'peer-alice',
      sdp: 'v=0\r\noffer',
      timestamp: 100,
      signature: [1],
    });

    const snapshot = await runtime.startOutgoingCall('peer-alice', { video: true });

    expect(getUserMedia).toHaveBeenNthCalledWith(1, { audio: true, video: true });
    expect(getUserMedia).toHaveBeenNthCalledWith(2, { audio: true, video: false });
    expect(snapshot.mediaMode).toBe('audio');
    expect(snapshot.videoRequested).toBe(true);
    expect(snapshot.cameraError).toContain('does not have permission');
    expect(callingService.startCall).toHaveBeenCalled();
  });

  it('uses the pre-negotiated video sender when camera becomes available after audio fallback', async () => {
    const pc = new MockPeerConnection();
    const audioTrack = new MockTrack('audio');
    const recoveredVideo = new MockTrack('video');
    const getUserMedia = vi
      .fn()
      .mockRejectedValueOnce(new DOMException('camera denied', 'NotAllowedError'))
      .mockResolvedValueOnce(new MockMediaStream([audioTrack]))
      .mockResolvedValueOnce(new MockMediaStream([recoveredVideo]));
    const { runtime } = runtimeWith(pc, getUserMedia);
    vi.mocked(callingService.startCall).mockResolvedValue({
      callId: 'call-1',
      callerPeerId: 'peer-local',
      calleePeerId: 'peer-alice',
      sdp: 'v=0\r\noffer',
      timestamp: 100,
      signature: [1],
    });

    await runtime.startOutgoingCall('peer-alice', { video: true });
    expect(pc.addTransceiver).toHaveBeenCalledWith('video', { direction: 'sendrecv' });

    await runtime.setCameraEnabled(true);

    expect(pc.transceivers[0].sender.replaceTrack).toHaveBeenCalledWith(recoveredVideo);
    expect(pc.addTrack).toHaveBeenCalledTimes(1);
    expect(audioTrack.stop).not.toHaveBeenCalled();
    expect(runtime.getSnapshot()).toMatchObject({
      mediaMode: 'video',
      localVideoEnabled: true,
      cameraError: null,
    });
  });

  it('toggles and switches the local camera without dropping audio', async () => {
    const pc = new MockPeerConnection();
    const audioTrack = new MockTrack('audio');
    const firstVideo = new MockTrack('video');
    const secondVideo = new MockTrack('video');
    const getUserMedia = vi
      .fn()
      .mockResolvedValueOnce(new MockMediaStream([audioTrack, firstVideo]))
      .mockResolvedValueOnce(new MockMediaStream([secondVideo]));
    const { runtime } = runtimeWith(pc, getUserMedia);
    vi.mocked(callingService.startCall).mockResolvedValue({
      callId: 'call-1',
      callerPeerId: 'peer-local',
      calleePeerId: 'peer-alice',
      sdp: 'v=0\r\noffer\r\nm=video 9 UDP/TLS/RTP/SAVPF 96',
      timestamp: 100,
      signature: [1],
    });
    await runtime.startOutgoingCall('peer-alice', { video: true });

    await runtime.setCameraEnabled(false);
    expect(firstVideo.enabled).toBe(false);
    await runtime.setCameraEnabled(true);
    expect(firstVideo.enabled).toBe(true);
    await runtime.switchCamera('camera-2');

    expect(getUserMedia).toHaveBeenLastCalledWith({
      audio: false,
      video: { deviceId: { exact: 'camera-2' } },
    });
    expect(firstVideo.stop).toHaveBeenCalled();
    expect(audioTrack.stop).not.toHaveBeenCalled();
    expect(runtime.getSnapshot().localVideoStream?.getVideoTracks()).toEqual([secondVideo]);
  });

  it('stops a replacement camera track when WebRTC rejects the switch', async () => {
    const pc = new MockPeerConnection();
    const audioTrack = new MockTrack('audio');
    const firstVideo = new MockTrack('video');
    const rejectedVideo = new MockTrack('video');
    const getUserMedia = vi
      .fn()
      .mockResolvedValueOnce(new MockMediaStream([audioTrack, firstVideo]))
      .mockResolvedValueOnce(new MockMediaStream([rejectedVideo]));
    const { runtime } = runtimeWith(pc, getUserMedia);
    vi.mocked(callingService.startCall).mockResolvedValue({
      callId: 'call-1',
      callerPeerId: 'peer-local',
      calleePeerId: 'peer-alice',
      sdp: 'v=0\r\noffer\r\nm=video 9 UDP/TLS/RTP/SAVPF 96',
      timestamp: 100,
      signature: [1],
    });
    await runtime.startOutgoingCall('peer-alice', { video: true });
    pc.senders
      .find((sender) => sender.track.kind === 'video')
      ?.replaceTrack.mockRejectedValueOnce(new Error('sender closed'));

    await expect(runtime.switchCamera('camera-broken')).rejects.toThrow('sender closed');

    expect(rejectedVideo.stop).toHaveBeenCalledTimes(1);
    expect(firstVideo.stop).not.toHaveBeenCalled();
    expect(audioTrack.stop).not.toHaveBeenCalled();
    expect(runtime.getSnapshot().localVideoStream?.getVideoTracks()).toEqual([firstVideo]);
  });

  it('hangs up via signed Tauri signaling and cleans up local media', async () => {
    const pc = new MockPeerConnection();
    const { runtime, track } = runtimeWith(pc);
    vi.mocked(callingService.startCall).mockResolvedValue({
      callId: 'call-1',
      callerPeerId: 'peer-local',
      calleePeerId: 'peer-alice',
      sdp: 'v=0\r\noffer',
      timestamp: 100,
      signature: [1],
    });
    vi.mocked(callingService.hangupCall).mockResolvedValue({
      callId: 'call-1',
      senderPeerId: 'peer-local',
      targetPeerId: 'peer-alice',
      reason: 'normal',
      timestamp: 100,
      signature: [1],
    });
    await runtime.startOutgoingCall('peer-alice');

    await runtime.hangup();

    expect(callingService.hangupCall).toHaveBeenCalledWith('call-1', 'peer-alice', 'normal');
    expect(track.stop).toHaveBeenCalled();
    expect(pc.close).toHaveBeenCalled();
    expect(runtime.getSnapshot()).toMatchObject({ state: 'ended', terminalReason: 'normal' });
  });

  it('reports permission denial and cleans up when microphone access fails', async () => {
    const pc = new MockPeerConnection();
    const denied = new DOMException('denied', 'NotAllowedError');
    const { runtime } = runtimeWith(pc, vi.fn().mockRejectedValue(denied));

    await expect(runtime.startOutgoingCall('peer-alice')).rejects.toThrow('denied');

    expect(callingService.startCall).not.toHaveBeenCalled();
    expect(pc.close).not.toHaveBeenCalled();
    expect(runtime.getSnapshot()).toMatchObject({
      state: 'failed',
      terminalReason: 'permission_denied',
    });
  });

  it('sends a signed terminal error when incoming media initialization fails', async () => {
    const pc = new MockPeerConnection();
    const denied = new DOMException('microphone denied', 'NotAllowedError');
    const { runtime } = runtimeWith(pc, vi.fn().mockRejectedValue(denied));

    await expect(runtime.acceptIncomingCall(offerEnvelope())).rejects.toThrow('microphone denied');
    await Promise.resolve();

    expect(callingService.answerCall).not.toHaveBeenCalled();
    expect(callingService.hangupCall).toHaveBeenCalledWith('call-1', 'peer-alice', 'error');
    expect(runtime.getSnapshot()).toMatchObject({
      state: 'failed',
      terminalReason: 'permission_denied',
    });
  });

  it('distinguishes a missing platform media API from missing hardware', async () => {
    const pc = new MockPeerConnection();
    const unavailable = new DOMException(
      'The media capture API is not available.',
      'NotSupportedError',
    );
    const { runtime } = runtimeWith(pc, vi.fn().mockRejectedValue(unavailable));

    await expect(runtime.startOutgoingCall('peer-alice')).rejects.toThrow(
      'The media capture API is not available.',
    );

    expect(runtime.getSnapshot()).toMatchObject({
      state: 'failed',
      terminalReason: 'missing_media_api',
      error: 'This Harbor build cannot access the system audio or video API.',
    });
  });

  it('rejects an unsupported WebRTC runtime before media capture or signaling', async () => {
    const getUserMedia = vi.fn();
    vi.stubGlobal('RTCPeerConnection', undefined);
    const runtime = new AudioCallRuntime({
      iceServers: [],
      mediaDevices: { getUserMedia },
      audioElementFactory: audioElement,
    });

    await expect(runtime.startOutgoingCall('peer-alice')).rejects.toThrow(
      'does not provide the WebRTC peer connection API',
    );

    expect(getUserMedia).not.toHaveBeenCalled();
    expect(callingService.startCall).not.toHaveBeenCalled();
    expect(runtime.getSnapshot()).toMatchObject({
      state: 'failed',
      terminalReason: 'missing_media_api',
      error: 'This Harbor build cannot access the system audio or video API.',
    });
  });

  it('terminates on call timeout and peer disconnect', async () => {
    vi.useFakeTimers();
    const pc = new MockPeerConnection();
    const { runtime } = runtimeWith(pc);
    vi.mocked(callingService.startCall).mockResolvedValue({
      callId: 'call-1',
      callerPeerId: 'peer-local',
      calleePeerId: 'peer-alice',
      sdp: 'v=0\r\noffer',
      timestamp: 100,
      signature: [1],
    });
    await runtime.startOutgoingCall('peer-alice');

    vi.advanceTimersByTime(1_000);
    expect(runtime.getSnapshot()).toMatchObject({ state: 'ended', terminalReason: 'timeout' });

    const pc2 = new MockPeerConnection();
    const second = runtimeWith(pc2).runtime;
    await second.startOutgoingCall('peer-alice');
    second.handlePeerDisconnected('peer-alice');

    expect(second.getSnapshot()).toMatchObject({
      state: 'ended',
      terminalReason: 'peer_disconnected',
    });
  });

  it('notifies the peer on timeout and ignores late answer, ICE, and connection events', async () => {
    vi.useFakeTimers();
    const pc = new MockPeerConnection();
    const { runtime } = runtimeWith(pc);
    vi.mocked(callingService.startCall).mockResolvedValue({
      callId: 'call-1',
      callerPeerId: 'peer-local',
      calleePeerId: 'peer-alice',
      sdp: 'v=0\r\noffer',
      timestamp: 100,
      signature: [1],
    });
    await runtime.startOutgoingCall('peer-alice');

    await vi.advanceTimersByTimeAsync(1_000);
    expect(callingService.hangupCall).toHaveBeenCalledWith('call-1', 'peer-alice', 'timeout');
    expect(runtime.getSnapshot()).toMatchObject({ state: 'ended', terminalReason: 'timeout' });

    await runtime.handleSignalingEnvelope(iceEnvelope());
    await runtime.handleSignalingEnvelope(answerEnvelope());
    await runtime.handleSignalingEnvelope(iceEnvelope());
    pc.connectionState = 'connected';
    pc.iceConnectionState = 'connected';
    pc.emit('connectionstatechange');
    pc.emit('iceconnectionstatechange');

    expect(pc.remoteDescription).toBeNull();
    expect(pc.addIceCandidate).not.toHaveBeenCalled();
    expect(runtime.getSnapshot()).toMatchObject({ state: 'ended', terminalReason: 'timeout' });
  });

  it('starts a subsequent call on the same runtime after timeout without accepting stale signaling', async () => {
    vi.useFakeTimers();
    const firstPc = new MockPeerConnection();
    const secondPc = new MockPeerConnection();
    const peerConnections = [firstPc, secondPc];
    const runtime = new AudioCallRuntime({
      iceServers: [],
      timeoutMs: 1_000,
      mediaDevices: {
        getUserMedia: vi.fn().mockResolvedValue(new MockMediaStream([new MockTrack('audio')])),
      },
      audioElementFactory: audioElement,
      peerConnectionFactory: () => peerConnections.shift() as unknown as RTCPeerConnection,
    });
    vi.mocked(callingService.startCall)
      .mockResolvedValueOnce({
        callId: 'call-1',
        callerPeerId: 'peer-local',
        calleePeerId: 'peer-alice',
        sdp: 'v=0\r\noffer-1',
        timestamp: 100,
        signature: [1],
      })
      .mockResolvedValueOnce({
        callId: 'call-2',
        callerPeerId: 'peer-local',
        calleePeerId: 'peer-alice',
        sdp: 'v=0\r\noffer-2',
        timestamp: 101,
        signature: [2],
      });

    await runtime.startOutgoingCall('peer-alice');
    await vi.advanceTimersByTimeAsync(1_000);
    await runtime.handleSignalingEnvelope(answerEnvelope());
    await runtime.handleSignalingEnvelope(iceEnvelope());

    const second = await runtime.startOutgoingCall('peer-alice');

    expect(second).toMatchObject({ state: 'ringing', callId: 'call-2' });
    expect(secondPc.close).not.toHaveBeenCalled();
    expect(firstPc.remoteDescription).toBeNull();
  });

  it('cannot revive an incoming call when remote teardown arrives during media permission', async () => {
    let releaseMedia!: (stream: MockMediaStream) => void;
    const pendingMedia = new Promise<MockMediaStream>((resolve) => {
      releaseMedia = resolve;
    });
    const pc = new MockPeerConnection();
    const { runtime } = runtimeWith(pc, vi.fn().mockReturnValue(pendingMedia));
    vi.mocked(callingService.answerCall).mockResolvedValue({
      callId: 'call-1',
      callerPeerId: 'peer-alice',
      calleePeerId: 'peer-local',
      sdp: 'v=0\r\nanswer',
      timestamp: 100,
      signature: [1],
    });

    const accepting = runtime.acceptIncomingCall(offerEnvelope());
    await Promise.resolve();
    await runtime.handleSignalingEnvelope(hangupEnvelope());
    const lateTrack = new MockTrack('audio');
    releaseMedia(new MockMediaStream([lateTrack]));

    await expect(accepting).rejects.toThrow('call ended');
    expect(lateTrack.stop).toHaveBeenCalledOnce();
    expect(callingService.answerCall).not.toHaveBeenCalled();
    expect(runtime.getSnapshot()).toMatchObject({
      state: 'ended',
      terminalReason: 'timeout',
    });
  });
});

describe('GroupMeshCallRuntime', () => {
  beforeEach(() => {
    vi.useRealTimers();
    vi.resetAllMocks();
    vi.stubGlobal('MediaStream', MockMediaStream);
    vi.mocked(callingService.sendIceCandidate).mockResolvedValue({
      callId: 'call-1',
      senderPeerId: 'peer-local',
      targetPeerId: 'peer-alice',
      candidate: 'candidate:local',
      sdpMid: '0',
      sdpMlineIndex: 0,
      timestamp: 100,
      signature: [1],
    });
    vi.mocked(callingService.hangupCall).mockResolvedValue({
      callId: 'call-1',
      senderPeerId: 'peer-local',
      targetPeerId: 'peer-alice',
      reason: 'normal',
      timestamp: 100,
      signature: [1],
    });
  });

  function groupRuntimeWith(
    peerConnections: MockPeerConnection[],
    getUserMedia?: ReturnType<typeof vi.fn>,
  ) {
    const getUserMediaMock =
      getUserMedia ?? vi.fn().mockResolvedValue(new MockMediaStream([new MockTrack('audio')]));
    let index = 0;
    const updates: unknown[] = [];
    const runtime = new GroupMeshCallRuntime({
      iceServers: [],
      timeoutMs: 1_000,
      mediaDevices: { getUserMedia: getUserMediaMock },
      audioElementFactory: audioElement,
      peerConnectionFactory: () => peerConnections[index++] as unknown as RTCPeerConnection,
      onStateChange: (snapshot) => updates.push(snapshot),
    });
    return { runtime, getUserMedia: getUserMediaMock, updates };
  }

  it('enforces the ADR-0001 four participant mesh limit before opening peer connections', async () => {
    const { runtime, getUserMedia } = groupRuntimeWith([]);

    await expect(
      runtime.startOutgoingGroupCall(['peer-a', 'peer-b', 'peer-c', 'peer-d']),
    ).rejects.toThrow(`${GROUP_CALL_MAX_PARTICIPANTS} total participants`);

    expect(getUserMedia).not.toHaveBeenCalled();
    expect(callingService.startCall).not.toHaveBeenCalled();
  });

  it('starts one signed WebRTC leg per remote participant and reports roster state', async () => {
    const pcs = [new MockPeerConnection(), new MockPeerConnection()];
    const { runtime, getUserMedia } = groupRuntimeWith(pcs);
    vi.mocked(callingService.startCall).mockImplementation(async (calleePeerId, sdp) => ({
      callId: `call-${calleePeerId}`,
      callerPeerId: 'peer-local',
      calleePeerId,
      sdp,
      timestamp: 100,
      signature: [1],
    }));

    const snapshot = await runtime.startOutgoingGroupCall(['peer-a', 'peer-b']);

    expect(getUserMedia).toHaveBeenCalledTimes(1);
    expect(callingService.startCall).toHaveBeenCalledTimes(2);
    expect(callingService.startCall).toHaveBeenNthCalledWith(1, 'peer-a', 'v=0\r\noffer');
    expect(callingService.startCall).toHaveBeenNthCalledWith(2, 'peer-b', 'v=0\r\noffer');
    expect(snapshot).toMatchObject({ state: 'ringing', participantCount: 3, maxParticipants: 4 });
    expect(snapshot.participants.map((participant) => participant.peerId)).toEqual([
      'peer-a',
      'peer-b',
    ]);
    expect(snapshot.participants.every((participant) => participant.state === 'ringing')).toBe(
      true,
    );
  });

  it('removes a leaving participant and closes only that mesh leg', async () => {
    const pcs = [new MockPeerConnection(), new MockPeerConnection()];
    const { runtime } = groupRuntimeWith(pcs);
    vi.mocked(callingService.startCall).mockImplementation(async (calleePeerId, sdp) => ({
      callId: `call-${calleePeerId}`,
      callerPeerId: 'peer-local',
      calleePeerId,
      sdp,
      timestamp: 100,
      signature: [1],
    }));

    await runtime.startOutgoingGroupCall(['peer-a', 'peer-b']);
    runtime.handleParticipantLeft('peer-a');

    const snapshot = runtime.getSnapshot();
    expect(snapshot.participantCount).toBe(2);
    expect(snapshot.participants.map((participant) => participant.peerId)).toEqual(['peer-b']);
    expect(pcs[0].close).toHaveBeenCalledTimes(1);
    expect(pcs[1].close).not.toHaveBeenCalled();
  });

  it('queues an incoming room without media and fills deterministic mesh legs on accept', async () => {
    const pcs = [new MockPeerConnection(), new MockPeerConnection()];
    const { runtime, getUserMedia } = groupRuntimeWith(pcs);
    const membership = {
      roomId: 'room-1',
      creatorPeerId: 'peer-alice',
      senderPeerId: 'peer-alice',
      action: 'invite' as const,
      topology: 'relay_assisted_mesh_v1' as const,
      rosterVersion: 1,
      participants: ['peer-alice', 'peer-local', 'peer-zed'],
      mediaMode: 'video' as const,
      nonce: 'nonce-1',
      timestamp: 100,
      signature: [1],
    };
    vi.mocked(callingService.answerCall).mockResolvedValue({
      callId: 'call-1',
      callerPeerId: 'peer-alice',
      calleePeerId: 'peer-local',
      sdp: 'v=0\r\nanswer',
      timestamp: 100,
      signature: [1],
    });
    vi.mocked(callingService.startCall).mockImplementation(async (calleePeerId, sdp) => ({
      callId: `call-${calleePeerId}`,
      callerPeerId: 'peer-local',
      calleePeerId,
      sdp,
      timestamp: 100,
      signature: [1],
    }));

    const pending = runtime.prepareIncomingGroupCall(membership, 'peer-local');
    expect(pending.state).toBe('ringing');
    expect(getUserMedia).not.toHaveBeenCalled();

    const accepted = await runtime.acceptIncomingGroupCall(membership, [offerEnvelope()]);
    expect(callingService.answerCall).toHaveBeenCalledTimes(1);
    expect(callingService.startCall).toHaveBeenCalledWith('peer-zed', 'v=0\r\noffer');
    expect(accepted.participants.map((participant) => participant.peerId).sort()).toEqual([
      'peer-alice',
      'peer-zed',
    ]);
  });

  it('rejects malformed incoming rosters before requesting media', () => {
    const { runtime, getUserMedia } = groupRuntimeWith([]);
    const membership = {
      roomId: 'room-invalid',
      creatorPeerId: 'peer-alice',
      senderPeerId: 'peer-alice',
      action: 'invite' as const,
      topology: 'relay_assisted_mesh_v1' as const,
      rosterVersion: 1,
      participants: ['peer-local', 'peer-alice', 'peer-alice'],
      mediaMode: 'audio' as const,
      nonce: 'nonce-invalid',
      timestamp: 100,
      signature: [1],
    };

    expect(() => runtime.prepareIncomingGroupCall(membership, 'peer-local')).toThrow(
      'Invalid group-call membership roster',
    );
    expect(getUserMedia).not.toHaveBeenCalled();
  });

  it('isolates a failed incoming leg while another participant continues', async () => {
    const pcs = [new MockPeerConnection()];
    const getUserMedia = vi
      .fn()
      .mockRejectedValueOnce(new DOMException('microphone unavailable', 'NotFoundError'))
      .mockResolvedValueOnce(new MockMediaStream([new MockTrack('audio')]));
    const { runtime } = groupRuntimeWith(pcs, getUserMedia);
    const membership = {
      roomId: 'room-partial',
      creatorPeerId: 'peer-alice',
      senderPeerId: 'peer-alice',
      action: 'invite' as const,
      topology: 'relay_assisted_mesh_v1' as const,
      rosterVersion: 1,
      participants: ['peer-alice', 'peer-local', 'peer-zed'],
      mediaMode: 'audio' as const,
      nonce: 'nonce-partial',
      timestamp: 100,
      signature: [1],
    };
    vi.mocked(callingService.startCall).mockImplementation(async (calleePeerId, sdp) => ({
      callId: `call-${calleePeerId}`,
      callerPeerId: 'peer-local',
      calleePeerId,
      sdp,
      timestamp: 100,
      signature: [1],
    }));

    runtime.prepareIncomingGroupCall(membership, 'peer-local');
    const snapshot = await runtime.acceptIncomingGroupCall(membership, [offerEnvelope()]);

    expect(snapshot.state).toBe('degraded');
    expect(snapshot.participants.find(({ peerId }) => peerId === 'peer-alice')).toMatchObject({
      state: 'failed',
    });
    expect(snapshot.participants.find(({ peerId }) => peerId === 'peer-zed')).toMatchObject({
      state: 'ringing',
    });
  });

  it('isolates a failed participant while remaining mesh legs continue', async () => {
    const pcs = [new MockPeerConnection(), new MockPeerConnection()];
    const { runtime } = groupRuntimeWith(pcs);
    vi.mocked(callingService.startCall).mockImplementation(async (calleePeerId, sdp) => {
      if (calleePeerId === 'peer-b') throw new Error('peer-b unreachable');
      return {
        callId: `call-${calleePeerId}`,
        callerPeerId: 'peer-local',
        calleePeerId,
        sdp,
        timestamp: 100,
        signature: [1],
      };
    });

    const snapshot = await runtime.startOutgoingGroupCall(['peer-a', 'peer-b']);

    expect(snapshot.state).toBe('degraded');
    expect(
      snapshot.participants.find((participant) => participant.peerId === 'peer-a'),
    ).toMatchObject({ state: 'ringing' });
    expect(
      snapshot.participants.find((participant) => participant.peerId === 'peer-b'),
    ).toMatchObject({
      state: 'failed',
      error: 'Harbor could not start the call.',
    });
  });

  it('applies a propagated participant failure without collapsing healthy legs', async () => {
    const peerConnections = [new MockPeerConnection(), new MockPeerConnection()];
    const { runtime } = groupRuntimeWith(peerConnections);
    vi.mocked(callingService.startCall)
      .mockResolvedValueOnce({
        callId: 'call-alice',
        callerPeerId: 'peer-local',
        calleePeerId: 'peer-alice',
        sdp: 'v=0\r\noffer',
        timestamp: 100,
        signature: [1],
      })
      .mockResolvedValueOnce({
        callId: 'call-bob',
        callerPeerId: 'peer-local',
        calleePeerId: 'peer-bob',
        sdp: 'v=0\r\noffer',
        timestamp: 100,
        signature: [1],
      });
    await runtime.startOutgoingGroupCall(['peer-alice', 'peer-bob'], {
      localPeerId: 'peer-local',
    });

    await runtime.handleParticipantFailed('peer-bob', 'Peer media runtime is unavailable.');

    const snapshot = runtime.getSnapshot();
    expect(snapshot.state).toBe('degraded');
    expect(snapshot.participants.find((item) => item.peerId === 'peer-bob')).toMatchObject({
      state: 'failed',
      error: 'Peer media runtime is unavailable.',
    });
    expect(snapshot.participants.find((item) => item.peerId === 'peer-alice')?.state).toBe(
      'ringing',
    );
    expect(peerConnections[0].close).not.toHaveBeenCalled();
  });

  it('retries one terminal participant without replacing healthy mesh legs', async () => {
    const peerConnections = [
      new MockPeerConnection(),
      new MockPeerConnection(),
      new MockPeerConnection(),
    ];
    const { runtime } = groupRuntimeWith(peerConnections);
    vi.mocked(callingService.startCall).mockImplementation(async (peerId, sdp) => ({
      callId: `call-${peerId}`,
      callerPeerId: 'peer-local',
      calleePeerId: peerId,
      sdp,
      timestamp: 100,
      signature: [1],
    }));
    await runtime.startOutgoingGroupCall(['peer-alice', 'peer-bob'], {
      localPeerId: 'peer-local',
    });
    await runtime.handleParticipantFailed('peer-bob', 'Peer media runtime is unavailable.');

    const snapshot = await runtime.retryParticipant('peer-bob');

    expect(snapshot.participants.find((item) => item.peerId === 'peer-bob')?.state).toBe('ringing');
    expect(peerConnections[0].close).not.toHaveBeenCalled();
    expect(callingService.startCall).toHaveBeenCalledTimes(3);
  });

  it('updates per-participant camera and mute state across mesh legs', async () => {
    const pcs = [new MockPeerConnection(), new MockPeerConnection()];
    const capturedAudio = new MockTrack('audio');
    const capturedVideo = new MockTrack('video');
    const getUserMedia = vi
      .fn()
      .mockResolvedValueOnce(new MockMediaStream([capturedAudio, capturedVideo]));
    const { runtime } = groupRuntimeWith(pcs, getUserMedia);
    vi.mocked(callingService.startCall).mockImplementation(async (calleePeerId, sdp) => ({
      callId: `call-${calleePeerId}`,
      callerPeerId: 'peer-local',
      calleePeerId,
      sdp,
      timestamp: 100,
      signature: [1],
    }));

    await runtime.startOutgoingGroupCall(['peer-a', 'peer-b'], { video: true });
    await runtime.setLocalMuted(true);
    await runtime.setCameraEnabled(false);

    expect(getUserMedia).toHaveBeenCalledTimes(1);
    expect(
      pcs.flatMap((peerConnection) => peerConnection.senders.map(({ track }) => track.enabled)),
    ).toEqual([false, false, false, false]);
    expect(runtime.getSnapshot()).toMatchObject({ localMuted: true, localCameraEnabled: false });
  });
});
