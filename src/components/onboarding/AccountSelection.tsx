import { useState } from 'react';
import { relaunch } from '@tauri-apps/plugin-process';
import { Button } from '../common';
import { useAccountsStore } from '../../stores';
import { HarborIcon, UserPlusIcon, TrashIcon, LockIcon } from '../icons';
import type { AccountInfo } from '../../types';
import toast from 'react-hot-toast';
import { isVerifiedQualifiedName, unverifiedIdentityLabel } from '../../utils/relayName';
import { accountBackupService } from '../../services';
import { suspendProfile } from '../../services/profileSession';
import { getErrorMessage } from '../../utils/errors';
import { AvatarMedia } from '../common/AvatarMedia';

interface AccountSelectionProps {
  onCreateAccount: () => void;
}

export function AccountSelection({ onCreateAccount }: AccountSelectionProps) {
  const { accounts, loadAccounts, setActiveAccount } = useAccountsStore();
  const [selectedAccountId, setSelectedAccountId] = useState<string | null>(null);
  const [switchingAccountId, setSwitchingAccountId] = useState<string | null>(null);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState<string | null>(null);
  const [deletePassword, setDeletePassword] = useState('');
  const [deleteError, setDeleteError] = useState('');
  const [isDeleting, setIsDeleting] = useState(false);

  const handleLogin = async (account: AccountInfo) => {
    if (switchingAccountId) return;

    setSwitchingAccountId(account.id);
    try {
      await setActiveAccount(account.id);
      suspendProfile();
      await relaunch();
    } catch (error) {
      toast.error(`Failed to switch account: ${getErrorMessage(error)}`);
      setSwitchingAccountId(null);
    }
  };

  const handleDelete = async (accountId: string) => {
    if (!deletePassword) {
      setDeleteError('Password is required');
      return;
    }

    setDeleteError('');
    setIsDeleting(true);
    try {
      const result = await accountBackupService.deleteAccountProfile(accountId, deletePassword);
      toast.success('Account data was deleted from this device.');
      setShowDeleteConfirm(null);
      setDeletePassword('');
      suspendProfile();
      if (result.restartRequired) await relaunch();
      else await loadAccounts();
    } catch (error) {
      setDeleteError(getErrorMessage(error));
    } finally {
      setIsDeleting(false);
    }
  };

  const closeDeleteConfirm = () => {
    if (isDeleting) return;
    setShowDeleteConfirm(null);
    setDeletePassword('');
    setDeleteError('');
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

  const localAccountLabel = (account: AccountInfo) => {
    const verifiedNameIsActive =
      isVerifiedQualifiedName(account.verifiedQualifiedName) &&
      typeof account.verifiedNameNotAfter === 'number' &&
      account.verifiedNameNotAfter >= Math.floor(Date.now() / 1000);
    if (verifiedNameIsActive && account.verifiedQualifiedName) {
      return account.verifiedQualifiedName;
    }
    return unverifiedIdentityLabel(account.displayName || 'Local Harbor account');
  };

  return (
    <div
      className="min-h-screen flex"
      style={{
        background:
          'linear-gradient(135deg, hsl(var(--harbor-brand-backdrop-start)) 0%, hsl(var(--harbor-brand-backdrop-mid)) 50%, hsl(var(--harbor-brand-backdrop-end)) 100%)',
      }}
    >
      {/* Left side - Branding */}
      <div className="hidden lg:flex flex-1 items-center justify-center p-12">
        <div className="max-w-md">
          {/* Logo */}
          <div className="flex items-center gap-4 mb-8">
            <div className="w-14 h-14 flex items-center justify-center">
              <HarborIcon className="w-14 h-14" />
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
              <div className="w-10 h-10 flex items-center justify-center">
                <HarborIcon className="w-10 h-10" />
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
                        background: account.avatarHash
                          ? 'transparent'
                          : 'hsl(var(--harbor-surface-3))',
                      }}
                    >
                      {account.avatarHash ? (
                        <AvatarMedia
                          hash={account.avatarHash}
                          className="w-full h-full rounded-full object-cover"
                        />
                      ) : (
                        getInitials(localAccountLabel(account))
                      )}
                    </div>

                    {/* Info */}
                    <div className="flex-1 min-w-0">
                      <p
                        className="font-semibold truncate"
                        style={{ color: 'hsl(var(--harbor-text-primary))' }}
                      >
                        {localAccountLabel(account)}
                      </p>
                      {account.bio && (
                        <p
                          className="text-xs truncate"
                          style={{ color: 'hsl(var(--harbor-text-secondary))' }}
                        >
                          {account.bio}
                        </p>
                      )}
                      <p
                        className="text-xs truncate"
                        style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                      >
                        Saved on this device
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
                            disabled={switchingAccountId !== null}
                            onClick={(e) => {
                              e.stopPropagation();
                              void handleLogin(account);
                            }}
                          >
                            <LockIcon className="w-4 h-4 mr-1" />
                            {switchingAccountId === account.id ? 'Switching...' : 'Login'}
                          </Button>
                          <button
                            className="p-2 rounded-lg transition-colors duration-200 hover:bg-red-500/10"
                            style={{ color: 'hsl(var(--harbor-error))' }}
                            onClick={(e) => {
                              e.stopPropagation();
                              setShowDeleteConfirm(account.id);
                              setDeletePassword('');
                              setDeleteError('');
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
          onClick={closeDeleteConfirm}
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
              This deletes this account's local keys and data from this device. Copies of content
              you already shared may remain with contacts or relays.
            </p>

            <label className="block mb-4">
              <span
                className="block text-sm font-medium mb-2"
                style={{ color: 'hsl(var(--harbor-text-primary))' }}
              >
                Account password
              </span>
              <input
                type="password"
                value={deletePassword}
                onChange={(event) => setDeletePassword(event.target.value)}
                autoComplete="current-password"
                disabled={isDeleting}
                className="w-full px-4 py-3 rounded-lg text-sm disabled:opacity-60"
                style={{
                  background: 'hsl(var(--harbor-surface-1))',
                  border: '1px solid hsl(var(--harbor-border-subtle))',
                  color: 'hsl(var(--harbor-text-primary))',
                }}
              />
            </label>

            {deleteError && (
              <p
                role="alert"
                className="text-sm mb-4"
                style={{ color: 'hsl(var(--harbor-error))' }}
              >
                {deleteError}
              </p>
            )}

            <div className="flex gap-3">
              <Button
                variant="secondary"
                className="flex-1"
                onClick={closeDeleteConfirm}
                disabled={isDeleting}
              >
                Cancel
              </Button>
              <Button
                variant="danger"
                className="flex-1"
                onClick={() => void handleDelete(showDeleteConfirm)}
                disabled={isDeleting}
              >
                {isDeleting ? 'Deleting...' : 'Delete'}
              </Button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
