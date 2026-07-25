import { describe, expect, it } from 'vitest';
import {
  MAX_CONTACT_INVITE_INPUT_LENGTH,
  contactInviteToWebUrl,
  normalizeContactInvite,
  parseContactInvite,
} from './contactInvite';

const peerId = '12D3KooWEKyPsvgm7g8vyRA59466ph3t9VLxEUFzNqTXHJdfY2Wq';
const bundle = {
  version: 1,
  peerId,
  multiaddr: `/dns4/relay.social-harbor.com/tcp/443/wss/p2p/${peerId}`,
  displayName: 'José',
  publicKey: 'QwRr/kCSs+lJlOraFdzCDYqqB7ZY/TlU644O+4vcpd4=',
  x25519Public: 'CAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAg=',
  bio: 'Hello',
};

const relayNameClaim = {
  request: {
    domain: 'harbor/name-claim-request/1',
    version: 1,
    localName: 'jose',
    relay: 'harbor.social',
    peerId,
    ed25519PublicKey: Array.from(atob(bundle.publicKey), (character) => character.charCodeAt(0)),
    x25519PublicKey: Array.from(atob(bundle.x25519Public), (character) =>
      character.charCodeAt(0),
    ),
    sequence: 1,
    issuedAt: 1_800_000_000,
    nonce: Array(16).fill(7),
  },
  userSignature: Array(64).fill(8),
  status: 'active',
  notBefore: 1_800_000_000,
  notAfter: 1_900_000_000,
  relayKeyId: 'relay-key-1',
  relaySignature: Array(64).fill(9),
};

function payload(value: unknown = bundle): string {
  const bytes = new TextEncoder().encode(JSON.stringify(value));
  let binary = '';
  bytes.forEach((byte) => (binary += String.fromCharCode(byte)));
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

describe('canonical contact invite', () => {
  it('normalizes the native and official HTTPS v1 forms', () => {
    const encoded = payload();
    const canonical = `harbor://contact/v1/${encoded}`;
    expect(normalizeContactInvite(canonical)).toBe(canonical);
    expect(normalizeContactInvite(`https://social-harbor.com/add-friend/v1/${encoded}`)).toBe(
      canonical,
    );
    expect(normalizeContactInvite(`https://social-harbor.com/add-friend?c=${encoded}`)).toBe(
      canonical,
    );
    expect(contactInviteToWebUrl(canonical)).toBe(
      `https://social-harbor.com/add-friend?c=${encoded}`,
    );
  });

  it('validates UTF-8, exact keys, address, and Ed25519 key-to-peer binding', () => {
    expect(parseContactInvite(`harbor://contact/v1/${payload()}`)).toMatchObject({
      version: 1,
      displayName: 'José',
      peerId,
      bio: 'Hello',
    });
  });

  it('preserves a structurally bound relay name claim for authoritative verification', () => {
    expect(
      parseContactInvite(
        `harbor://contact/v1/${payload({ ...bundle, relayNameClaim })}`,
      ).relayNameClaim,
    ).toEqual(relayNameClaim);
  });

  it.each([
    'harbor://legacy',
    'harbor://contact/v1/not+base64',
    'http://social-harbor.com/add-friend/v1/abc',
    'https://evil.example/add-friend/v1/abc',
    'https://social-harbor.com.evil.example/add-friend/v1/abc',
    'https://social-harbor.com/add-friend/v1/abc?next=evil',
    'https://social-harbor.com/add-friend/v1/abc/extra',
    'https://social-harbor.com/add-friend?c=abc&next=evil',
  ])('rejects an untrusted, legacy, or malformed form: %s', (input) => {
    expect(() => normalizeContactInvite(input)).toThrow(/Invalid contact invite/);
  });

  it('rejects tampering, mismatches, double-base64 keys, and unknown fields', () => {
    const anotherPeer = '12D3KooWMEo4jDyz9hGAVEcZhiGkjRH4A73FXRpk8MwJkhnSqy7z';
    expect(() =>
      normalizeContactInvite(
        `harbor://contact/v1/${payload({
          ...bundle,
          peerId: anotherPeer,
          multiaddr: `/ip4/127.0.0.1/tcp/1/p2p/${anotherPeer}`,
        })}`,
      ),
    ).toThrow(/public key and peer ID do not match/);
    expect(() =>
      normalizeContactInvite(
        `harbor://contact/v1/${payload({ ...bundle, multiaddr: `${bundle.multiaddr}x` })}`,
      ),
    ).toThrow(/network address and peer ID do not match/);
    expect(() =>
      normalizeContactInvite(
        `harbor://contact/v1/${payload({
          ...bundle,
          publicKey: btoa(bundle.publicKey),
        })}`,
      ),
    ).toThrow(/public key/);
    expect(() =>
      normalizeContactInvite(
        `harbor://contact/v1/${payload({ ...bundle, display_name: 'alias' })}`,
      ),
    ).toThrow(/unsupported fields/);
    expect(() =>
      normalizeContactInvite(
        `harbor://contact/v1/${payload({
          ...bundle,
          relayNameClaim: {
            ...relayNameClaim,
            request: { ...relayNameClaim.request, peerId: anotherPeer },
          },
        })}`,
      ),
    ).toThrow(/does not match the invited identity/);
  });

  it('rejects oversized input before decoding', () => {
    expect(() => normalizeContactInvite('x'.repeat(MAX_CONTACT_INVITE_INPUT_LENGTH + 1))).toThrow(
      /too large/,
    );
  });
});
