/** Account info stored in the accounts registry */
export interface AccountInfo {
  /** Unique identifier for the account */
  id: string;
  /** User's display name */
  displayName: string;
  /** Avatar hash if set */
  avatarHash: string | null;
  /** Short bio */
  bio: string | null;
  /** Peer ID for this account */
  peerId: string;
  /** When the account was created (timestamp) */
  createdAt: number;
  /** When the account was last accessed (timestamp) */
  lastAccessedAt: number | null;
  /** Path to the account's data directory */
  dataPath: string;
}
