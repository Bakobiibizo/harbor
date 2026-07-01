import { describe, expect, it, vi } from 'vitest';
import type { IceServerConfig } from '../types';
import {
  buildRtcConfiguration,
  createCallPeerConnection,
  describeIceFailure,
  parseIceServerUrls,
  redactIceServer,
  stripSessionCredentialsForPersistence,
  validateIceServerInput,
} from './callingIce';

class FakePeerConnection {
  iceGatheringState: RTCIceGatheringState = 'new';
  iceConnectionState: RTCIceConnectionState = 'new';
  connectionState: RTCPeerConnectionState = 'new';
  close = vi.fn();

  private listeners = new Map<string, Set<EventListener>>();

  addEventListener(type: string, listener: EventListener): void {
    const listeners = this.listeners.get(type) ?? new Set<EventListener>();
    listeners.add(listener);
    this.listeners.set(type, listeners);
  }

  removeEventListener(type: string, listener: EventListener): void {
    this.listeners.get(type)?.delete(listener);
  }

  emit(type: string): void {
    this.listeners.get(type)?.forEach((listener) => listener(new Event(type)));
  }
}

function turnServer(overrides: Partial<IceServerConfig> = {}): IceServerConfig {
  return {
    id: 'turn-main',
    urls: ['turn:turn.example.test:3478?transport=udp'],
    username: 'operator',
    credential: 'super-secret',
    credentialPersistence: 'session',
    ...overrides,
  };
}

describe('calling ICE configuration helpers', () => {
  it('parses whitespace and comma separated ICE server URLs', () => {
    expect(
      parseIceServerUrls(
        'stun:one.example.test:3478, turn:two.example.test:3478\nturns:three.example.test:5349',
      ),
    ).toEqual([
      'stun:one.example.test:3478',
      'turn:two.example.test:3478',
      'turns:three.example.test:5349',
    ]);
  });

  it('validates STUN entries without credentials', () => {
    const result = validateIceServerInput({ urls: 'stun:stun.example.test:3478' });

    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.server.urls).toEqual(['stun:stun.example.test:3478']);
      expect(result.server.credential).toBeUndefined();
      expect(result.server.credentialPersistence).toBeUndefined();
    }
  });

  it('validates TURN entries with default session-only credential persistence', () => {
    const result = validateIceServerInput({
      urls: 'turn:turn.example.test:3478?transport=tcp',
      username: 'alice',
      credential: 'secret',
    });

    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.server.username).toBe('alice');
      expect(result.server.credential).toBe('secret');
      expect(result.server.credentialPersistence).toBe('session');
    }
  });

  it('rejects unsupported URL schemes, embedded credentials, duplicates, and incomplete TURN entries', () => {
    expect(validateIceServerInput({ urls: 'https://turn.example.test' }).ok).toBe(false);
    expect(validateIceServerInput({ urls: 'turn:alice:secret@turn.example.test:3478' }).ok).toBe(
      false,
    );
    expect(
      validateIceServerInput({ urls: 'turn:turn.example.test:3478', username: 'alice' }).ok,
    ).toBe(false);

    const existing = [turnServer({ urls: ['stun:existing.example.test:3478'] })];
    expect(validateIceServerInput({ urls: 'stun:existing.example.test:3478' }, existing).ok).toBe(
      false,
    );
  });

  it('redacts credentials and strips session-only TURN secrets before persistence', () => {
    const server = turnServer();

    expect(redactIceServer(server)).toMatchObject({
      username: 'operator',
      hasCredential: true,
      redactedCredential: '••••••••',
    });

    expect(stripSessionCredentialsForPersistence(server)).toEqual({
      id: 'turn-main',
      urls: ['turn:turn.example.test:3478?transport=udp'],
      username: 'operator',
      credentialPersistence: 'session',
    });

    expect(
      stripSessionCredentialsForPersistence(
        turnServer({ credentialPersistence: 'device', credential: 'persist-me' }),
      ).credential,
    ).toBe('persist-me');
  });

  it('keeps LAN/direct calling enabled by default when no TURN server is configured', () => {
    const config = buildRtcConfiguration([]);

    expect(config).toEqual({ iceServers: [], iceTransportPolicy: 'all' });
  });

  it('converts configured STUN and TURN entries into RTCPeerConnection configuration', () => {
    const config = buildRtcConfiguration([
      { id: 'stun', urls: ['stun:stun.example.test:3478'] },
      turnServer({ credentialPersistence: 'device' }),
    ]);

    expect(config.iceServers).toEqual([
      { urls: ['stun:stun.example.test:3478'], username: undefined, credential: undefined },
      {
        urls: ['turn:turn.example.test:3478?transport=udp'],
        username: 'operator',
        credential: 'super-secret',
      },
    ]);
  });

  it('skips persisted TURN entries when session-only credentials are unavailable', () => {
    const config = buildRtcConfiguration([turnServer({ credential: undefined })]);

    expect(config.iceServers).toEqual([]);
    expect(describeIceFailure([turnServer({ credential: undefined })]).code).toBe(
      'turn-credentials-missing',
    );
  });

  it('surfaces actionable errors for strict NAT and relay-only failure modes', () => {
    const strictNat = describeIceFailure([]);
    const relayOnly = describeIceFailure([], 'relay');

    expect(strictNat.code).toBe('strict-nat-no-turn');
    expect(strictNat.message).toContain('TURN');
    expect(relayOnly.code).toBe('relay-only-without-turn');
    expect(relayOnly.message).toContain('libp2p relays');
  });

  it('creates a WebRTC runtime that consumes ICE servers and reports state changes', () => {
    const fake = new FakePeerConnection();
    const states: string[] = [];
    const factory = vi.fn((configuration: RTCConfiguration) => {
      expect(configuration.iceServers).toEqual([
        {
          urls: ['turn:turn.example.test:3478?transport=udp'],
          username: 'operator',
          credential: 'super-secret',
        },
      ]);
      return fake as unknown as RTCPeerConnection;
    });

    const runtime = createCallPeerConnection({
      iceServers: [turnServer()],
      peerConnectionFactory: factory,
      onStateChange: (state) => {
        states.push(
          `${state.iceGatheringState}/${state.iceConnectionState}/${state.connectionState}`,
        );
      },
    });

    expect(factory).toHaveBeenCalledTimes(1);
    expect(runtime.configuration.iceTransportPolicy).toBe('all');
    expect(runtime.getState().iceGatheringState).toBe('new');

    fake.iceGatheringState = 'gathering';
    fake.emit('icegatheringstatechange');
    expect(runtime.getState().iceGatheringState).toBe('gathering');

    fake.iceConnectionState = 'failed';
    fake.connectionState = 'failed';
    fake.emit('iceconnectionstatechange');
    expect(runtime.getState().error?.code).toBe('ice-failed');
    expect(states).toContain('gathering/failed/failed');

    runtime.close();
    expect(fake.close).toHaveBeenCalledTimes(1);
  });

  it('reports relay-only without TURN from the runtime failure path', () => {
    const fake = new FakePeerConnection();
    const runtime = createCallPeerConnection({
      iceServers: [],
      iceTransportPolicy: 'relay',
      peerConnectionFactory: () => fake as unknown as RTCPeerConnection,
    });

    fake.iceConnectionState = 'failed';
    fake.connectionState = 'failed';
    fake.emit('connectionstatechange');

    expect(runtime.getState().error?.code).toBe('relay-only-without-turn');
  });
});
