export type GameSigningRequest =
  | {
      kind: 'auth';
      approvalId: string;
      accountId: string;
      audience: string;
      challengeId: string;
      expiresAt: number;
      requestId: string;
    }
  | {
      kind: 'package';
      approvalId: string;
      expiresAt: number;
      gameId: string;
      packageDigest: string;
      permissions: string[];
      requestId: string;
      title: string;
      versionId: string;
    };

export type GameSigningDelivery =
  | { status: 'delivered'; requestId: string }
  | { status: 'pending'; requestId: string; error: string };

export interface StoreApproval {
  algorithm: 'ed25519';
  payload: {
    approvedAt: number;
    archiveDigest: string;
    creatorPeerId: string;
    domain: 'neo-grounds.harbor-store.approval.v1';
    gameId: string;
    reviewerId: string;
    version: 1;
    versionId: string;
  };
  publicKey: string;
  signature: string;
}

export interface GamePackagePreview {
  archiveDigest: string;
  byteLength: number;
  creatorPeerId: string;
  gameId: string;
  packageDigest: string;
  permissions: string[];
  title: string;
  versionId: string;
}

export interface GameInstallation {
  archiveDigest: string;
  byteLength: number;
  creatorPeerId: string;
  gameId: string;
  installedAt: number;
  lastPlayedAt: number | null;
  packageDigest: string;
  permissions: string[];
  playCount: number;
  source: 'manual' | 'folder' | 'store';
  storeApproval: StoreApproval | null;
  title: string;
  versionId: string;
}

export interface GameDiscoveryResult {
  error: string | null;
  fileName: string;
  installation: GameInstallation | null;
}
