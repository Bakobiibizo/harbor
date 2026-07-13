import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  AudioCallRuntime,
  GROUP_CALL_MAX_PARTICIPANTS,
  GroupMeshCallRuntime,
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

class MockTrack {
  stopped = false;
  enabled = true;
  kind: 'audio' | 'video';

  constructor(kind: 'audio' | 'video' = 'audio') {
    this.kind = kind;
  }

  stop = vi.fn(() => {
    this.stopped = true;
  });
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

function answerEnvelope(): SignalingEnvelope {
  return {
    senderPeerId: 'peer-alice',
    recipientPeerId: 'peer-local',
    payload: {
      type: 'answer',
      payload: {
        callId: 'call-1',
        callerPeerId: 'peer-local',
        calleePeerId: 'peer-alice',
        sdp: 'v=0\r\nremote-answer',
        timestamp: 100,
        signature: [1],
      },
    },
  };
}

function runtimeWith(pc: MockPeerConnection, getUserMedia?: ReturnType<typeof vi.fn>) {
  const track = new MockTrack();
  const getUserMediaMock = getUserMedia ?? vi.fn().mockResolvedValue(new MockMediaStream([track]));
  return {
    track,
    runtime: new AudioCallRuntime({
      iceServers: [],
      timeoutMs: 1_000,
      mediaDevices: { getUserMedia: getUserMediaMock },
      audioElementFactory: audioElement,
      peerConnectionFactory: () => pc as unknown as RTCPeerConnection,
    }),
    getUserMedia: getUserMediaMock,
  };
}

describe('AudioCallRuntime', () => {
  beforeEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
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
    pc.emit('icecandidate', {
      candidate: { candidate: 'candidate:local', sdpMid: '0', sdpMLineIndex: 0 },
    });

    expect(pc.remoteDescription).toEqual({ type: 'answer', sdp: 'v=0\r\nremote-answer' });
    expect(callingService.sendIceCandidate).toHaveBeenCalledWith(
      'call-1',
      'peer-alice',
      'candidate:local',
      '0',
      0,
    );
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

  it('distinguishes a missing platform media API from missing hardware', async () => {
    const pc = new MockPeerConnection();
    const unavailable = new DOMException('The media capture API is not available.', 'NotSupportedError');
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
});

describe('GroupMeshCallRuntime', () => {
  beforeEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
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

    expect(getUserMedia).toHaveBeenCalledTimes(2);
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
      callId: 'call-1', callerPeerId: 'peer-alice', calleePeerId: 'peer-local',
      sdp: 'v=0\r\nanswer', timestamp: 100, signature: [1],
    });
    vi.mocked(callingService.startCall).mockImplementation(async (calleePeerId, sdp) => ({
      callId: `call-${calleePeerId}`, callerPeerId: 'peer-local', calleePeerId,
      sdp, timestamp: 100, signature: [1],
    }));

    const pending = runtime.prepareIncomingGroupCall(membership, 'peer-local');
    expect(pending.state).toBe('ringing');
    expect(getUserMedia).not.toHaveBeenCalled();

    const accepted = await runtime.acceptIncomingGroupCall(membership, [offerEnvelope()]);
    expect(callingService.answerCall).toHaveBeenCalledTimes(1);
    expect(callingService.startCall).toHaveBeenCalledWith('peer-zed', 'v=0\r\noffer');
    expect(accepted.participants.map((participant) => participant.peerId).sort()).toEqual([
      'peer-alice', 'peer-zed',
    ]);
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

  it('updates per-participant camera and mute state across mesh legs', async () => {
    const pcs = [new MockPeerConnection(), new MockPeerConnection()];
    const audioA = new MockTrack('audio');
    const videoA = new MockTrack('video');
    const audioB = new MockTrack('audio');
    const videoB = new MockTrack('video');
    const getUserMedia = vi
      .fn()
      .mockResolvedValueOnce(new MockMediaStream([audioA, videoA]))
      .mockResolvedValueOnce(new MockMediaStream([audioB, videoB]));
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

    expect(audioA.enabled).toBe(false);
    expect(audioB.enabled).toBe(false);
    expect(videoA.enabled).toBe(false);
    expect(videoB.enabled).toBe(false);
    expect(runtime.getSnapshot()).toMatchObject({ localMuted: true, localCameraEnabled: false });
  });
});
