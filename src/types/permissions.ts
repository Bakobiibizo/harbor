/** Permission capability types */
export type Capability = "chat" | "wall_read" | "call";

/** Permission info */
export interface PermissionInfo {
  grantId: string;
  issuerPeerId: string;
  subjectPeerId: string;
  capability: string;
  issuedAt: number;
  expiresAt: number | null;
  isValid: boolean;
}

/** Result of granting a permission */
export interface GrantResult {
  grantId: string;
  capability: string;
  subjectPeerId: string;
  issuedAt: number;
  expiresAt: number | null;
}
