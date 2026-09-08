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
