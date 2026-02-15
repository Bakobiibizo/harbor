import { describe, it, expect, vi, beforeEach } from 'vitest';
import { accountsService } from './accounts';
import { invoke } from '@tauri-apps/api/core';

describe('accountsService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('listAccounts', () => {
    it('should invoke list_accounts', async () => {
      vi.mocked(invoke).mockResolvedValue([]);

      const result = await accountsService.listAccounts();

      expect(invoke).toHaveBeenCalledWith('list_accounts');
      expect(result).toEqual([]);
    });
  });

  describe('getAccount', () => {
    it('should invoke get_account with accountId', async () => {
      vi.mocked(invoke).mockResolvedValue(null);

      const result = await accountsService.getAccount('acct-1');

      expect(invoke).toHaveBeenCalledWith('get_account', { accountId: 'acct-1' });
      expect(result).toBeNull();
    });
  });

  describe('getActiveAccount', () => {
    it('should invoke get_active_account', async () => {
      vi.mocked(invoke).mockResolvedValue(null);

      const result = await accountsService.getActiveAccount();

      expect(invoke).toHaveBeenCalledWith('get_active_account');
      expect(result).toBeNull();
    });
  });

  describe('hasAccounts', () => {
    it('should invoke has_accounts', async () => {
      vi.mocked(invoke).mockResolvedValue(true);

      const result = await accountsService.hasAccounts();

      expect(invoke).toHaveBeenCalledWith('has_accounts');
      expect(result).toBe(true);
    });
  });

  describe('setActiveAccount', () => {
    it('should invoke set_active_account', async () => {
      const mockAccount = {
        accountId: 'acct-1',
        peerId: 'peer-1',
        displayName: 'User',
      };
      vi.mocked(invoke).mockResolvedValue(mockAccount);

      const result = await accountsService.setActiveAccount('acct-1');

      expect(invoke).toHaveBeenCalledWith('set_active_account', { accountId: 'acct-1' });
      expect(result).toEqual(mockAccount);
    });
  });

  describe('removeAccount', () => {
    it('should invoke remove_account with default deleteData=false', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);

      await accountsService.removeAccount('acct-1');

      expect(invoke).toHaveBeenCalledWith('remove_account', {
        accountId: 'acct-1',
        deleteData: false,
      });
    });

    it('should invoke remove_account with deleteData=true', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);

      await accountsService.removeAccount('acct-1', true);

      expect(invoke).toHaveBeenCalledWith('remove_account', {
        accountId: 'acct-1',
        deleteData: true,
      });
    });
  });

  describe('updateAccountMetadata', () => {
    it('should invoke update_account_metadata with all params', async () => {
      vi.mocked(invoke).mockResolvedValue({});

      await accountsService.updateAccountMetadata('acct-1', 'New Name', 'New bio', 'hash123');

      expect(invoke).toHaveBeenCalledWith('update_account_metadata', {
        accountId: 'acct-1',
        displayName: 'New Name',
        bio: 'New bio',
        avatarHash: 'hash123',
      });
    });

    it('should invoke update_account_metadata with optional params as undefined', async () => {
      vi.mocked(invoke).mockResolvedValue({});

      await accountsService.updateAccountMetadata('acct-1');

      expect(invoke).toHaveBeenCalledWith('update_account_metadata', {
        accountId: 'acct-1',
        displayName: undefined,
        bio: undefined,
        avatarHash: undefined,
      });
    });
  });
});
