import { useEffect, useRef } from 'react';
import { LockIcon } from '../icons';

interface LockAccountDialogProps {
  isLocking: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}

export function LockAccountDialog({ isLocking, onCancel, onConfirm }: LockAccountDialogProps) {
  const cancelButtonRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    cancelButtonRef.current?.focus();

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !isLocking) {
        event.preventDefault();
        onCancel();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isLocking, onCancel]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4"
      style={{ background: 'rgba(0, 0, 0, 0.68)' }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="lock-account-title"
        aria-describedby="lock-account-description"
        className="w-full max-w-md rounded-2xl p-6"
        style={{
          background: 'hsl(var(--harbor-bg-elevated))',
          border: '1px solid hsl(var(--harbor-border-subtle))',
          boxShadow: '0 25px 50px -12px rgba(0, 0, 0, 0.5)',
        }}
      >
        <div className="flex items-start gap-4">
          <div
            className="flex h-11 w-11 flex-shrink-0 items-center justify-center rounded-xl"
            style={{ background: 'hsl(var(--harbor-warning) / 0.15)' }}
          >
            <LockIcon className="h-5 w-5" style={{ color: 'hsl(var(--harbor-warning))' }} />
          </div>
          <div>
            <h2
              id="lock-account-title"
              className="text-lg font-semibold"
              style={{ color: 'hsl(var(--harbor-text-primary))' }}
            >
              Lock this account?
            </h2>
            <p
              id="lock-account-description"
              className="mt-2 text-sm leading-6"
              style={{ color: 'hsl(var(--harbor-text-secondary))' }}
            >
              You will need your password to unlock this account on this device. Harbor cannot
              recover a forgotten password.
            </p>
          </div>
        </div>

        <div className="mt-6 flex justify-end gap-3">
          <button
            ref={cancelButtonRef}
            type="button"
            onClick={onCancel}
            disabled={isLocking}
            className="rounded-lg px-4 py-2 text-sm font-medium disabled:cursor-not-allowed disabled:opacity-50"
            style={{
              background: 'hsl(var(--harbor-surface-1))',
              border: '1px solid hsl(var(--harbor-border-subtle))',
              color: 'hsl(var(--harbor-text-primary))',
            }}
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={onConfirm}
            disabled={isLocking}
            className="rounded-lg px-4 py-2 text-sm font-medium disabled:cursor-not-allowed disabled:opacity-50"
            style={{ background: 'hsl(var(--harbor-warning))', color: 'hsl(216 70% 10%)' }}
          >
            {isLocking ? 'Locking...' : 'Lock Account'}
          </button>
        </div>
      </div>
    </div>
  );
}
