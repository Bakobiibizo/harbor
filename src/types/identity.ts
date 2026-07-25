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

export type IdentityInitializationFailureSource =
  | 'identityDatabase'
  | 'identityCorruption'
  | 'accountRegistry';

export interface IdentityInitializationError {
  code: string;
  message: string;
  details?: string;
  recovery?: string;
}

/** Authoritative startup state returned by the desktop backend. */
export type IdentityInitializationResult =
  | { status: 'absent' }
  | { status: 'locked'; identity: IdentityInfo }
  | { status: 'unlocked'; identity: IdentityInfo }
  | {
      status: 'recoverableError';
      source: IdentityInitializationFailureSource;
      error: IdentityInitializationError;
    }
  | {
      status: 'fatalError';
      source: IdentityInitializationFailureSource;
      error: IdentityInitializationError;
    };

/** Application state for identity */
export type IdentityState =
  | { status: 'loading' }
  | IdentityInitializationResult
  | {
      status: 'recoverableError';
      source: 'ipc' | 'profileStorage';
      error: IdentityInitializationError;
    };
