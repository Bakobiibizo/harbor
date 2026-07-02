import { beforeEach, describe, expect, it, vi } from 'vitest';
import { AudioCallRuntime } from './callingRuntime';
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
  stop = vi.fn();
}

class MockMediaStream {
  private tracks: MockTrack[];

  constructor(tracks: MockTrack[] = []) {
    this.tracks = tracks;
  }

  getAudioTracks() {
    return this.tracks;
  }

  getTracks() {
    return this.tracks;
  }

  addTrack(track: MockTrack) {
    this.tracks.push(track);
  }
}

class MockPeerConnection {
  localDescription: RTCSessionDescriptionInit | null = null;
  remoteDescription: RTCSessionDescriptionInit | null = null;
  iceGatheringState: RTCIceGatheringState = 'new';
  iceConnectionState: RTCIceConnectionState = 'new';
  connectionState: RTCPeerConnectionState = 'new';
  addTrack = vi.fn();
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
    return { type: 'offer', sdp: 'v=0\r\noffer-from-alice' };
  }

  async createAnswer(): Promise<RTCSessionDescriptionInit> {
    return { type: 'answer', sdp: 'v=0\r\nanswer-from-bob' };
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

  connect() {
    this.connectionState = 'connected';
    this.iceConnectionState = 'connected';
    this.emit('connectionstatechange');
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

function runtimeWith(peerConnection: MockPeerConnection): AudioCallRuntime {
  return new AudioCallRuntime({
    iceServers: [],
    timeoutMs: 5_000,
    mediaDevices: {
      getUserMedia: vi.fn().mockResolvedValue(new MockMediaStream([new MockTrack()])),
    },
    audioElementFactory: audioElement,
    peerConnectionFactory: () => peerConnection as unknown as RTCPeerConnection,
  });
}

function envelopeFromOffer(): SignalingEnvelope {
  return {
    senderPeerId: 'peer-alice',
    recipientPeerId: 'peer-bob',
    payload: {
      type: 'offer',
      payload: {
        callId: 'call-e2e-1',
        callerPeerId: 'peer-alice',
        calleePeerId: 'peer-bob',
        sdp: 'v=0\r\noffer-from-alice',
        timestamp: 100,
        signature: [1, 2, 3],
      },
    },
  };
}

function envelopeFromAnswer(): SignalingEnvelope {
  return {
    senderPeerId: 'peer-bob',
    recipientPeerId: 'peer-alice',
    payload: {
      type: 'answer',
      payload: {
        callId: 'call-e2e-1',
        callerPeerId: 'peer-alice',
        calleePeerId: 'peer-bob',
        sdp: 'v=0\r\nanswer-from-bob',
        timestamp: 101,
        signature: [4, 5, 6],
      },
    },
  };
}

function iceEnvelope(
  senderPeerId: string,
  recipientPeerId: string,
  candidate: string,
): SignalingEnvelope {
  return {
    senderPeerId,
    recipientPeerId,
    payload: {
      type: 'ice',
      payload: {
        callId: 'call-e2e-1',
        senderPeerId,
        candidate,
        sdpMid: '0',
        sdpMlineIndex: 0,
        timestamp: 102,
        signature: [7, 8, 9],
      },
    },
  };
}

describe('voice call end-to-end runtime regression', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.stubGlobal('MediaStream', MockMediaStream);

    vi.mocked(callingService.startCall).mockResolvedValue({
      callId: 'call-e2e-1',
      callerPeerId: 'peer-alice',
      calleePeerId: 'peer-bob',
      sdp: 'v=0\r\noffer-from-alice',
      timestamp: 100,
      signature: [1, 2, 3],
    });
    vi.mocked(callingService.answerCall).mockResolvedValue({
      callId: 'call-e2e-1',
      callerPeerId: 'peer-alice',
      calleePeerId: 'peer-bob',
      sdp: 'v=0\r\nanswer-from-bob',
      timestamp: 101,
      signature: [4, 5, 6],
    });
    vi.mocked(callingService.sendIceCandidate).mockResolvedValue({
      callId: 'call-e2e-1',
      senderPeerId: 'peer-local',
      targetPeerId: 'peer-remote',
      candidate: 'candidate:local',
      sdpMid: '0',
      sdpMlineIndex: 0,
      timestamp: 102,
      signature: [7, 8, 9],
    });
    vi.mocked(callingService.hangupCall).mockResolvedValue({
      callId: 'call-e2e-1',
      senderPeerId: 'peer-alice',
      targetPeerId: 'peer-bob',
      reason: 'normal',
      timestamp: 103,
      signature: [10, 11, 12],
    });
  });

  it('drives two isolated audio runtimes through offer, answer, ICE, connected, and hangup states', async () => {
    const alicePc = new MockPeerConnection();
    const bobPc = new MockPeerConnection();
    const alice = runtimeWith(alicePc);
    const bob = runtimeWith(bobPc);

    await alice.startOutgoingCall('peer-bob');
    await bob.handleSignalingEnvelope(envelopeFromOffer());
    await bob.acceptIncomingCall(envelopeFromOffer());
    await alice.handleSignalingEnvelope(envelopeFromAnswer());

    alicePc.emit('icecandidate', {
      candidate: { candidate: 'candidate:alice', sdpMid: '0', sdpMLineIndex: 0 },
    });
    bobPc.emit('icecandidate', {
      candidate: { candidate: 'candidate:bob', sdpMid: '0', sdpMLineIndex: 0 },
    });
    await bob.handleSignalingEnvelope(iceEnvelope('peer-alice', 'peer-bob', 'candidate:alice'));
    await alice.handleSignalingEnvelope(iceEnvelope('peer-bob', 'peer-alice', 'candidate:bob'));

    alicePc.connect();
    bobPc.connect();
    await alice.hangup('normal');

    expect(callingService.startCall).toHaveBeenCalledWith('peer-bob', 'v=0\r\noffer-from-alice');
    expect(callingService.answerCall).toHaveBeenCalledWith(
      'call-e2e-1',
      'peer-alice',
      'v=0\r\nanswer-from-bob',
    );
    expect(callingService.sendIceCandidate).toHaveBeenCalledTimes(2);
    expect(bobPc.remoteDescription).toEqual({ type: 'offer', sdp: 'v=0\r\noffer-from-alice' });
    expect(alicePc.remoteDescription).toEqual({ type: 'answer', sdp: 'v=0\r\nanswer-from-bob' });
    expect(alicePc.addIceCandidate).toHaveBeenCalledWith({
      candidate: 'candidate:bob',
      sdpMid: '0',
      sdpMLineIndex: 0,
    });
    expect(bobPc.addIceCandidate).toHaveBeenCalledWith({
      candidate: 'candidate:alice',
      sdpMid: '0',
      sdpMLineIndex: 0,
    });
    expect(bob.getSnapshot()).toMatchObject({ state: 'connected', callId: 'call-e2e-1' });
    expect(alice.getSnapshot()).toMatchObject({ state: 'ended', terminalReason: 'normal' });
    expect(callingService.hangupCall).toHaveBeenCalledWith('call-e2e-1', 'peer-bob', 'normal');
  });
});
