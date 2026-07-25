import type {
  DeleteAccountProfileResult,
  IdentityBackupExportResult,
  IdentityBackupRestoreResult,
} from '../types';
import { invokeCommand } from './command';

/** Native identity backup operations. Backup contents remain opaque to the renderer. */
export const accountBackupService = {
  exportIdentity(path: string, password: string): Promise<IdentityBackupExportResult> {
    return invokeCommand('export_identity_backup', { path, password });
  },

  restoreIdentity(path: string, password: string): Promise<IdentityBackupRestoreResult> {
    return invokeCommand('restore_identity_backup', { path, password });
  },

  deleteAccountProfile(accountId: string, password: string): Promise<DeleteAccountProfileResult> {
    return invokeCommand('delete_account_profile', { accountId, password });
  },
};
