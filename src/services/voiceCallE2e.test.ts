import { beforeEach, describe, expect, it, vi } from 'vitest';
import { AudioCallRuntime } from './callingRuntime';
import { callingService } from './calling';
import type { IceServerConfig, SignalingEnvelope } from '../types';

vi.mock('./calling', () => ({
  callingService: {
    startCall: vi.fn(),
    answerCall: vi.fn(),
    sendIceCandidate: vi.fn(),
    hangupCall: vi.fn(),
  },
}));

class SyntheticTrack {
  enabled = true;
  stopped = false;

  constructor(readonly kind: 'audio' | 'video') {}

  stop = vi.fn(() => {
    this.stopped = true;
  });
}

class SyntheticMediaStream {
  constructor(private tracks: SyntheticTrack[] = []) {}

  getTracks() {
    return this.tracks;
  }

  getAudioTracks() {
    return this.tracks.filter((track) => track.kind === 'audio');
  }

  getVideoTracks() {
    return this.tracks.filter((track) => track.kind === 'video');
  }

  addTrack(track: SyntheticTrack) {
    this.tracks.push(track);
  }

  removeTrack(track: SyntheticTrack) {
    this.tracks = this.tracks.filter((candidate) => candidate !== track);
  }
}

class SyntheticPeerConnection {
  localDescription: RTCSessionDescriptionInit | null = null;
  remoteDescription: RTCSessionDescriptionInit | null = null;
  iceGatheringState: RTCIceGatheringState = 'new';
  iceConnectionState: RTCIceConnectionState = 'new';
  connectionState: RTCPeerConnectionState = 'new';
  addIceCandidate = vi.fn(async () => undefined);
  close = vi.fn();
  private listeners = new Map<string, Array<(event: any) => void>>();
  private tracks: SyntheticTrack[] = [];
  private senders: Array<{
    track: SyntheticTrack;
    replaceTrack: (replacement: SyntheticTrack | null) => Promise<void>;
  }> = [];

  constructor(
    readonly profile: string,
    readonly configuration: RTCConfiguration,
  ) {}

  addEventListener(type: string, listener: (event: any) => void) {
    this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener]);
  }

  removeEventListener(type: string, listener: (event: any) => void) {
    this.listeners.set(
      type,
      (this.listeners.get(type) ?? []).filter((candidate) => candidate !== listener),
    );
  }

  addTrack(track: SyntheticTrack) {
    this.tracks.push(track);
    const sender = {
      track,
      replaceTrack: async (replacement: SyntheticTrack | null) => {
        if (replacement) sender.track = replacement;
      },
    };
    this.senders.push(sender);
    return sender;
  }

  getSenders() {
    return this.senders;
  }

  async createOffer(options?: RTCOfferOptions): Promise<RTCSessionDescriptionInit> {
    return { type: 'offer', sdp: this.sdp('offer', Boolean(options?.offerToReceiveVideo)) };
  }

  async createAnswer(): Promise<RTCSessionDescriptionInit> {
    return {
      type: 'answer',
      sdp: this.sdp('answer', Boolean(this.remoteDescription?.sdp?.includes('m=video'))),
    };
  }

  async setLocalDescription(description: RTCSessionDescriptionInit) {
    this.localDescription = description;
  }

  async setRemoteDescription(description: RTCSessionDescriptionInit) {
    this.remoteDescription = description;
  }

  emit(type: string, event: any = {}) {
    for (const listener of this.listeners.get(type) ?? []) listener(event);
  }

  connect() {
    this.connectionState = 'connected';
    this.iceConnectionState = 'connected';
    this.emit('connectionstatechange');
  }

  private sdp(kind: 'offer' | 'answer', wantsVideo: boolean): string {
    const hasVideo = wantsVideo && this.tracks.some((track) => track.kind === 'video');
    return [
      'v=0',
      `s=synthetic-${this.profile}-${kind}`,
      'm=audio 9 UDP/TLS/RTP/SAVPF 111',
      ...(hasVideo ? ['m=video 9 UDP/TLS/RTP/SAVPF 96'] : []),
    ].join('\r\n');
  }
}

interface SyntheticProfile {
  peerId: string;
  runtime: AudioCallRuntime;
  pc: SyntheticPeerConnection;
  tracks: SyntheticTrack[];
}

const turnServer: IceServerConfig = {
  id: 'turn-test',
  urls: ['turn:turn.example.test:3478'],
  username: 'synthetic-user',
  credential: 'volatile-turn-secret',
  credentialPersistence: 'session',
};

function audioElement(): HTMLAudioElement {
  return {
    autoplay: false,
    srcObject: null,
    play: vi.fn(async () => undefined),
    pause: vi.fn(),
  } as unknown as HTMLAudioElement;
}

function profile(peerId: string, video: boolean): SyntheticProfile {
  const tracks = [new SyntheticTrack('audio'), ...(video ? [new SyntheticTrack('video')] : [])];
  let pc: SyntheticPeerConnection | null = null;
  const runtime = new AudioCallRuntime({
    iceServers: [turnServer],
    timeoutMs: 5_000,
    mediaDevices: {
      getUserMedia: vi.fn(async (constraints: MediaStreamConstraints) => {
        const requested = constraints.video
          ? tracks
          : tracks.filter((track) => track.kind === 'audio');
        return new SyntheticMediaStream(requested) as unknown as MediaStream;
      }),
    },
    audioElementFactory: audioElement,
    peerConnectionFactory: (configuration) => {
      pc = new SyntheticPeerConnection(peerId, configuration);
      return pc as unknown as RTCPeerConnection;
    },
  });
  // Media capture creates the peer connection lazily. The accessor below is
  // replaced after start/accept before a caller consumes it.
  return {
    peerId,
    runtime,
    get pc() {
      if (!pc) throw new Error(`Peer connection for ${peerId} has not been created.`);
      return pc;
    },
    tracks,
  };
}

function offerEnvelope(
  callId: string,
  caller: string,
  callee: string,
  sdp: string,
): SignalingEnvelope {
  return {
    senderPeerId: caller,
    recipientPeerId: callee,
    payload: {
      type: 'offer',
      payload: {
        callId,
        callerPeerId: caller,
        calleePeerId: callee,
        sdp,
        timestamp: 100,
        signature: [1, 2, 3],
      },
    },
  };
}

function answerEnvelope(
  callId: string,
  caller: string,
  callee: string,
  sdp: string,
): SignalingEnvelope {
  return {
    senderPeerId: callee,
    recipientPeerId: caller,
    payload: {
      type: 'answer',
      payload: {
        callId,
        callerPeerId: caller,
        calleePeerId: callee,
        sdp,
        timestamp: 101,
        signature: [4, 5, 6],
      },
    },
  };
}

function iceEnvelope(
  callId: string,
  sender: string,
  recipient: string,
  candidate: string,
): SignalingEnvelope {
  return {
    senderPeerId: sender,
    recipientPeerId: recipient,
    payload: {
      type: 'ice',
      payload: {
        callId,
        senderPeerId: sender,
        candidate,
        sdpMid: '0',
        sdpMlineIndex: 0,
        timestamp: 102,
        signature: [7, 8, 9],
      },
    },
  };
}

async function connectProfiles(caller: SyntheticProfile, callee: SyntheticProfile, video: boolean) {
  const callId = `call-${caller.peerId}-${callee.peerId}`;
  vi.mocked(callingService.startCall).mockImplementationOnce(async (calleePeerId, sdp) => ({
    callId,
    callerPeerId: caller.peerId,
    calleePeerId,
    sdp,
    timestamp: 100,
    signature: [1],
  }));
  vi.mocked(callingService.answerCall).mockImplementationOnce(
    async (_callId, callerPeerId, sdp) => ({
      callId,
      callerPeerId,
      calleePeerId: callee.peerId,
      sdp,
      timestamp: 101,
      signature: [2],
    }),
  );

  await caller.runtime.startOutgoingCall(callee.peerId, { video });
  const offer = offerEnvelope(
    callId,
    caller.peerId,
    callee.peerId,
    caller.pc.localDescription!.sdp!,
  );
  await callee.runtime.handleSignalingEnvelope(offer);
  await callee.runtime.acceptIncomingCall(offer);
  const answer = answerEnvelope(
    callId,
    caller.peerId,
    callee.peerId,
    callee.pc.localDescription!.sdp!,
  );
  await caller.runtime.handleSignalingEnvelope(answer);

  caller.pc.emit('icecandidate', {
    candidate: { candidate: `candidate:${caller.peerId}`, sdpMid: '0', sdpMLineIndex: 0 },
  });
  callee.pc.emit('icecandidate', {
    candidate: { candidate: `candidate:${callee.peerId}`, sdpMid: '0', sdpMLineIndex: 0 },
  });
  await callee.runtime.handleSignalingEnvelope(
    iceEnvelope(callId, caller.peerId, callee.peerId, `candidate:${caller.peerId}`),
  );
  await caller.runtime.handleSignalingEnvelope(
    iceEnvelope(callId, callee.peerId, caller.peerId, `candidate:${callee.peerId}`),
  );
  caller.pc.connect();
  callee.pc.connect();
  return callId;
}

describe('deterministic isolated-profile calling harness', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.stubGlobal('MediaStream', SyntheticMediaStream);
    vi.mocked(callingService.sendIceCandidate).mockResolvedValue({
      callId: 'synthetic',
      senderPeerId: 'sender',
      targetPeerId: 'target',
      candidate: 'candidate:synthetic',
      sdpMid: '0',
      sdpMlineIndex: 0,
      timestamp: 102,
      signature: [3],
    });
    vi.mocked(callingService.hangupCall).mockImplementation(
      async (callId, targetPeerId, reason) => ({
        callId,
        senderPeerId: 'sender',
        targetPeerId,
        reason: reason ?? 'normal',
        timestamp: 103,
        signature: [4],
      }),
    );
  });

  it.each([
    ['alice-to-bob', 'peer-alice', 'peer-bob'],
    ['bob-to-alice', 'peer-bob', 'peer-alice'],
  ])(
    'connects audio in both directions with offer, answer, ICE, mute, and hangup (%s)',
    async (_, from, to) => {
      const caller = profile(from, false);
      const callee = profile(to, false);
      const callId = await connectProfiles(caller, callee, false);

      caller.runtime.setMicrophoneMuted(true);
      expect(caller.tracks[0].enabled).toBe(false);
      caller.runtime.setMicrophoneMuted(false);
      expect(caller.tracks[0].enabled).toBe(true);
      expect(caller.runtime.getSnapshot().state).toBe('connected');
      expect(callee.runtime.getSnapshot().state).toBe('connected');
      expect(caller.pc.addIceCandidate).toHaveBeenCalledTimes(1);
      expect(callee.pc.addIceCandidate).toHaveBeenCalledTimes(1);

      await caller.runtime.hangup();
      await callee.runtime.handleSignalingEnvelope({
        senderPeerId: from,
        recipientPeerId: to,
        payload: {
          type: 'hangup',
          payload: { callId, senderPeerId: from, reason: 'normal', timestamp: 103, signature: [4] },
        },
      });
      expect(caller.runtime.getSnapshot().state).toBe('ended');
      expect(callee.runtime.getSnapshot()).toMatchObject({
        state: 'ended',
        terminalReason: 'remote_hangup',
      });
    },
  );

  it.each([
    ['alice-to-bob', 'peer-alice', 'peer-bob'],
    ['bob-to-alice', 'peer-bob', 'peer-alice'],
  ])(
    'negotiates audio and video in both directions and toggles the camera (%s)',
    async (_, from, to) => {
      const caller = profile(from, true);
      const callee = profile(to, true);
      await connectProfiles(caller, callee, true);

      expect(caller.pc.localDescription?.sdp).toContain('m=video');
      expect(callee.pc.localDescription?.sdp).toContain('m=video');
      await caller.runtime.setCameraEnabled(false);
      expect(caller.tracks.find((track) => track.kind === 'video')?.enabled).toBe(false);
      await caller.runtime.setCameraEnabled(true);
      expect(caller.tracks.find((track) => track.kind === 'video')?.enabled).toBe(true);
    },
  );

  it('handles reject without media capture and ignores stale or wrong-peer signaling', async () => {
    const caller = profile('peer-alice', false);
    const callee = profile('peer-bob', false);
    vi.mocked(callingService.startCall).mockResolvedValueOnce({
      callId: 'call-reject',
      callerPeerId: 'peer-alice',
      calleePeerId: 'peer-bob',
      sdp: 'v=0\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111',
      timestamp: 100,
      signature: [1],
    });
    await caller.runtime.startOutgoingCall(callee.peerId);
    const offer = offerEnvelope(
      'call-reject',
      'peer-alice',
      'peer-bob',
      caller.pc.localDescription!.sdp!,
    );
    await callee.runtime.handleSignalingEnvelope(offer);
    expect(() => callee.pc).toThrow('has not been created');

    await caller.runtime.handleSignalingEnvelope(
      iceEnvelope('stale-call', 'peer-mallory', 'peer-alice', 'candidate:private-address'),
    );
    expect(caller.pc.addIceCandidate).not.toHaveBeenCalled();
    await caller.runtime.handleSignalingEnvelope({
      senderPeerId: 'peer-bob',
      recipientPeerId: 'peer-alice',
      payload: {
        type: 'decline',
        payload: {
          callId: 'call-reject',
          senderPeerId: 'peer-bob',
          reason: 'declined',
          timestamp: 101,
          signature: [2],
        },
      },
    });
    expect(caller.runtime.getSnapshot()).toMatchObject({
      state: 'ended',
      terminalReason: 'remote_hangup',
    });
  });

  it('passes TURN only to the volatile peer configuration and relaunches without call material', async () => {
    const alice = profile('peer-alice', false);
    const bob = profile('peer-bob', false);
    await connectProfiles(alice, bob, false);
    expect(alice.pc.configuration.iceServers).toEqual([
      { urls: turnServer.urls, username: turnServer.username, credential: turnServer.credential },
    ]);

    const serializedSnapshot = JSON.stringify(alice.runtime.getSnapshot());
    expect(serializedSnapshot).not.toContain('volatile-turn-secret');
    expect(serializedSnapshot).not.toContain('candidate:peer-bob');
    expect(serializedSnapshot).not.toContain('synthetic-peer-alice-offer');
    alice.runtime.dispose();
    bob.runtime.dispose();
    expect(alice.tracks.every((track) => track.stopped)).toBe(true);
    expect(alice.runtime.getSnapshot()).toMatchObject({
      state: 'idle',
      callId: null,
      peerId: null,
      localPeerId: null,
      direction: null,
      ice: null,
    });

    const relaunched = profile('peer-alice', false);
    expect(relaunched.runtime.getSnapshot()).toMatchObject({
      state: 'idle',
      callId: null,
      peerId: null,
    });
  });
});
