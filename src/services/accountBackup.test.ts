import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { accountBackupService } from './accountBackup';

describe('accountBackupService', () => {
  beforeEach(() => vi.clearAllMocks());

  it('passes only the selected path and password to the export command', async () => {
    vi.mocked(invoke).mockResolvedValue({
      peerId: 'peer-1',
      path: '/safe/account.harbor-backup',
      createdAt: 10,
      bytesWritten: 256,
    });

    await accountBackupService.exportIdentity('/safe/account.harbor-backup', 'correct horse');

    expect(invoke).toHaveBeenCalledWith('export_identity_backup', {
      path: '/safe/account.harbor-backup',
      password: 'correct horse',
    });
  });

  it('passes only the selected path and password to the restore command', async () => {
    vi.mocked(invoke).mockResolvedValue({ account: {}, restartRequired: true });

    await accountBackupService.restoreIdentity('/safe/account.harbor-backup', 'correct horse');

    expect(invoke).toHaveBeenCalledWith('restore_identity_backup', {
      path: '/safe/account.harbor-backup',
      password: 'correct horse',
    });
  });

  it('requires the authenticated account command contract for deletion', async () => {
    vi.mocked(invoke).mockResolvedValue({ restartRequired: true, nextAccountId: null });

    await accountBackupService.deleteAccountProfile('account-1', 'correct horse');

    expect(invoke).toHaveBeenCalledWith('delete_account_profile', {
      accountId: 'account-1',
      password: 'correct horse',
    });
  });
});
