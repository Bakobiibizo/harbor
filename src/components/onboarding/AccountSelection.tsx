import { useState } from 'react';
import { Button } from '../common';
import { useAccountsStore } from '../../stores';
import { HarborIcon, UserPlusIcon, TrashIcon, LockIcon } from '../icons';
import type { AccountInfo } from '../../types';
import toast from 'react-hot-toast';

interface AccountSelectionProps {
  onSelectAccount: (account: AccountInfo) => void;
  onCreateAccount: () => void;
}

export function AccountSelection({ onSelectAccount, onCreateAccount }: AccountSelectionProps) {
  const { accounts, removeAccount } = useAccountsStore();
  const [selectedAccountId, setSelectedAccountId] = useState<string | null>(null);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState<string | null>(null);
  const [deleteData, setDeleteData] = useState(false);

  const handleLogin = (account: AccountInfo) => {
    onSelectAccount(account);
  };

  const handleDelete = async (accountId: string) => {
    try {
      await removeAccount(accountId, deleteData);
      toast.success('Account removed');
      setShowDeleteConfirm(null);
      setDeleteData(false);
    } catch (error) {
      toast.error(`Failed to remove account: ${error}`);
    }
  };

  const getInitials = (name: string) => {
    const parts = name.trim().split(/\s+/).filter(Boolean);

    if (parts.length === 0) return '?';

    return parts
      .slice(0, 2)
      .map((p) => p[0]?.toUpperCase() ?? '')
      .join('');
  };

  const formatDate = (timestamp: number) => {
    return new Date(timestamp * 1000).toLocaleDateString(undefined, {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
    });
  };

  return (
    <div
      className="min-h-screen flex"
      style={{
        background:
          'linear-gradient(135deg, hsl(220 91% 8%) 0%, hsl(262 60% 12%) 50%, hsl(220 91% 8%) 100%)',
      }}
    >
      {/* Left side - Branding */}
      <div className="hidden lg:flex flex-1 items-center justify-center p-12">
        <div className="max-w-md">
          {/* Logo */}
          <div className="flex items-center gap-4 mb-8">
            <div
              className="w-14 h-14 rounded-2xl flex items-center justify-center"
              style={{
                background:
                  'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
                boxShadow: '0 8px 32px hsl(var(--harbor-primary) / 0.4)',
              }}
            >
              <HarborIcon className="w-8 h-8 text-white" />
            </div>
            <div>
              <h1
                className="text-2xl font-bold"
                style={{ color: 'hsl(var(--harbor-text-primary))' }}
              >
                Harbor
              </h1>
              <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                Decentralized Chat
              </p>
            </div>
          </div>

          {/* Features */}
          <div className="space-y-4">
            <p className="text-lg" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
              Welcome back! Select an account to continue, or create a new one.
            </p>
            <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
              Each account has its own identity, contacts, and messages.
            </p>
          </div>
        </div>
      </div>

      {/* Right side - Account List */}
      <div className="flex-1 flex items-center justify-center p-6 lg:p-12">
        <div className="w-full max-w-md">
          <div
            className="rounded-2xl p-6"
            style={{
              background: 'hsl(var(--harbor-bg-elevated))',
              border: '1px solid hsl(var(--harbor-border-subtle))',
              boxShadow: '0 25px 50px -12px rgba(0, 0, 0, 0.5)',
            }}
          >
            {/* Mobile logo */}
            <div className="lg:hidden flex items-center gap-3 mb-6">
              <div
                className="w-10 h-10 rounded-xl flex items-center justify-center"
                style={{
                  background:
                    'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
                }}
              >
                <HarborIcon className="w-6 h-6 text-white" />
              </div>
              <span
                className="text-lg font-bold"
                style={{ color: 'hsl(var(--harbor-text-primary))' }}
              >
                Harbor
              </span>
            </div>

            {/* Header */}
            <div className="mb-6">
              <h2
                className="text-xl font-bold mb-1"
                style={{ color: 'hsl(var(--harbor-text-primary))' }}
              >
                Choose Account
              </h2>
              <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                {accounts.length} account{accounts.length !== 1 ? 's' : ''} available
              </p>
            </div>

            {/* Account List */}
            <div className="space-y-3 mb-6 max-h-80 overflow-y-auto">
              {accounts.map((account) => (
                <div
                  key={account.id}
                  className={`p-4 rounded-xl cursor-pointer transition-all duration-200 ${
                    selectedAccountId === account.id ? 'ring-2' : ''
                  }`}
                  style={{
                    background:
                      selectedAccountId === account.id
                        ? 'hsl(var(--harbor-primary) / 0.1)'
                        : 'hsl(var(--harbor-surface-1))',
                    border:
                      selectedAccountId === account.id
                        ? '1px solid hsl(var(--harbor-primary))'
                        : '1px solid hsl(var(--harbor-border-subtle))',
                  }}
                  onClick={() => setSelectedAccountId(account.id)}
                >
                  <div className="flex items-center gap-4">
                    {/* Avatar */}
                    <div
                      className="w-12 h-12 rounded-full flex items-center justify-center text-white font-semibold flex-shrink-0"
                      style={{
                        background:
                          'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
                      }}
                    >
                      {account.avatarHash ? (
                        <img
                          src={`/media/${account.avatarHash}`}
                          alt=""
                          className="w-full h-full rounded-full object-cover"
                        />
                      ) : (
                        getInitials(account.displayName)
                      )}
                    </div>

                    {/* Info */}
                    <div className="flex-1 min-w-0">
                      <p
                        className="font-semibold truncate"
                        style={{ color: 'hsl(var(--harbor-text-primary))' }}
                      >
                        {account.displayName}
                      </p>
                      <p
                        className="text-xs truncate font-mono"
                        style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                      >
                        {account.peerId.slice(0, 12)}...
                      </p>
                      {account.lastAccessedAt && (
                        <p
                          className="text-xs mt-1"
                          style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                        >
                          Last active: {formatDate(account.lastAccessedAt)}
                        </p>
                      )}
                    </div>

                    {/* Actions */}
                    <div className="flex items-center gap-2">
                      {selectedAccountId === account.id && (
                        <>
                          <Button
                            size="sm"
                            onClick={(e) => {
                              e.stopPropagation();
                              handleLogin(account);
                            }}
                          >
                            <LockIcon className="w-4 h-4 mr-1" />
                            Login
                          </Button>
                          <button
                            className="p-2 rounded-lg transition-colors duration-200 hover:bg-red-500/10"
                            style={{ color: 'hsl(var(--harbor-error))' }}
                            onClick={(e) => {
                              e.stopPropagation();
                              setShowDeleteConfirm(account.id);
                            }}
                            title="Delete account"
                          >
                            <TrashIcon className="w-4 h-4" />
                          </button>
                        </>
                      )}
                    </div>
                  </div>
                </div>
              ))}
            </div>

            {/* Create New Account Button */}
            <Button variant="secondary" className="w-full" onClick={onCreateAccount}>
              <UserPlusIcon className="w-5 h-5 mr-2" />
              Create New Account
            </Button>
          </div>
        </div>
      </div>

      {/* Delete Confirmation Modal */}
      {showDeleteConfirm && (
        <div
          className="fixed inset-0 flex items-center justify-center z-50 p-4"
          style={{ background: 'rgba(0, 0, 0, 0.6)', backdropFilter: 'blur(4px)' }}
          onClick={() => setShowDeleteConfirm(null)}
        >
          <div
            className="rounded-2xl p-6 w-full max-w-sm"
            style={{
              background: 'hsl(var(--harbor-bg-elevated))',
              border: '1px solid hsl(var(--harbor-border-subtle))',
            }}
            onClick={(e) => e.stopPropagation()}
          >
            <h3
              className="text-lg font-bold mb-2"
              style={{ color: 'hsl(var(--harbor-text-primary))' }}
            >
              Delete Account?
            </h3>
            <p className="text-sm mb-4" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
              This will remove the account from the list. You can optionally delete all account data
              permanently.
            </p>

            <label
              className="flex items-center gap-3 mb-4 p-3 rounded-lg cursor-pointer"
              style={{ background: 'hsl(var(--harbor-surface-1))' }}
            >
              <input
                type="checkbox"
                checked={deleteData}
                onChange={(e) => setDeleteData(e.target.checked)}
                className="w-4 h-4 rounded"
              />
              <div>
                <p
                  className="text-sm font-medium"
                  style={{ color: 'hsl(var(--harbor-text-primary))' }}
                >
                  Delete all account data
                </p>
                <p className="text-xs" style={{ color: 'hsl(var(--harbor-error))' }}>
                  This cannot be undone
                </p>
              </div>
            </label>

            <div className="flex gap-3">
              <Button
                variant="secondary"
                className="flex-1"
                onClick={() => setShowDeleteConfirm(null)}
              >
                Cancel
              </Button>
              <Button
                variant="danger"
                className="flex-1"
                onClick={() => handleDelete(showDeleteConfirm)}
              >
                Delete
              </Button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
