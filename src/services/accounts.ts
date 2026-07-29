import type { AccountInfo } from '../types';
import { invokeCommand } from './command';

/** Accounts service - wraps Tauri commands for multi-user account management */
export const accountsService = {
  /** List all registered accounts */
  async listAccounts(): Promise<AccountInfo[]> {
    return invokeCommand('list_accounts');
  },

  /** Get a specific account by ID */
  async getAccount(accountId: string): Promise<AccountInfo | null> {
    return invokeCommand('get_account', { accountId });
  },

  /** Get the currently active account */
  async getActiveAccount(): Promise<AccountInfo | null> {
    return invokeCommand('get_active_account');
  },

  /** Check if any accounts exist */
  async hasAccounts(): Promise<boolean> {
    return invokeCommand('has_accounts');
  },

  /** Set the active account (for switching between accounts) */
  async setActiveAccount(accountId: string): Promise<AccountInfo> {
    return invokeCommand('set_active_account', { accountId });
  },

  /** Remove an account from the registry */
  async removeAccount(accountId: string, deleteData: boolean = false): Promise<void> {
    return invokeCommand('remove_account', { accountId, deleteData });
  },

  /** Update account metadata in the registry */
  async updateAccountMetadata(
    accountId: string,
    displayName?: string,
    bio?: string | null,
    avatarHash?: string | null,
  ): Promise<AccountInfo> {
    return invokeCommand('update_account_metadata', {
      accountId,
      displayName,
      bio,
      avatarHash,
    });
  },
};
