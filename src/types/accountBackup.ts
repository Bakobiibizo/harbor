import type { AccountInfo } from './accounts';

export interface IdentityBackupExportResult {
  peerId: string;
  path: string;
  createdAt: number;
  bytesWritten: number;
}

export interface IdentityBackupRestoreResult {
  account: AccountInfo;
  restartRequired: boolean;
}

export interface DeleteAccountProfileResult {
  restartRequired: boolean;
  nextAccountId: string | null;
}
