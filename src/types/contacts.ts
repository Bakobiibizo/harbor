/** Contact information */
export interface Contact {
  id: number;
  peerId: string;
  publicKey: string; // base64 encoded
  x25519Public: string; // base64 encoded
  displayName: string;
  avatarHash: string | null;
  bio: string | null;
  isBlocked: boolean;
  trustLevel: number;
  lastSeenAt: number | null;
  addedAt: number;
  updatedAt: number;
}

/** Data needed to add a new contact */
export interface ContactData {
  peerId: string;
  publicKey: string; // base64 encoded
  x25519Public: string; // base64 encoded
  displayName: string;
  avatarHash?: string | null;
  bio?: string | null;
}
