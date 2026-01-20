import { invoke } from '@tauri-apps/api/core';
import type { AccountInfo } from '../types';

/** Accounts service - wraps Tauri commands for multi-user account management */
export const accountsService = {
  /** List all registered accounts */
  async listAccounts(): Promise<AccountInfo[]> {
    return invoke<AccountInfo[]>('list_accounts');
  },

  /** Get a specific account by ID */
  async getAccount(accountId: string): Promise<AccountInfo | null> {
    return invoke<AccountInfo | null>('get_account', { accountId });
  },

  /** Get the currently active account */
  async getActiveAccount(): Promise<AccountInfo | null> {
    return invoke<AccountInfo | null>('get_active_account');
  },

  /** Check if any accounts exist */
  async hasAccounts(): Promise<boolean> {
    return invoke<boolean>('has_accounts');
  },

  /** Set the active account (for switching between accounts) */
  async setActiveAccount(accountId: string): Promise<AccountInfo> {
    return invoke<AccountInfo>('set_active_account', { accountId });
  },

  /** Remove an account from the registry */
  async removeAccount(accountId: string, deleteData: boolean = false): Promise<void> {
    return invoke('remove_account', { accountId, deleteData });
  },

  /** Update account metadata in the registry */
  async updateAccountMetadata(
    accountId: string,
    displayName?: string,
    bio?: string | null,
    avatarHash?: string | null,
  ): Promise<AccountInfo> {
    return invoke<AccountInfo>('update_account_metadata', {
      accountId,
      displayName,
      bio,
      avatarHash,
    });
  },
};
