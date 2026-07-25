import { useState } from 'react';
import { open, save } from '@tauri-apps/plugin-dialog';
import { relaunch } from '@tauri-apps/plugin-process';
import toast from 'react-hot-toast';
import { accountBackupService } from '../../services';
import type { IdentityBackupRestoreResult } from '../../types';
import { getErrorMessage } from '../../utils/errors';
import { XIcon } from '../icons';

const BACKUP_FILTER = [{ name: 'Harbor identity backup', extensions: ['harbor-backup'] }];

function safeBackupName(label?: string): string {
  const normalized = label
    ?.trim()
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, '-')
    .replace(/^-+|-+$/g, '');
  return `harbor-${normalized || 'identity'}.harbor-backup`;
}

interface IdentityBackupActionsProps {
  allowExport?: boolean;
  identityLabel?: string;
  className?: string;
  onRestored?: (result: IdentityBackupRestoreResult) => void | Promise<void>;
}

/**
 * Shared native backup flow. The encrypted file is selected by path and passed
 * directly to Rust; its contents and metadata are never read by the renderer.
 */
export function IdentityBackupActions({
  allowExport = false,
  identityLabel,
  className = '',
  onRestored,
}: IdentityBackupActionsProps) {
  const [mode, setMode] = useState<'export' | 'restore' | null>(null);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
  const [pending, setPending] = useState(false);

  const close = () => {
    if (pending) return;
    setMode(null);
    setSelectedPath(null);
    setPassword('');
    setError('');
  };

  const beginExport = () => {
    setMode('export');
    setSelectedPath(null);
    setPassword('');
    setError('');
  };

  const beginRestore = async () => {
    setError('');
    try {
      const path = await open({
        title: 'Choose a Harbor identity backup',
        multiple: false,
        directory: false,
        filters: BACKUP_FILTER,
      });
      if (typeof path !== 'string') return;
      setSelectedPath(path);
      setMode('restore');
      setPassword('');
    } catch (dialogError) {
      toast.error(`Could not open the backup picker: ${getErrorMessage(dialogError)}`);
    }
  };

  const submit = async () => {
    if (!password) {
      setError('Password is required');
      return;
    }

    setError('');
    setPending(true);
    try {
      if (mode === 'export') {
        const path = await save({
          title: 'Save Harbor identity backup',
          defaultPath: safeBackupName(identityLabel),
          filters: BACKUP_FILTER,
        });
        if (!path) return;
        const result = await accountBackupService.exportIdentity(path, password);
        toast.success(`Encrypted backup saved (${result.bytesWritten.toLocaleString()} bytes).`);
        setMode(null);
        setPassword('');
        return;
      }

      if (mode === 'restore' && selectedPath) {
        const result = await accountBackupService.restoreIdentity(selectedPath, password);
        toast.success('Account recovered from the encrypted backup.');
        setMode(null);
        setSelectedPath(null);
        setPassword('');
        await onRestored?.(result);
        if (result.restartRequired) await relaunch();
      }
    } catch (operationError) {
      setError(getErrorMessage(operationError));
    } finally {
      setPending(false);
    }
  };

  return (
    <>
      <div className={`flex gap-3 ${className}`.trim()}>
        {allowExport && (
          <button
            type="button"
            onClick={beginExport}
            className="flex-1 px-4 py-3 rounded-lg text-sm font-medium transition-colors duration-200 disabled:opacity-50"
            style={{ background: 'hsl(var(--harbor-primary))', color: 'white' }}
          >
            Export Backup
          </button>
        )}
        <button
          type="button"
          onClick={() => void beginRestore()}
          className="flex-1 px-4 py-3 rounded-lg text-sm font-medium transition-colors duration-200"
          style={{
            background: 'hsl(var(--harbor-surface-1))',
            color: 'hsl(var(--harbor-text-primary))',
            border: '1px solid hsl(var(--harbor-border-subtle))',
          }}
        >
          Recover from Backup
        </button>
      </div>

      {mode && (
        <div
          className="fixed inset-0 flex items-center justify-center z-50 p-4"
          style={{ background: 'rgba(0, 0, 0, 0.65)', backdropFilter: 'blur(4px)' }}
          role="presentation"
        >
          <div
            role="dialog"
            aria-modal="true"
            aria-labelledby="identity-backup-dialog-title"
            className="w-full max-w-md rounded-xl overflow-hidden"
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
                id="identity-backup-dialog-title"
                className="text-lg font-semibold"
                style={{ color: 'hsl(var(--harbor-text-primary))' }}
              >
                {mode === 'export' ? 'Export Encrypted Backup' : 'Recover Account'}
              </h3>
              <button
                type="button"
                onClick={close}
                disabled={pending}
                aria-label="Close backup dialog"
                className="p-1 rounded-lg disabled:opacity-50"
                style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
              >
                <XIcon className="w-5 h-5" />
              </button>
            </div>

            <div className="p-6 space-y-4">
              <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                {mode === 'export'
                  ? 'Enter your current password. Harbor encrypts the backup before writing it to the location you choose.'
                  : 'Enter the password used to create this backup. Harbor validates and decrypts it locally.'}
              </p>
              {mode === 'restore' && (
                <p className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                  A .harbor-backup file is selected. Harbor does not inspect it in the page.
                </p>
              )}
              <div>
                <label
                  htmlFor="identity-backup-password"
                  className="block text-sm font-medium mb-2"
                  style={{ color: 'hsl(var(--harbor-text-primary))' }}
                >
                  {mode === 'export' ? 'Current password' : 'Backup password'}
                </label>
                <input
                  id="identity-backup-password"
                  type="password"
                  value={password}
                  onChange={(event) => setPassword(event.target.value)}
                  autoComplete="current-password"
                  disabled={pending}
                  className="w-full px-4 py-3 rounded-lg text-sm disabled:opacity-60"
                  style={{
                    background: 'hsl(var(--harbor-surface-1))',
                    border: '1px solid hsl(var(--harbor-border-subtle))',
                    color: 'hsl(var(--harbor-text-primary))',
                  }}
                />
              </div>
              {error && (
                <p role="alert" className="text-sm" style={{ color: 'hsl(var(--harbor-error))' }}>
                  {error}
                </p>
              )}
              <p className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                Harbor cannot recover a lost backup password. Keep the backup and its password in
                separate safe locations.
              </p>
            </div>

            <div
              className="px-6 py-4 flex gap-3 border-t"
              style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
            >
              <button
                type="button"
                onClick={close}
                disabled={pending}
                className="flex-1 px-4 py-3 rounded-lg text-sm font-medium disabled:opacity-50"
                style={{
                  background: 'hsl(var(--harbor-surface-1))',
                  color: 'hsl(var(--harbor-text-primary))',
                }}
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={() => void submit()}
                disabled={pending}
                className="flex-1 px-4 py-3 rounded-lg text-sm font-medium disabled:opacity-50"
                style={{ background: 'hsl(var(--harbor-primary))', color: 'white' }}
              >
                {pending
                  ? mode === 'export'
                    ? 'Exporting...'
                    : 'Recovering...'
                  : mode === 'export'
                    ? 'Choose Save Location'
                    : 'Recover Account'}
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
