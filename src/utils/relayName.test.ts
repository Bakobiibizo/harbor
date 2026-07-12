import { describe, expect, it } from 'vitest';
import { presentRelayName } from './relayName';

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
      label: 'Alice',
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
