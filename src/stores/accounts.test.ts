import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useAccountsStore } from './accounts';
import { accountsService } from '../services';

vi.mock('../services', () => ({
  accountsService: {
    listAccounts: vi.fn(),
    getActiveAccount: vi.fn(),
    setActiveAccount: vi.fn(),
    removeAccount: vi.fn(),
  },
}));

const mockAccounts = [
  {
    accountId: 'acct-1',
    peerId: 'peer-1',
    displayName: 'User One',
    bio: 'First account',
    avatarHash: null,
    createdAt: 1700000000,
    lastActiveAt: 1700000100,
    isActive: true,
  },
  {
    accountId: 'acct-2',
    peerId: 'peer-2',
    displayName: 'User Two',
    bio: null,
    avatarHash: null,
    createdAt: 1700000200,
    lastActiveAt: 1700000200,
    isActive: false,
  },
];

describe('useAccountsStore', () => {
  beforeEach(() => {
    useAccountsStore.setState({
      accounts: [],
      activeAccount: null,
      loading: true,
      error: null,
    });
    vi.clearAllMocks();
  });

  describe('loadAccounts', () => {
    it('should load accounts and active account', async () => {
      vi.mocked(accountsService.listAccounts).mockResolvedValue(mockAccounts);
      vi.mocked(accountsService.getActiveAccount).mockResolvedValue(mockAccounts[0]);

      await useAccountsStore.getState().loadAccounts();

      const state = useAccountsStore.getState();
      expect(state.accounts).toEqual(mockAccounts);
      expect(state.activeAccount).toEqual(mockAccounts[0]);
      expect(state.loading).toBe(false);
      expect(state.error).toBeNull();
    });

    it('should handle load errors', async () => {
      vi.mocked(accountsService.listAccounts).mockRejectedValue(new Error('Load failed'));

      await useAccountsStore.getState().loadAccounts();

      const state = useAccountsStore.getState();
      expect(state.error).toBe('Load failed');
      expect(state.loading).toBe(false);
    });
  });

  describe('setActiveAccount', () => {
    it('should set the active account', async () => {
      vi.mocked(accountsService.setActiveAccount).mockResolvedValue(mockAccounts[1]);

      await useAccountsStore.getState().setActiveAccount('acct-2');

      expect(useAccountsStore.getState().activeAccount).toEqual(mockAccounts[1]);
    });

    it('should throw and set error on failure', async () => {
      vi.mocked(accountsService.setActiveAccount).mockRejectedValue(
        new Error('Switch failed'),
      );

      await expect(
        useAccountsStore.getState().setActiveAccount('acct-2'),
      ).rejects.toThrow('Switch failed');

      expect(useAccountsStore.getState().error).toBe('Switch failed');
    });
  });

  describe('removeAccount', () => {
    it('should remove account and reload list', async () => {
      vi.mocked(accountsService.removeAccount).mockResolvedValue(undefined);
      vi.mocked(accountsService.listAccounts).mockResolvedValue([mockAccounts[1]]);
      vi.mocked(accountsService.getActiveAccount).mockResolvedValue(mockAccounts[1]);

      await useAccountsStore.getState().removeAccount('acct-1');

      const state = useAccountsStore.getState();
      expect(state.accounts).toHaveLength(1);
      expect(state.activeAccount).toEqual(mockAccounts[1]);
    });

    it('should pass deleteData flag', async () => {
      vi.mocked(accountsService.removeAccount).mockResolvedValue(undefined);
      vi.mocked(accountsService.listAccounts).mockResolvedValue([]);
      vi.mocked(accountsService.getActiveAccount).mockResolvedValue(null);

      await useAccountsStore.getState().removeAccount('acct-1', true);

      expect(accountsService.removeAccount).toHaveBeenCalledWith('acct-1', true);
    });

    it('should throw and set error on failure', async () => {
      vi.mocked(accountsService.removeAccount).mockRejectedValue(
        new Error('Remove failed'),
      );

      await expect(
        useAccountsStore.getState().removeAccount('acct-1'),
      ).rejects.toThrow('Remove failed');

      expect(useAccountsStore.getState().error).toBe('Remove failed');
    });
  });

  describe('clearError', () => {
    it('should clear error state', () => {
      useAccountsStore.setState({ error: 'Some error' });

      useAccountsStore.getState().clearError();

      expect(useAccountsStore.getState().error).toBeNull();
    });
  });
});
