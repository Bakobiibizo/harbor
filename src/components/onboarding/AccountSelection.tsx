import { useState } from 'react';
import { Button } from '../common';
import { useAccountsStore } from '../../stores';
import { HarborIcon, UserPlusIcon, TrashIcon } from '../icons';
import type { AccountInfo } from '../../types';
import toast from 'react-hot-toast';
import { getInitials } from '../../utils/formatting';

interface AccountSelectionProps {
  onSelectAccount: (account: AccountInfo) => void;
  onCreateAccount: () => void;
}

export function AccountSelection({ onSelectAccount, onCreateAccount }: AccountSelectionProps) {
  const { accounts, removeAccount } = useAccountsStore();
  const [deletingAccountId, setDeletingAccountId] = useState<string | null>(null);

  const handleLogin = (account: AccountInfo) => {
    onSelectAccount(account);
  };

  const handleDeleteClick = (e: React.MouseEvent, account: AccountInfo) => {
    e.stopPropagation();

    // Show a warning toast with a confirmation button
    toast(
      (t) => (
        <div>
          <p style={{ fontWeight: 600, marginBottom: 4 }}>
            Delete "{account.displayName}"?
          </p>
          <p style={{ fontSize: 13, opacity: 0.85, marginBottom: 12 }}>
            Deleting this account is irreversible. All data will be permanently lost.
          </p>
          <div style={{ display: 'flex', gap: 8 }}>
            <button
              onClick={() => toast.dismiss(t.id)}
              style={{
                flex: 1,
                padding: '6px 12px',
                borderRadius: 8,
                border: '1px solid hsl(222, 30%, 22%)',
                background: 'transparent',
                color: 'hsl(220, 14%, 96%)',
                fontSize: 13,
                cursor: 'pointer',
              }}
            >
              Cancel
            </button>
            <button
              onClick={() => {
                toast.dismiss(t.id);
                confirmDelete(account.id);
              }}
              style={{
                flex: 1,
                padding: '6px 12px',
                borderRadius: 8,
                border: 'none',
                background: 'hsl(0, 84%, 60%)',
                color: 'white',
                fontSize: 13,
                fontWeight: 600,
                cursor: 'pointer',
              }}
            >
              Delete
            </button>
          </div>
        </div>
      ),
      {
        duration: Infinity,
        style: {
          maxWidth: 360,
          background: 'hsl(222, 41%, 13%)',
          color: 'hsl(220, 14%, 96%)',
          border: '1px solid hsl(0, 84%, 60%, 0.4)',
          borderRadius: 12,
          padding: '16px',
        },
        icon: '⚠',
      },
    );
  };

  const confirmDelete = async (accountId: string) => {
    setDeletingAccountId(accountId);
    try {
      await removeAccount(accountId, true);
      toast.success('Account deleted permanently');
    } catch (error) {
      toast.error(`Failed to delete account: ${error}`);
    } finally {
      setDeletingAccountId(null);
    }
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
                  className="group p-4 rounded-xl cursor-pointer transition-all duration-200"
                  style={{
                    background: 'hsl(var(--harbor-surface-1))',
                    border: '1px solid hsl(var(--harbor-border-subtle))',
                  }}
                  onMouseEnter={(e) => {
                    e.currentTarget.style.background = 'hsl(var(--harbor-primary) / 0.1)';
                    e.currentTarget.style.borderColor = 'hsl(var(--harbor-primary))';
                  }}
                  onMouseLeave={(e) => {
                    e.currentTarget.style.background = 'hsl(var(--harbor-surface-1))';
                    e.currentTarget.style.borderColor = 'hsl(var(--harbor-border-subtle))';
                  }}
                  onClick={() => handleLogin(account)}
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
                        {account.peerId.slice(0, 8)}...{account.peerId.slice(-4)}
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

                    {/* Delete button - always visible */}
                    <button
                      className="p-2 rounded-lg transition-all duration-200 opacity-40 hover:opacity-100 hover:bg-red-500/10 flex-shrink-0"
                      style={{ color: 'hsl(var(--harbor-error))' }}
                      onClick={(e) => handleDeleteClick(e, account)}
                      disabled={deletingAccountId === account.id}
                      title="Delete account"
                    >
                      {deletingAccountId === account.id ? (
                        <svg
                          className="w-4 h-4 animate-spin"
                          fill="none"
                          viewBox="0 0 24 24"
                        >
                          <circle
                            className="opacity-25"
                            cx="12"
                            cy="12"
                            r="10"
                            stroke="currentColor"
                            strokeWidth="4"
                          />
                          <path
                            className="opacity-75"
                            fill="currentColor"
                            d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
                          />
                        </svg>
                      ) : (
                        <TrashIcon className="w-4 h-4" />
                      )}
                    </button>
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
    </div>
  );
}
