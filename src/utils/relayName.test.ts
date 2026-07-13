import { describe, expect, it } from 'vitest';
import type { IdentityInfo } from '../types';
import {
  presentRelayName,
  safeIdentityLabel,
  safePeerLabel,
  UNVERIFIED_HARBOR_USER,
} from './relayName';

const claim = {
  request: {
    domain: 'harbor/name-claim-request/1',
    version: 1,
    localName: 'alice',
    relay: 'relay.example',
    peerId: 'peer',
    ed25519PublicKey: [],
    x25519PublicKey: [],
    sequence: 1,
    issuedAt: 1,
    nonce: [],
  },
  userSignature: [],
  status: 'active',
  notBefore: 1,
  notAfter: 200,
  relayKeyId: 'r',
  relaySignature: [],
};

describe('relay name presentation', () => {
  it('never presents a legacy label as verified', () =>
    expect(presentRelayName(null, 'Alice', true, 100)).toEqual({
      label: UNVERIFIED_HARBOR_USER,
      qualifiedName: null,
      trust: 'unverified',
    }));
  it('shows the full qualified name when verified', () =>
    expect(presentRelayName(claim, 'ignored', true, 100).label).toBe('@alice@relay.example'));
  it('distinguishes expired and untrusted claims', () => {
    expect(presentRelayName(claim, '', true, 201).trust).toBe('expired');
    expect(presentRelayName(claim, '', false, 100).trust).toBe('untrusted');
  });
});

describe('verified peer labels', () => {
  it('uses a verified qualified claim when present', () =>
    expect(safePeerLabel('12D3peer', '@alice@relay.test')).toBe('@alice@relay.test'));
  it('rejects malformed labels without exposing the peer key or accepting an alias', () => {
    expect(safePeerLabel('12D3peer-secret', 'Alice')).toBe(UNVERIFIED_HARBOR_USER);
    expect(safePeerLabel('12D3peer-secret')).toBe(UNVERIFIED_HARBOR_USER);
    expect(safePeerLabel('12D3peer-secret')).not.toContain('12D3');
  });

  it('does not present an expired local claim as a verified identity', () => {
    const identity = {
      peerId: '12D3peer-secret',
      displayName: 'Saved alias',
      relayNameClaim: claim,
      relayNameVerified: true,
    } as IdentityInfo;

    expect(safeIdentityLabel(identity)).toBe(UNVERIFIED_HARBOR_USER);
    expect(safeIdentityLabel(identity)).not.toContain('12D3');
  });
});
