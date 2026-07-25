import type { IdentityState } from '../../types';
import { Button } from '../common';
import { HarborIcon } from '../icons';

type FailureState = Extract<IdentityState, { status: 'recoverableError' | 'fatalError' }>;

interface IdentityInitializationFailureProps {
  state: FailureState;
  onRetry: () => void;
  onSwitchAccount?: () => void;
}

const sourceLabels: Record<FailureState['source'], string> = {
  ipc: 'the Harbor desktop process',
  profileStorage: 'this account\'s local preferences',
  identityDatabase: 'your local account database',
  identityCorruption: 'your saved identity',
  accountRegistry: 'the local account registry',
};

export function IdentityInitializationFailure({
  state,
  onRetry,
  onSwitchAccount,
}: IdentityInitializationFailureProps) {
  const recoverable = state.status === 'recoverableError';

  return (
    <div
      className="min-h-screen flex items-center justify-center p-6"
      style={{
        background:
          'linear-gradient(135deg, hsl(var(--harbor-brand-backdrop-start)) 0%, hsl(var(--harbor-brand-backdrop-mid)) 50%, hsl(var(--harbor-brand-backdrop-end)) 100%)',
      }}
    >
      <div
        className="w-full max-w-lg rounded-2xl p-8"
        style={{
          background: 'hsl(var(--harbor-bg-elevated))',
          border: '1px solid hsl(var(--harbor-border-subtle))',
          boxShadow: '0 25px 50px -12px rgba(0, 0, 0, 0.5)',
        }}
      >
        <div className="w-14 h-14 mb-6">
          <HarborIcon className="w-14 h-14" />
        </div>
        <h1
          className="text-2xl font-bold mb-3"
          style={{ color: 'hsl(var(--harbor-text-primary))' }}
        >
          {recoverable
            ? "Harbor couldn't load your account"
            : 'Harbor cannot safely open this account'}
        </h1>
        <p className="mb-4" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
          There was a problem with {sourceLabels[state.source]}. Harbor did not treat this as a new
          account.
        </p>
        <div
          className="rounded-xl p-4 mb-4"
          style={{
            background: 'hsl(var(--harbor-surface-1))',
            border: '1px solid hsl(var(--harbor-border-subtle))',
          }}
        >
          <p className="font-medium" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
            {state.error.message}
          </p>
          {state.error.details && (
            <p className="text-sm mt-2" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
              {state.error.details}
            </p>
          )}
          <p className="text-xs mt-3" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
            Error code: {state.error.code}
          </p>
        </div>
        {state.error.recovery && (
          <p className="text-sm mb-5" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
            {state.error.recovery}
          </p>
        )}
        {!recoverable && (
          <p className="text-sm mb-5" style={{ color: 'hsl(var(--harbor-error))' }}>
            Harbor stopped before changing your account. Do not create a replacement account; your
            existing data may still be recoverable.
          </p>
        )}
        <div className="flex flex-wrap gap-3">
          {recoverable && <Button onClick={onRetry}>Retry</Button>}
          {onSwitchAccount && (
            <Button variant="secondary" onClick={onSwitchAccount}>
              Switch account
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}
