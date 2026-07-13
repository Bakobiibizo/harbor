/** Identity info returned from backend */
export interface IdentityInfo {
  peerId: string;
  publicKey: string; // base64 encoded
  x25519Public: string; // base64 encoded
  displayName: string;
  relayNameClaim?: RelayNameClaim | null;
  relayNameVerified?: boolean;
  avatarHash: string | null;
  bio: string | null;
  passphraseHint: string | null;
  createdAt: number;
  updatedAt: number;
}

/** Request to create a new identity */
export interface CreateIdentityRequest {
  displayName: string;
  relayName?: string;
  relayNamespace?: string;
  passphrase: string;
  bio?: string;
  passphraseHint?: string;
}

export type RelayNameTrust = 'verified' | 'expired' | 'untrusted' | 'unverified';
export interface RelayNameClaim {
  request: {
    domain: string;
    version: number;
    localName: string;
    relay: string;
    peerId: string;
    ed25519PublicKey: number[];
    x25519PublicKey: number[];
    sequence: number;
    issuedAt: number;
    nonce: number[];
  };
  userSignature: number[];
  status: string;
  notBefore: number;
  notAfter: number;
  relayKeyId: string;
  relaySignature: number[];
}
export interface RegisterRelayNameRequest {
  name: string;
  namespace: string;
}
export interface RelayNamePresentation {
  label: string;
  qualifiedName: string | null;
  trust: RelayNameTrust;
}
export function qualifiedRelayName(
  claim: RelayNameClaim | { name: string; namespace: string },
): string {
  return 'request' in claim
    ? `@${claim.request.localName}@${claim.request.relay}`
    : `@${claim.name}@${claim.namespace}`;
}

/** Application state for identity */
export type IdentityState =
  | { status: 'loading' }
  | { status: 'no_identity' }
  | { status: 'locked'; identity: IdentityInfo }
  | { status: 'unlocked'; identity: IdentityInfo };
