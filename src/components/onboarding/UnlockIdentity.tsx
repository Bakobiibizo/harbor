import { useState, type FormEvent } from 'react';
import { Button, Input } from '../common';
import { useIdentityStore } from '../../stores';
import { HarborIcon, LockIcon, UnlockIcon, UsersIcon } from '../icons';
import { getInitials } from '../../utils/formatting';

interface UnlockIdentityProps {
  onSwitchAccount?: () => void;
}

export function UnlockIdentity({ onSwitchAccount }: UnlockIdentityProps) {
  const { state, unlock, error, clearError } = useIdentityStore();

  const [passphrase, setPassphrase] = useState('');
  const [loading, setLoading] = useState(false);
  const [showPassphrase, setShowPassphrase] = useState(false);
  const [showHint, setShowHint] = useState(false);

  const identity = state.status === 'locked' ? state.identity : null;

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    clearError();

    if (!passphrase) {
      return;
    }

    setLoading(true);
    try {
      await unlock(passphrase);
    } catch {
      // Error is handled by store
    } finally {
      setLoading(false);
    }
  };

  return (
    <div
      className="min-h-screen flex items-center justify-center p-6"
      style={{
        background:
          'linear-gradient(135deg, hsl(220 91% 8%) 0%, hsl(262 60% 12%) 50%, hsl(220 91% 8%) 100%)',
      }}
    >
      <div className="w-full max-w-md">
        {/* Card */}
        <div
          className="rounded-2xl p-8 relative overflow-hidden"
          style={{
            background: 'hsl(var(--harbor-bg-elevated))',
            border: '1px solid hsl(var(--harbor-border-subtle))',
            boxShadow: '0 25px 50px -12px rgba(0, 0, 0, 0.5)',
          }}
        >
          {/* Decorative gradient blob */}
          <div
            className="absolute -top-24 -right-24 w-48 h-48 rounded-full blur-3xl opacity-20"
            style={{
              background:
                'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
            }}
          />

          {/* Logo and branding */}
          <div className="relative text-center mb-8">
            <div className="flex items-center justify-center gap-3 mb-6">
              <div
                className="w-12 h-12 rounded-xl flex items-center justify-center"
                style={{
                  background:
                    'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
                  boxShadow: '0 8px 24px hsl(var(--harbor-primary) / 0.4)',
                }}
              >
                <HarborIcon className="w-7 h-7 text-white" />
              </div>
              <span
                className="text-xl font-bold"
                style={{ color: 'hsl(var(--harbor-text-primary))' }}
              >
                Harbor
              </span>
            </div>

            {/* Lock icon */}
            <div className="relative inline-block mb-4">
              <div
                className="w-20 h-20 rounded-full flex items-center justify-center mx-auto"
                style={{
                  background: 'hsl(var(--harbor-surface-1))',
                  border: '2px solid hsl(var(--harbor-border-subtle))',
                }}
              >
                <LockIcon className="w-8 h-8" style={{ color: 'hsl(var(--harbor-primary))' }} />
              </div>
            </div>

            <h1
              className="text-2xl font-bold mb-2"
              style={{ color: 'hsl(var(--harbor-text-primary))' }}
            >
              Welcome Back
            </h1>
            <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
              Enter your passphrase to unlock your identity
            </p>
          </div>

          {/* User info card */}
          {identity && (
            <div
              className="relative mb-6 p-4 rounded-xl"
              style={{
                background: 'hsl(var(--harbor-surface-1))',
                border: '1px solid hsl(var(--harbor-border-subtle))',
              }}
            >
              <div className="flex items-center gap-4">
                {/* Avatar */}
                <div
                  className="w-14 h-14 rounded-full flex items-center justify-center text-lg font-semibold text-white flex-shrink-0"
                  style={{
                    background:
                      'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
                  }}
                >
                  {identity.avatarHash ? (
                    <img
                      src={`/media/${identity.avatarHash}`}
                      alt=""
                      className="w-full h-full rounded-full object-cover"
                    />
                  ) : (
                    getInitials(identity.displayName)
                  )}
                </div>

                {/* Info */}
                <div className="flex-1 min-w-0">
                  <p
                    className="font-semibold truncate"
                    style={{ color: 'hsl(var(--harbor-text-primary))' }}
                  >
                    {identity.displayName}
                  </p>
                  <p
                    className="text-xs truncate font-mono"
                    style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                  >
                    {identity.peerId.slice(0, 12)}...{identity.peerId.slice(-8)}
                  </p>
                </div>
              </div>
            </div>
          )}

          {/* Form */}
          <form onSubmit={handleSubmit} className="relative space-y-4">
            <div className="relative">
              <Input
                label="Passphrase"
                type={showPassphrase ? 'text' : 'password'}
                value={passphrase}
                onChange={(e) => setPassphrase(e.target.value)}
                placeholder="Enter your passphrase"
                autoFocus
              />
              {/* Show/hide toggle */}
              <button
                type="button"
                className="absolute right-3 top-8 p-1.5 rounded-lg transition-colors duration-200"
                style={{
                  color: 'hsl(var(--harbor-text-tertiary))',
                  background: showPassphrase ? 'hsl(var(--harbor-surface-2))' : 'transparent',
                }}
                onClick={() => setShowPassphrase(!showPassphrase)}
              >
                {showPassphrase ? (
                  <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={1.5}
                      d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
                    />
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={1.5}
                      d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z"
                    />
                  </svg>
                ) : (
                  <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={1.5}
                      d="M13.875 18.825A10.05 10.05 0 0112 19c-4.478 0-8.268-2.943-9.543-7a9.97 9.97 0 011.563-3.029m5.858.908a3 3 0 114.243 4.243M9.878 9.878l4.242 4.242M9.88 9.88l-3.29-3.29m7.532 7.532l3.29 3.29M3 3l3.59 3.59m0 0A9.953 9.953 0 0112 5c4.478 0 8.268 2.943 9.543 7a10.025 10.025 0 01-4.132 5.411m0 0L21 21"
                    />
                  </svg>
                )}
              </button>
            </div>

            {/* Passphrase hint toggle */}
            {identity?.passphraseHint && (
              <div>
                {showHint ? (
                  <div
                    className="p-3 rounded-xl text-sm flex items-center gap-2"
                    style={{
                      background: 'hsl(var(--harbor-primary) / 0.1)',
                      color: 'hsl(var(--harbor-primary))',
                      border: '1px solid hsl(var(--harbor-primary) / 0.2)',
                    }}
                  >
                    <svg
                      className="w-4 h-4 flex-shrink-0"
                      fill="none"
                      viewBox="0 0 24 24"
                      stroke="currentColor"
                    >
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        strokeWidth={2}
                        d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
                      />
                    </svg>
                    <span>
                      <strong>Hint:</strong> {identity.passphraseHint}
                    </span>
                  </div>
                ) : (
                  <button
                    type="button"
                    onClick={() => setShowHint(true)}
                    className="text-sm transition-colors duration-200 flex items-center gap-1.5"
                    style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                  >
                    <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        strokeWidth={2}
                        d="M8.228 9c.549-1.165 2.03-2 3.772-2 2.21 0 4 1.343 4 3 0 1.4-1.278 2.575-3.006 2.907-.542.104-.994.54-.994 1.093m0 3h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
                      />
                    </svg>
                    Show passphrase hint
                  </button>
                )}
              </div>
            )}

            {error && (
              <div
                className="p-3 rounded-xl text-sm flex items-center gap-2"
                style={{
                  background: 'hsl(var(--harbor-error) / 0.1)',
                  color: 'hsl(var(--harbor-error))',
                  border: '1px solid hsl(var(--harbor-error) / 0.2)',
                }}
              >
                <svg
                  className="w-4 h-4 flex-shrink-0"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"
                  />
                </svg>
                {error}
              </div>
            )}

            <Button type="submit" className="w-full" size="lg" loading={loading}>
              <UnlockIcon className="w-5 h-5 mr-2" />
              Unlock
            </Button>
          </form>

          {/* Security tip */}
          <div
            className="relative mt-6 p-4 rounded-xl text-center"
            style={{
              background: 'hsl(var(--harbor-surface-1))',
            }}
          >
            <p className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
              Your identity is encrypted and stored locally.
              <br />
              <span style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                Only you can access it with your passphrase.
              </span>
            </p>
          </div>

          {/* Switch account button */}
          {onSwitchAccount && (
            <button
              onClick={onSwitchAccount}
              className="w-full mt-4 p-3 rounded-xl flex items-center justify-center gap-2 text-sm transition-colors duration-200"
              style={{
                background: 'hsl(var(--harbor-surface-1))',
                color: 'hsl(var(--harbor-text-secondary))',
                border: '1px solid hsl(var(--harbor-border-subtle))',
              }}
            >
              <UsersIcon className="w-4 h-4" />
              Switch Account
            </button>
          )}
        </div>

        {/* Footer */}
        <p
          className="text-center mt-6 text-xs"
          style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
        >
          Need help? Check the{' '}
          <a
            href="#"
            className="underline underline-offset-2 transition-colors duration-200"
            style={{ color: 'hsl(var(--harbor-primary))' }}
          >
            documentation
          </a>
        </p>
      </div>
    </div>
  );
}
