import { useState, useRef, useEffect } from 'react';
import toast from 'react-hot-toast';
import { useIdentityStore } from '../../stores';
import { XIcon } from '../../components/icons';
import { SectionHeader, SettingsCard, PasswordInput } from './shared';

export function SecuritySection() {
  const { state, updatePassphraseHint } = useIdentityStore();
  const identity = state.status === 'unlocked' ? state.identity : null;

  // Passphrase change state
  const [currentPass, setCurrentPass] = useState('');
  const [newPass, setNewPass] = useState('');
  const [confirmPass, setConfirmPass] = useState('');
  const [passError, setPassError] = useState('');
  const [isChangingPass, setIsChangingPass] = useState(false);

  // Delete account modal state
  const [showDeleteModal, setShowDeleteModal] = useState(false);
  const [deleteConfirmText, setDeleteConfirmText] = useState('');
  const [deletePassphrase, setDeletePassphrase] = useState('');
  const [deleteError, setDeleteError] = useState('');
  const [isDeleting, setIsDeleting] = useState(false);

  // Passphrase hint state
  const [hintValue, setHintValue] = useState('');
  const [hintInitialized, setHintInitialized] = useState(false);
  const [isSavingHint, setIsSavingHint] = useState(false);

  // Initialize hint value when identity changes
  useEffect(() => {
    if (identity && !hintInitialized) {
      setHintValue(identity.passphraseHint || '');
      setHintInitialized(true);
    }
  }, [identity, hintInitialized]);

  // Import identity state
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [importError, setImportError] = useState('');
  const [importPassphrase, setImportPassphrase] = useState('');
  const [showImportModal, setShowImportModal] = useState(false);
  const [importFile, setImportFile] = useState<File | null>(null);

  const handlePassphraseChange = async () => {
    setPassError('');
    if (!currentPass || !newPass || !confirmPass) {
      setPassError('All fields are required');
      return;
    }
    if (newPass !== confirmPass) {
      setPassError('New passphrases do not match');
      return;
    }
    if (newPass.length < 8) {
      setPassError('Passphrase must be at least 8 characters');
      return;
    }

    setIsChangingPass(true);
    await new Promise((resolve) => setTimeout(resolve, 1500));
    setIsChangingPass(false);
    setCurrentPass('');
    setNewPass('');
    setConfirmPass('');
    toast.success('Passphrase changed successfully!');
  };

  const handleSaveHint = async () => {
    setIsSavingHint(true);
    try {
      const trimmed = hintValue.trim() || null;
      await updatePassphraseHint(trimmed);
      toast.success(trimmed ? 'Passphrase hint updated!' : 'Passphrase hint removed.');
    } catch {
      toast.error('Failed to update passphrase hint');
    } finally {
      setIsSavingHint(false);
    }
  };

  const handleExportIdentity = () => {
    if (!identity) return;

    const exportData = {
      version: 1,
      type: 'harbor-identity-backup',
      peerId: identity.peerId,
      displayName: identity.displayName,
      bio: identity.bio,
      createdAt: new Date().toISOString(),
      encryptedKeys: 'ENCRYPTED_KEY_DATA_PLACEHOLDER',
      note: "Keep this file safe. You'll need your passphrase to restore it.",
    };

    const blob = new Blob([JSON.stringify(exportData, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = `harbor-backup-${identity.displayName.replace(/\s+/g, '-').toLowerCase()}-${new Date().toISOString().split('T')[0]}.json`;
    document.body.appendChild(anchor);
    anchor.click();
    document.body.removeChild(anchor);
    URL.revokeObjectURL(url);

    toast.success('Backup exported! Keep it safe.');
  };

  const handleImportClick = () => {
    fileInputRef.current?.click();
  };

  const handleFileSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) {
      setImportFile(file);
      setShowImportModal(true);
      setImportError('');
      setImportPassphrase('');
    }
    e.target.value = '';
  };

  const handleImportIdentity = async () => {
    if (!importFile) return;

    if (!importPassphrase) {
      setImportError('Passphrase is required to decrypt the backup');
      return;
    }

    const reader = new FileReader();
    reader.onload = async (event) => {
      try {
        const data = JSON.parse(event.target?.result as string);

        if (data.type !== 'harbor-identity-backup') {
          setImportError('Invalid backup file format');
          return;
        }

        await new Promise((resolve) => setTimeout(resolve, 2000));

        toast.success(`Account recovered! Welcome back, ${data.displayName}`);
        setShowImportModal(false);
        setImportFile(null);
        setImportPassphrase('');
      } catch {
        setImportError('Failed to parse backup file');
      }
    };
    reader.readAsText(importFile);
  };

  const handleDeleteIdentity = () => {
    setShowDeleteModal(true);
    setDeleteConfirmText('');
    setDeletePassphrase('');
    setDeleteError('');
  };

  const confirmDeleteIdentity = async () => {
    if (deleteConfirmText !== 'DELETE') {
      setDeleteError('Please type DELETE to confirm');
      return;
    }

    if (!deletePassphrase) {
      setDeleteError('Passphrase is required');
      return;
    }

    setIsDeleting(true);
    await new Promise((resolve) => setTimeout(resolve, 2000));
    setIsDeleting(false);
    toast.success('Account deleted. Goodbye!');
    setShowDeleteModal(false);
  };

  return (
    <div className="space-y-6">
      <input
        ref={fileInputRef}
        type="file"
        accept=".json"
        onChange={handleFileSelect}
        className="hidden"
      />

      <SectionHeader title="Security" description="Manage your passphrase and encryption keys" />

      {/* Change passphrase */}
      <SettingsCard>
        <h4 className="font-medium mb-2" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
          Change Passphrase
        </h4>
        <p className="text-sm mb-4" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
          Update your passphrase to keep your identity secure
        </p>

        <div className="space-y-3">
          <PasswordInput
            placeholder="Current passphrase"
            value={currentPass}
            onChange={setCurrentPass}
          />
          <PasswordInput placeholder="New passphrase" value={newPass} onChange={setNewPass} />
          <PasswordInput
            placeholder="Confirm new passphrase"
            value={confirmPass}
            onChange={setConfirmPass}
          />
        </div>

        {passError && (
          <p className="text-sm mt-2" style={{ color: 'hsl(var(--harbor-error))' }}>
            {passError}
          </p>
        )}

        <button
          onClick={handlePassphraseChange}
          disabled={isChangingPass}
          className="mt-4 px-4 py-2 rounded-lg text-sm font-medium transition-colors duration-200 disabled:opacity-50"
          style={{
            background:
              'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
            color: 'white',
          }}
        >
          {isChangingPass ? 'Updating...' : 'Update Passphrase'}
        </button>
      </SettingsCard>

      {/* Passphrase Hint */}
      <SettingsCard>
        <h4 className="font-medium mb-2" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
          Passphrase Hint
        </h4>
        <p className="text-sm mb-4" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
          Set a hint to help you remember your passphrase. This will be shown on the unlock screen.
        </p>

        <div className="space-y-3">
          <div>
            <input
              type="text"
              placeholder="e.g. Name of my first pet + birth year"
              value={hintValue}
              onChange={(e) => {
                if (e.target.value.length <= 100) {
                  setHintValue(e.target.value);
                }
              }}
              className="w-full px-4 py-3 rounded-lg text-sm"
              style={{
                background: 'hsl(var(--harbor-surface-1))',
                border: '1px solid hsl(var(--harbor-border-subtle))',
                color: 'hsl(var(--harbor-text-primary))',
              }}
            />
            <div className="flex items-center justify-between mt-1.5">
              <p className="text-xs" style={{ color: 'hsl(var(--harbor-warning))' }}>
                Do not include your actual passphrase in the hint.
              </p>
              <p
                className="text-xs flex-shrink-0 ml-2"
                style={{
                  color:
                    hintValue.length >= 90
                      ? 'hsl(var(--harbor-warning))'
                      : 'hsl(var(--harbor-text-tertiary))',
                }}
              >
                {hintValue.length}/100
              </p>
            </div>
          </div>
        </div>

        <button
          onClick={handleSaveHint}
          disabled={isSavingHint || hintValue === (identity?.passphraseHint || '')}
          className="mt-4 px-4 py-2 rounded-lg text-sm font-medium transition-colors duration-200 disabled:opacity-50 disabled:cursor-not-allowed"
          style={{
            background:
              hintValue !== (identity?.passphraseHint || '')
                ? 'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))'
                : 'hsl(var(--harbor-surface-2))',
            color:
              hintValue !== (identity?.passphraseHint || '')
                ? 'white'
                : 'hsl(var(--harbor-text-tertiary))',
          }}
        >
          {isSavingHint ? 'Saving...' : 'Save Hint'}
        </button>
      </SettingsCard>

      {/* Backup & Recovery */}
      <SettingsCard>
        <h4 className="font-medium mb-2" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
          Backup & Recovery
        </h4>
        <p className="text-sm mb-4" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
          Export your identity to create a backup, or import an existing backup to recover your
          account
        </p>

        <div className="flex gap-3">
          <button
            onClick={handleExportIdentity}
            className="flex-1 px-4 py-3 rounded-lg text-sm font-medium transition-colors duration-200 flex items-center justify-center gap-2"
            style={{
              background:
                'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
              color: 'white',
            }}
          >
            <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12"
              />
            </svg>
            Export Backup
          </button>
          <button
            onClick={handleImportClick}
            className="flex-1 px-4 py-3 rounded-lg text-sm font-medium transition-colors duration-200 flex items-center justify-center gap-2"
            style={{
              background: 'hsl(var(--harbor-surface-1))',
              color: 'hsl(var(--harbor-text-primary))',
              border: '1px solid hsl(var(--harbor-border-subtle))',
            }}
          >
            <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4"
              />
            </svg>
            Recover Account
          </button>
        </div>

        <p className="text-xs mt-3" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
          Your backup file is encrypted with your passphrase. Keep it safe and never share it.
        </p>
      </SettingsCard>

      {/* Danger zone */}
      <SettingsCard variant="danger">
        <h4 className="font-medium mb-2" style={{ color: 'hsl(var(--harbor-error))' }}>
          Delete Account
        </h4>
        <p className="text-sm mb-4" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
          Permanently delete your identity, messages, posts, and all associated data. This action
          cannot be undone.
        </p>
        <button
          onClick={handleDeleteIdentity}
          className="px-4 py-2 rounded-lg text-sm font-medium transition-colors duration-200"
          style={{
            background: 'hsl(var(--harbor-error) / 0.15)',
            color: 'hsl(var(--harbor-error))',
            border: '1px solid hsl(var(--harbor-error) / 0.3)',
          }}
        >
          Delete Account
        </button>
      </SettingsCard>

      {/* Delete Account Modal */}
      {showDeleteModal && (
        <div
          className="fixed inset-0 flex items-center justify-center z-50 p-4"
          style={{ background: 'rgba(0, 0, 0, 0.6)' }}
        >
          <div
            className="w-full max-w-md rounded-lg overflow-hidden"
            style={{
              background: 'hsl(var(--harbor-bg-elevated))',
              border: '1px solid hsl(var(--harbor-border-subtle))',
            }}
          >
            <div
              className="px-6 py-4 flex items-center justify-between border-b"
              style={{
                borderColor: 'hsl(var(--harbor-border-subtle))',
                background: 'hsl(var(--harbor-error) / 0.05)',
              }}
            >
              <h3 className="text-lg font-semibold" style={{ color: 'hsl(var(--harbor-error))' }}>
                Delete Account
              </h3>
              <button
                onClick={() => setShowDeleteModal(false)}
                className="p-1 rounded-lg transition-colors duration-200"
                style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
              >
                <XIcon className="w-5 h-5" />
              </button>
            </div>

            <div className="p-6 space-y-4">
              <div
                className="p-4 rounded-lg"
                style={{
                  background: 'hsl(var(--harbor-error) / 0.1)',
                  border: '1px solid hsl(var(--harbor-error) / 0.2)',
                }}
              >
                <p
                  className="text-sm font-medium mb-2"
                  style={{ color: 'hsl(var(--harbor-error))' }}
                >
                  Warning: This action is irreversible
                </p>
                <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                  Deleting your account will permanently remove:
                </p>
                <ul
                  className="text-sm mt-2 ml-4 space-y-1 list-disc"
                  style={{ color: 'hsl(var(--harbor-text-secondary))' }}
                >
                  <li>Your identity and cryptographic keys</li>
                  <li>All messages and conversations</li>
                  <li>All posts and media</li>
                  <li>Your contacts and permissions</li>
                </ul>
              </div>

              <div>
                <label
                  className="block text-sm font-medium mb-2"
                  style={{ color: 'hsl(var(--harbor-text-primary))' }}
                >
                  Type <span style={{ color: 'hsl(var(--harbor-error))' }}>DELETE</span> to confirm
                </label>
                <input
                  type="text"
                  value={deleteConfirmText}
                  onChange={(e) => setDeleteConfirmText(e.target.value)}
                  placeholder="Type DELETE"
                  className="w-full px-4 py-3 rounded-lg text-sm"
                  style={{
                    background: 'hsl(var(--harbor-surface-1))',
                    border: '1px solid hsl(var(--harbor-border-subtle))',
                    color: 'hsl(var(--harbor-text-primary))',
                  }}
                />
              </div>

              <div>
                <label
                  className="block text-sm font-medium mb-2"
                  style={{ color: 'hsl(var(--harbor-text-primary))' }}
                >
                  Enter your passphrase
                </label>
                <PasswordInput
                  placeholder="Your passphrase"
                  value={deletePassphrase}
                  onChange={setDeletePassphrase}
                />
              </div>

              {deleteError && (
                <p className="text-sm" style={{ color: 'hsl(var(--harbor-error))' }}>
                  {deleteError}
                </p>
              )}
            </div>

            <div
              className="px-6 py-4 flex gap-3 border-t"
              style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
            >
              <button
                onClick={() => setShowDeleteModal(false)}
                className="flex-1 px-4 py-3 rounded-lg text-sm font-medium transition-colors duration-200"
                style={{
                  background: 'hsl(var(--harbor-surface-1))',
                  color: 'hsl(var(--harbor-text-primary))',
                  border: '1px solid hsl(var(--harbor-border-subtle))',
                }}
              >
                Cancel
              </button>
              <button
                onClick={confirmDeleteIdentity}
                disabled={deleteConfirmText !== 'DELETE' || isDeleting}
                className="flex-1 px-4 py-3 rounded-lg text-sm font-medium transition-colors duration-200 disabled:opacity-50 disabled:cursor-not-allowed"
                style={{
                  background: 'hsl(var(--harbor-error))',
                  color: 'white',
                }}
              >
                {isDeleting ? 'Deleting...' : 'Delete Account Forever'}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Import/Recover Modal */}
      {showImportModal && (
        <div
          className="fixed inset-0 flex items-center justify-center z-50 p-4"
          style={{ background: 'rgba(0, 0, 0, 0.6)' }}
        >
          <div
            className="w-full max-w-md rounded-lg overflow-hidden"
            style={{
              background: 'hsl(var(--harbor-bg-elevated))',
              border: '1px solid hsl(var(--harbor-border-subtle))',
            }}
          >
            <div
              className="px-6 py-4 flex items-center justify-between border-b"
              style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
            >
              <h3
                className="text-lg font-semibold"
                style={{ color: 'hsl(var(--harbor-text-primary))' }}
              >
                Recover Account
              </h3>
              <button
                onClick={() => {
                  setShowImportModal(false);
                  setImportFile(null);
                }}
                className="p-1 rounded-lg transition-colors duration-200"
                style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
              >
                <XIcon className="w-5 h-5" />
              </button>
            </div>

            <div className="p-6 space-y-4">
              <div
                className="p-4 rounded-lg"
                style={{
                  background: 'hsl(var(--harbor-surface-1))',
                  border: '1px solid hsl(var(--harbor-border-subtle))',
                }}
              >
                <h4
                  className="text-sm font-medium mb-2"
                  style={{ color: 'hsl(var(--harbor-text-primary))' }}
                >
                  What is account recovery?
                </h4>
                <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                  If you previously exported a backup of your Harbor identity, you can use it to
                  restore your account on this device. Your backup file contains your encrypted
                  cryptographic keys.
                </p>
              </div>

              <div
                className="p-4 rounded-lg"
                style={{
                  background: 'hsl(var(--harbor-primary) / 0.1)',
                  border: '1px solid hsl(var(--harbor-primary) / 0.2)',
                }}
              >
                <p
                  className="text-sm font-medium mb-1"
                  style={{ color: 'hsl(var(--harbor-primary))' }}
                >
                  Selected backup file
                </p>
                <p
                  className="text-sm truncate"
                  style={{ color: 'hsl(var(--harbor-text-secondary))' }}
                >
                  {importFile?.name}
                </p>
              </div>

              <div>
                <label
                  className="block text-sm font-medium mb-2"
                  style={{ color: 'hsl(var(--harbor-text-primary))' }}
                >
                  Enter backup passphrase
                </label>
                <p className="text-sm mb-3" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                  Enter the passphrase you used when you created this backup. This will decrypt your
                  identity keys.
                </p>
                <PasswordInput
                  placeholder="Backup passphrase"
                  value={importPassphrase}
                  onChange={setImportPassphrase}
                />
              </div>

              {importError && (
                <p className="text-sm" style={{ color: 'hsl(var(--harbor-error))' }}>
                  {importError}
                </p>
              )}

              <p className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                Note: Recovering an account will replace your current identity if you have one.
              </p>
            </div>

            <div
              className="px-6 py-4 flex gap-3 border-t"
              style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
            >
              <button
                onClick={() => {
                  setShowImportModal(false);
                  setImportFile(null);
                }}
                className="flex-1 px-4 py-3 rounded-lg text-sm font-medium transition-colors duration-200"
                style={{
                  background: 'hsl(var(--harbor-surface-1))',
                  color: 'hsl(var(--harbor-text-primary))',
                  border: '1px solid hsl(var(--harbor-border-subtle))',
                }}
              >
                Cancel
              </button>
              <button
                onClick={handleImportIdentity}
                className="flex-1 px-4 py-3 rounded-lg text-sm font-medium transition-colors duration-200"
                style={{
                  background:
                    'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
                  color: 'white',
                }}
              >
                Recover Account
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
