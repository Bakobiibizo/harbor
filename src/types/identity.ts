/** Identity info returned from backend */
export interface IdentityInfo {
  peerId: string;
  publicKey: string; // base64 encoded
  x25519Public: string; // base64 encoded
  displayName: string;
  avatarHash: string | null;
  bio: string | null;
  createdAt: number;
  updatedAt: number;
}

/** Request to create a new identity */
export interface CreateIdentityRequest {
  displayName: string;
  passphrase: string;
  bio?: string;
}

/** Application state for identity */
export type IdentityState =
  | { status: 'loading' }
  | { status: 'no_identity' }
  | { status: 'locked'; identity: IdentityInfo }
  | { status: 'unlocked'; identity: IdentityInfo };
