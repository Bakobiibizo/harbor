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

export interface GameRuntimeBundle {
  assetManifest: unknown;
  assets: Record<string, number[]>;
  gameId: string;
  manifest: NeoGroundsRuntimeManifest;
  versionId: string;
  wasmBytes: number[];
}

export interface NeoGroundsRuntimeManifest {
  compatibility: {
    hostIntegration: 'worker-host-canvas-proxy';
    packageFormat: 'neo-grounds-runtime-package';
    renderer: 'canvas2d-command-buffer';
    runtimeAbi: 'neo-grounds-wasm-component-v1';
    schemaVersion: 1;
  };
  metadata: { gameId: string; title: string; versionId: string };
  platformApi: { bindingMode: 'declared-permissions-only'; permissions: string[] };
  wasmModule: { exports: string[]; imports: string[] };
}

export interface GameDiscoveryPreview {
  error: string | null;
  fileName: string;
  filePath: string;
  package: GamePackagePreview | null;
}

export type StoreGamePreview = GamePackagePreview;

export interface GameDiscoveryResult {
  error: string | null;
  fileName: string;
  installation: GameInstallation | null;
}
