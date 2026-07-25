import type { RelayNameClaim } from '../types';

export const MAX_CONTACT_INVITE_INPUT_LENGTH = 8_192;
export const MAX_CONTACT_INVITE_PAYLOAD_LENGTH = 6_144;
export const MAX_CONTACT_INVITE_DECODED_BYTES = 4_096;

const INVITE_VERSION = 1;
const NATIVE_PREFIX = 'harbor://contact/v1/';
const WEB_INVITE_HOSTS = new Set(['social-harbor.com', 'www.social-harbor.com']);
const WEB_INVITE_PATH = '/add-friend/v1/';
const WEB_INVITE_QUERY_PATH = '/add-friend';
const BASE64URL_PATTERN = /^[A-Za-z0-9_-]+$/;
const CANONICAL_32_BYTE_KEY_PATTERN = /^[A-Za-z0-9+/]{43}=$/;
const BASE58_ALPHABET = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';

export interface ContactInviteBundle {
  version: 1;
  peerId: string;
  multiaddr: string;
  displayName: string;
  publicKey: string;
  x25519Public: string;
  bio?: string;
  avatarHash?: string;
  relayNameClaim?: RelayNameClaim;
}

function inviteError(message: string): never {
  throw new Error(`Invalid contact invite: ${message}`);
}

function bytesToBinary(bytes: Uint8Array): string {
  let binary = '';
  bytes.forEach((byte) => (binary += String.fromCharCode(byte)));
  return binary;
}

function encodePayload(bytes: Uint8Array): string {
  return btoa(bytesToBinary(bytes)).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

function extractPayload(input: string): string {
  if (input.length > MAX_CONTACT_INVITE_INPUT_LENGTH) inviteError('the link is too large.');
  if (input.startsWith(NATIVE_PREFIX)) return input.slice(NATIVE_PREFIX.length);
  if (input.startsWith('harbor://')) inviteError('only canonical Harbor v1 invites are accepted.');

  let url: URL;
  try {
    url = new URL(input);
  } catch {
    inviteError('paste a canonical Harbor v1 invite.');
  }
  const isVersionedPath =
    url.pathname.startsWith(WEB_INVITE_PATH) && !url.search && !url.hash;
  const queryKeys = Array.from(url.searchParams.keys());
  const isQueryHandoff =
    url.pathname === WEB_INVITE_QUERY_PATH &&
    !url.hash &&
    queryKeys.length === 1 &&
    queryKeys[0] === 'c';
  if (
    url.protocol !== 'https:' ||
    !WEB_INVITE_HOSTS.has(url.hostname) ||
    url.port ||
    url.username ||
    url.password ||
    (!isVersionedPath && !isQueryHandoff)
  ) {
    inviteError('only official Harbor v1 invite links are accepted.');
  }
  const payload = isQueryHandoff
    ? url.searchParams.get('c') ?? ''
    : url.pathname.slice(WEB_INVITE_PATH.length);
  if (!payload || payload.includes('/')) inviteError('the invite URL is malformed.');
  return payload;
}

function decodePayload(payload: string): Uint8Array {
  if (!payload) inviteError('the invite payload is missing.');
  if (payload.length > MAX_CONTACT_INVITE_PAYLOAD_LENGTH) inviteError('the payload is too large.');
  if (!BASE64URL_PATTERN.test(payload)) inviteError('the payload encoding is malformed.');
  const standard = payload.replace(/-/g, '+').replace(/_/g, '/');
  const padded = standard + '='.repeat((4 - (standard.length % 4)) % 4);
  let binary: string;
  try {
    binary = atob(padded);
  } catch {
    inviteError('the payload encoding is malformed.');
  }
  const bytes = Uint8Array.from(binary, (character) => character.charCodeAt(0));
  if (bytes.length > MAX_CONTACT_INVITE_DECODED_BYTES) inviteError('the payload is too large.');
  if (encodePayload(bytes) !== payload) inviteError('the payload encoding is not canonical.');
  return bytes;
}

function requiredString(value: unknown, label: string, maximumLength: number): string {
  if (typeof value !== 'string' || value.length === 0 || value.length > maximumLength) {
    inviteError(`${label} is malformed.`);
  }
  return value;
}

function decodeCanonicalKey(value: unknown, label: string): Uint8Array {
  const encoded = requiredString(value, label, 64);
  if (!CANONICAL_32_BYTE_KEY_PATTERN.test(encoded)) inviteError(`${label} is malformed.`);
  let binary: string;
  try {
    binary = atob(encoded);
  } catch {
    inviteError(`${label} is malformed.`);
  }
  const bytes = Uint8Array.from(binary, (character) => character.charCodeAt(0));
  if (bytes.length !== 32 || btoa(binary) !== encoded) {
    inviteError(`${label} must be one canonical 32-byte key.`);
  }
  return bytes;
}

function integer(value: unknown, label: string, minimum?: number): number {
  if (!Number.isSafeInteger(value) || (minimum != null && (value as number) < minimum)) {
    inviteError(`${label} is malformed.`);
  }
  return value as number;
}

function byteArray(
  value: unknown,
  label: string,
  minimumLength: number,
  maximumLength = minimumLength,
): number[] {
  if (
    !Array.isArray(value) ||
    value.length < minimumLength ||
    value.length > maximumLength ||
    value.some((byte) => !Number.isInteger(byte) || byte < 0 || byte > 255)
  ) {
    inviteError(`${label} is malformed.`);
  }
  return value as number[];
}

function equalBytes(first: Uint8Array, second: number[]): boolean {
  return first.length === second.length && first.every((byte, index) => byte === second[index]);
}

function parseRelayNameClaim(
  value: unknown,
  peerId: string,
  publicKey: Uint8Array,
  x25519Public: Uint8Array,
): RelayNameClaim {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    inviteError('relay name claim is malformed.');
  }
  const claim = value as Record<string, unknown>;
  const claimKeys = new Set([
    'request',
    'userSignature',
    'status',
    'notBefore',
    'notAfter',
    'relayKeyId',
    'relaySignature',
  ]);
  if (Object.keys(claim).some((key) => !claimKeys.has(key))) {
    inviteError('relay name claim contains unsupported fields.');
  }
  if (!claim.request || typeof claim.request !== 'object' || Array.isArray(claim.request)) {
    inviteError('relay name claim request is malformed.');
  }
  const request = claim.request as Record<string, unknown>;
  const requestKeys = new Set([
    'domain',
    'version',
    'localName',
    'relay',
    'peerId',
    'ed25519PublicKey',
    'x25519PublicKey',
    'sequence',
    'issuedAt',
    'nonce',
  ]);
  if (Object.keys(request).some((key) => !requestKeys.has(key))) {
    inviteError('relay name claim request contains unsupported fields.');
  }

  const localName = requiredString(request.localName, 'relay local name', 32);
  const relay = requiredString(request.relay, 'relay namespace', 253);
  if (!/^@[a-z0-9](?:[a-z0-9-]*[a-z0-9])?@[a-z0-9.-]+$/.test(`@${localName}@${relay}`)) {
    inviteError('relay-qualified name is malformed.');
  }
  const claimPeerId = requiredString(request.peerId, 'relay claim peer ID', 128);
  const ed25519PublicKey = byteArray(request.ed25519PublicKey, 'relay claim public key', 32);
  const claimX25519Public = byteArray(request.x25519PublicKey, 'relay claim encryption key', 32);
  if (
    claimPeerId !== peerId ||
    !equalBytes(publicKey, ed25519PublicKey) ||
    !equalBytes(x25519Public, claimX25519Public)
  ) {
    inviteError('relay name claim does not match the invited identity.');
  }

  const notBefore = integer(claim.notBefore, 'relay claim start');
  const notAfter = integer(claim.notAfter, 'relay claim expiry');
  if (notAfter <= notBefore) inviteError('relay name claim validity window is malformed.');
  if (requiredString(claim.status, 'relay claim status', 32) !== 'active') {
    inviteError('relay name claim is not active.');
  }
  const domain = requiredString(request.domain, 'relay claim domain', 128);
  const version = integer(request.version, 'relay claim version', 1);
  if (domain !== 'harbor/name-claim-request/1' || version !== 1) {
    inviteError('relay name claim protocol is unsupported.');
  }

  return {
    request: {
      domain,
      version,
      localName,
      relay,
      peerId: claimPeerId,
      ed25519PublicKey,
      x25519PublicKey: claimX25519Public,
      sequence: integer(request.sequence, 'relay claim sequence', 1),
      issuedAt: integer(request.issuedAt, 'relay claim issue time'),
      nonce: byteArray(request.nonce, 'relay claim nonce', 16, 64),
    },
    userSignature: byteArray(claim.userSignature, 'relay claim user signature', 64),
    status: 'active',
    notBefore,
    notAfter,
    relayKeyId: requiredString(claim.relayKeyId, 'relay claim key ID', 256),
    relaySignature: byteArray(claim.relaySignature, 'relay claim relay signature', 64),
  };
}

function base58Encode(bytes: Uint8Array): string {
  const digits = [0];
  for (const byte of bytes) {
    let carry = byte;
    for (let index = 0; index < digits.length; index += 1) {
      const value = digits[index] * 256 + carry;
      digits[index] = value % 58;
      carry = Math.floor(value / 58);
    }
    while (carry > 0) {
      digits.push(carry % 58);
      carry = Math.floor(carry / 58);
    }
  }
  let output = '';
  for (let index = 0; index < bytes.length - 1 && bytes[index] === 0; index += 1) output += '1';
  for (let index = digits.length - 1; index >= 0; index -= 1) {
    output += BASE58_ALPHABET[digits[index]];
  }
  return output;
}

function deriveEd25519PeerId(publicKey: Uint8Array): string {
  // libp2p Ed25519 peer IDs are identity multihashes of the protobuf PublicKey.
  const bytes = new Uint8Array(2 + 4 + publicKey.length);
  bytes.set([0x00, 0x24, 0x08, 0x01, 0x12, 0x20]);
  bytes.set(publicKey, 6);
  return base58Encode(bytes);
}

/** Normalize the only accepted native/web invite envelope. */
export function normalizeContactInvite(value: string): string {
  const input = value.trim();
  if (!input) inviteError('paste a contact link first.');
  const payload = extractPayload(input);
  parseContactInvitePayload(payload);
  return `${NATIVE_PREFIX}${payload}`;
}

export function contactInviteToWebUrl(value: string, site = 'https://social-harbor.com'): string {
  const canonical = normalizeContactInvite(value);
  return `${site}/add-friend?c=${canonical.slice(NATIVE_PREFIX.length)}`;
}

export function parseContactInvite(value: string): ContactInviteBundle {
  const normalized = normalizeContactInvite(value);
  return parseContactInvitePayload(normalized.slice(NATIVE_PREFIX.length));
}

function parseContactInvitePayload(payload: string): ContactInviteBundle {
  const bytes = decodePayload(payload);
  let value: unknown;
  try {
    value = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes));
  } catch {
    inviteError('the payload data is malformed.');
  }
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    inviteError('the payload data is malformed.');
  }
  const record = value as Record<string, unknown>;
  const allowed = new Set([
    'version',
    'peerId',
    'multiaddr',
    'displayName',
    'publicKey',
    'x25519Public',
    'bio',
    'avatarHash',
    'relayNameClaim',
  ]);
  if (Object.keys(record).some((key) => !allowed.has(key))) {
    inviteError('the payload contains unsupported fields.');
  }
  if (record.version !== INVITE_VERSION) inviteError('the invite version is unsupported.');

  const peerId = requiredString(record.peerId, 'peer ID', 128);
  const multiaddr = requiredString(record.multiaddr, 'network address', 2_048);
  const addressPeerId = multiaddr.match(/^\/.+\/p2p\/([^/]+)$/)?.[1];
  if (addressPeerId !== peerId) inviteError('network address and peer ID do not match.');
  const publicKey = decodeCanonicalKey(record.publicKey, 'public key');
  if (deriveEd25519PeerId(publicKey) !== peerId) {
    inviteError('public key and peer ID do not match.');
  }
  const x25519Public = decodeCanonicalKey(record.x25519Public, 'encryption key');

  const bundle: ContactInviteBundle = {
    version: 1,
    peerId,
    multiaddr,
    displayName: requiredString(record.displayName, 'contact name', 128),
    publicKey: record.publicKey as string,
    x25519Public: record.x25519Public as string,
  };
  if (record.bio != null) bundle.bio = requiredString(record.bio, 'bio', 2_048);
  if (record.avatarHash != null) {
    bundle.avatarHash = requiredString(record.avatarHash, 'avatar', 512);
  }
  if (record.relayNameClaim != null) {
    bundle.relayNameClaim = parseRelayNameClaim(
      record.relayNameClaim,
      peerId,
      publicKey,
      x25519Public,
    );
  }
  return bundle;
}
