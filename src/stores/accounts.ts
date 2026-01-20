import { create } from 'zustand';
import type { AccountInfo } from '../types';
import { accountsService } from '../services';

interface AccountsStore {
  accounts: AccountInfo[];
  activeAccount: AccountInfo | null;
  loading: boolean;
  error: string | null;

  // Actions
  loadAccounts: () => Promise<void>;
  setActiveAccount: (accountId: string) => Promise<void>;
  removeAccount: (accountId: string, deleteData?: boolean) => Promise<void>;
  clearError: () => void;
}

export const useAccountsStore = create<AccountsStore>((set) => ({
  accounts: [],
  activeAccount: null,
  loading: true,
  error: null,

  loadAccounts: async () => {
    try {
      set({ loading: true, error: null });
      const [accounts, activeAccount] = await Promise.all([
        accountsService.listAccounts(),
        accountsService.getActiveAccount(),
      ]);
      set({ accounts, activeAccount, loading: false });
    } catch (err) {
      set({
        error: err instanceof Error ? err.message : String(err),
        loading: false,
      });
    }
  },

  setActiveAccount: async (accountId: string) => {
    try {
      set({ error: null });
      const activeAccount = await accountsService.setActiveAccount(accountId);
      set({ activeAccount });
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err) });
      throw err;
    }
  },

  removeAccount: async (accountId: string, deleteData = false) => {
    try {
      set({ error: null });
      await accountsService.removeAccount(accountId, deleteData);
      // Reload accounts list
      const accounts = await accountsService.listAccounts();
      const activeAccount = await accountsService.getActiveAccount();
      set({ accounts, activeAccount });
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err) });
      throw err;
    }
  },

  clearError: () => set({ error: null }),
}));
