export const MAX_CONTACT_INVITE_INPUT_LENGTH = 16_384;
export const MAX_CONTACT_INVITE_PAYLOAD_LENGTH = 12_288;
export const MAX_CONTACT_INVITE_DECODED_BYTES = 8_192;

const WEB_INVITE_HOSTS = new Set(['social-harbor.com', 'www.social-harbor.com']);
const WEB_INVITE_PATH = '/add-friend/';
const BASE64URL_PATTERN = /^[A-Za-z0-9_-]+$/;
const KEY_PATTERN = /^[A-Za-z0-9+/]+={0,2}$/;

export interface ContactInviteBundle {
  multiaddr: string;
  displayName: string;
  publicKey: string;
  x25519Public: string;
  bio?: string;
  avatarHash?: string;
  peerId: string;
}

function inviteError(message: string): never {
  throw new Error(`Invalid contact invite: ${message}`);
}

function extractPayload(input: string): string {
  if (input.length > MAX_CONTACT_INVITE_INPUT_LENGTH) inviteError('the link is too large.');

  if (input.startsWith('harbor://add-friend/')) {
    return input.slice('harbor://add-friend/'.length);
  }
  if (input.startsWith('harbor://')) {
    return input.slice('harbor://'.length);
  }

  let url: URL;
  try {
    url = new URL(input);
  } catch {
    inviteError('paste a Harbor link from social-harbor.com.');
  }

  if (
    url.protocol !== 'https:' ||
    !WEB_INVITE_HOSTS.has(url.hostname) ||
    url.port ||
    url.username ||
    url.password ||
    !url.pathname.startsWith(WEB_INVITE_PATH) ||
    url.search ||
    url.hash
  ) {
    inviteError('only official social-harbor.com invite links are accepted.');
  }

  const payload = url.pathname.slice(WEB_INVITE_PATH.length);
  if (payload.endsWith('/')) return payload.slice(0, -1);
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
  if (binary.length > MAX_CONTACT_INVITE_DECODED_BYTES) inviteError('the payload is too large.');
  return Uint8Array.from(binary, (character) => character.charCodeAt(0));
}

function requiredString(
  value: unknown,
  label: string,
  maximumLength: number,
  pattern?: RegExp,
): string {
  if (
    typeof value !== 'string' ||
    value.length === 0 ||
    value.length > maximumLength ||
    (pattern && !pattern.test(value))
  ) {
    inviteError(`${label} is malformed.`);
  }
  return value;
}

/**
 * Accept an official web invite or native Harbor deep link and return the one
 * canonical string understood by the desktop backend.
 */
export function normalizeContactInvite(value: string): string {
  const input = value.trim();
  if (!input) inviteError('paste a contact link first.');
  const payload = extractPayload(input);
  // Fully validate before passing untrusted text across the command boundary.
  parseContactInvitePayload(payload);
  return `harbor://${payload}`;
}

export function parseContactInvite(value: string): ContactInviteBundle {
  const normalized = normalizeContactInvite(value);
  return parseContactInvitePayload(normalized.slice('harbor://'.length));
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
  const multiaddr = requiredString(record.multiaddr, 'network address', 2_048);
  const peerMatch = multiaddr.match(/\/p2p\/([^/]+)$/);
  if (!peerMatch || peerMatch[1].length > 256) inviteError('network address is malformed.');

  const displayNameValue = record.display_name ?? record.displayName;
  const publicKeyValue = record.public_key ?? record.publicKey;
  const x25519Value = record.x25519_public ?? record.x25519Public;
  const avatarValue = record.avatar_hash ?? record.avatarHash;
  const bioValue = record.bio;

  const bundle: ContactInviteBundle = {
    multiaddr,
    displayName: requiredString(displayNameValue, 'contact name', 128),
    publicKey: requiredString(publicKeyValue, 'public key', 512, KEY_PATTERN),
    x25519Public: requiredString(x25519Value, 'encryption key', 512, KEY_PATTERN),
    peerId: peerMatch[1],
  };
  if (bioValue != null) bundle.bio = requiredString(bioValue, 'bio', 2_048);
  if (avatarValue != null) bundle.avatarHash = requiredString(avatarValue, 'avatar', 512);
  return bundle;
}
