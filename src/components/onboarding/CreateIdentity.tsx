import { useState, type FormEvent } from 'react';
import { Button, Input } from '../common';
import { useIdentityStore, useAccountsStore } from '../../stores';
import { HarborIcon, UserIcon, LockIcon, ShieldIcon, ChevronRightIcon } from '../icons';
import { accountsService } from '../../services';

interface CreateIdentityProps {
  onBack?: () => void;
}

export function CreateIdentity({ onBack }: CreateIdentityProps) {
  const { createIdentity, error, clearError } = useIdentityStore();
  const { loadAccounts } = useAccountsStore();

  const [displayName, setDisplayName] = useState('');
  const [passphrase, setPassphrase] = useState('');
  const [confirmPassphrase, setConfirmPassphrase] = useState('');
  const [bio, setBio] = useState('');
  const [loading, setLoading] = useState(false);
  const [localError, setLocalError] = useState<string | null>(null);
  const [step, setStep] = useState<1 | 2>(1);

  const handleNextStep = () => {
    setLocalError(null);
    if (!displayName.trim()) {
      setLocalError('Display name is required');
      return;
    }
    setStep(2);
  };

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    clearError();
    setLocalError(null);

    if (passphrase.length < 8) {
      setLocalError('Passphrase must be at least 8 characters');
      return;
    }

    if (passphrase !== confirmPassphrase) {
      setLocalError('Passphrases do not match');
      return;
    }

    setLoading(true);
    try {
      const identity = await createIdentity({
        displayName: displayName.trim(),
        passphrase,
        bio: bio.trim() || undefined,
      });

      // Register the new account in the accounts registry
      try {
        await accountsService.listAccounts().then(async (accounts) => {
          // Only register if not already in registry (migration case)
          const exists = accounts.some((a) => a.peerId === identity.peerId);
          if (!exists) {
            // The backend will register it when the identity is created
            // Just reload the accounts list
            await loadAccounts();
          }
        });
      } catch {
        // Non-critical, accounts list may not be set up yet
      }
    } catch {
      // Error is handled by store
    } finally {
      setLoading(false);
    }
  };

  const displayError = localError || error;

  // Password strength indicator
  const getPasswordStrength = () => {
    if (!passphrase) return { level: 0, label: '', color: '' };
    if (passphrase.length < 8)
      return { level: 1, label: 'Too short', color: 'hsl(var(--harbor-error))' };
    if (passphrase.length < 12)
      return { level: 2, label: 'Fair', color: 'hsl(var(--harbor-warning))' };
    if (passphrase.length < 16)
      return { level: 3, label: 'Good', color: 'hsl(var(--harbor-success))' };
    return { level: 4, label: 'Strong', color: 'hsl(152 69% 50%)' };
  };

  const strength = getPasswordStrength();

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
          <div className="space-y-6">
            <div className="flex gap-4">
              <div
                className="w-10 h-10 rounded-xl flex items-center justify-center flex-shrink-0"
                style={{ background: 'hsl(var(--harbor-primary) / 0.15)' }}
              >
                <ShieldIcon className="w-5 h-5" style={{ color: 'hsl(var(--harbor-primary))' }} />
              </div>
              <div>
                <h3
                  className="font-semibold mb-1"
                  style={{ color: 'hsl(var(--harbor-text-primary))' }}
                >
                  End-to-End Encrypted
                </h3>
                <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                  Your messages are encrypted before they leave your device. Only you and your
                  contacts can read them.
                </p>
              </div>
            </div>

            <div className="flex gap-4">
              <div
                className="w-10 h-10 rounded-xl flex items-center justify-center flex-shrink-0"
                style={{ background: 'hsl(var(--harbor-accent) / 0.15)' }}
              >
                <LockIcon className="w-5 h-5" style={{ color: 'hsl(var(--harbor-accent))' }} />
              </div>
              <div>
                <h3
                  className="font-semibold mb-1"
                  style={{ color: 'hsl(var(--harbor-text-primary))' }}
                >
                  Own Your Data
                </h3>
                <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                  No central servers, no data harvesting. Your identity and content stay on your
                  device.
                </p>
              </div>
            </div>

            <div className="flex gap-4">
              <div
                className="w-10 h-10 rounded-xl flex items-center justify-center flex-shrink-0"
                style={{ background: 'hsl(var(--harbor-success) / 0.15)' }}
              >
                <UserIcon className="w-5 h-5" style={{ color: 'hsl(var(--harbor-success))' }} />
              </div>
              <div>
                <h3
                  className="font-semibold mb-1"
                  style={{ color: 'hsl(var(--harbor-text-primary))' }}
                >
                  Peer-to-Peer
                </h3>
                <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                  Connect directly with friends. No middlemen, no tracking, just secure
                  communication.
                </p>
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Right side - Form */}
      <div className="flex-1 flex items-center justify-center p-6 lg:p-12">
        <div className="w-full max-w-md">
          <div
            className="rounded-2xl p-8"
            style={{
              background: 'hsl(var(--harbor-bg-elevated))',
              border: '1px solid hsl(var(--harbor-border-subtle))',
              boxShadow: '0 25px 50px -12px rgba(0, 0, 0, 0.5)',
            }}
          >
            {/* Mobile logo */}
            <div className="lg:hidden flex items-center gap-3 mb-8">
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

            {/* Back button (when coming from account selection) */}
            {onBack && (
              <button
                onClick={onBack}
                className="flex items-center gap-2 mb-4 text-sm transition-colors duration-200"
                style={{ color: 'hsl(var(--harbor-text-secondary))' }}
              >
                <ChevronRightIcon className="w-4 h-4 rotate-180" />
                Back to accounts
              </button>
            )}

            {/* Step indicator */}
            <div className="flex items-center gap-3 mb-8">
              <div className="flex items-center gap-2">
                <div
                  className="w-8 h-8 rounded-full flex items-center justify-center text-sm font-semibold"
                  style={{
                    background:
                      'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
                    color: 'white',
                  }}
                >
                  1
                </div>
                <span
                  className="text-sm font-medium"
                  style={{ color: 'hsl(var(--harbor-text-primary))' }}
                >
                  Profile
                </span>
              </div>
              <div
                className="flex-1 h-0.5 rounded"
                style={{
                  background:
                    step === 2
                      ? 'linear-gradient(90deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))'
                      : 'hsl(var(--harbor-surface-2))',
                }}
              />
              <div className="flex items-center gap-2">
                <div
                  className="w-8 h-8 rounded-full flex items-center justify-center text-sm font-semibold transition-all duration-300"
                  style={{
                    background:
                      step === 2
                        ? 'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))'
                        : 'hsl(var(--harbor-surface-2))',
                    color: step === 2 ? 'white' : 'hsl(var(--harbor-text-tertiary))',
                  }}
                >
                  2
                </div>
                <span
                  className="text-sm font-medium"
                  style={{
                    color:
                      step === 2
                        ? 'hsl(var(--harbor-text-primary))'
                        : 'hsl(var(--harbor-text-tertiary))',
                  }}
                >
                  Security
                </span>
              </div>
            </div>

            {/* Header */}
            <div className="mb-6">
              <h2
                className="text-2xl font-bold mb-2"
                style={{ color: 'hsl(var(--harbor-text-primary))' }}
              >
                {step === 1 ? 'Create Your Identity' : 'Secure Your Keys'}
              </h2>
              <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                {step === 1
                  ? 'Choose how others will see you on the network'
                  : 'Your passphrase encrypts your private keys locally'}
              </p>
            </div>

            {/* Form */}
            <form
              onSubmit={
                step === 1
                  ? (e) => {
                      e.preventDefault();
                      handleNextStep();
                    }
                  : handleSubmit
              }
            >
              {step === 1 ? (
                <div className="space-y-4">
                  <Input
                    label="Display Name"
                    type="text"
                    value={displayName}
                    onChange={(e) => setDisplayName(e.target.value)}
                    placeholder="How others will see you"
                    autoFocus
                  />

                  <Input
                    label="Bio (optional)"
                    type="text"
                    value={bio}
                    onChange={(e) => setBio(e.target.value)}
                    placeholder="Tell others about yourself"
                  />

                  {displayError && (
                    <div
                      className="p-3 rounded-xl text-sm"
                      style={{
                        background: 'hsl(var(--harbor-error) / 0.1)',
                        color: 'hsl(var(--harbor-error))',
                        border: '1px solid hsl(var(--harbor-error) / 0.2)',
                      }}
                    >
                      {displayError}
                    </div>
                  )}

                  <Button type="submit" className="w-full" size="lg">
                    Continue
                  </Button>
                </div>
              ) : (
                <div className="space-y-4">
                  <div>
                    <Input
                      label="Passphrase"
                      type="password"
                      value={passphrase}
                      onChange={(e) => setPassphrase(e.target.value)}
                      placeholder="At least 8 characters"
                      autoFocus
                    />
                    {/* Password strength indicator */}
                    {passphrase && (
                      <div className="mt-2">
                        <div className="flex gap-1 mb-1">
                          {[1, 2, 3, 4].map((level) => (
                            <div
                              key={level}
                              className="h-1 flex-1 rounded-full transition-colors duration-200"
                              style={{
                                background:
                                  level <= strength.level
                                    ? strength.color
                                    : 'hsl(var(--harbor-surface-2))',
                              }}
                            />
                          ))}
                        </div>
                        <p className="text-xs" style={{ color: strength.color }}>
                          {strength.label}
                        </p>
                      </div>
                    )}
                  </div>

                  <Input
                    label="Confirm Passphrase"
                    type="password"
                    value={confirmPassphrase}
                    onChange={(e) => setConfirmPassphrase(e.target.value)}
                    placeholder="Enter passphrase again"
                  />

                  {displayError && (
                    <div
                      className="p-3 rounded-xl text-sm"
                      style={{
                        background: 'hsl(var(--harbor-error) / 0.1)',
                        color: 'hsl(var(--harbor-error))',
                        border: '1px solid hsl(var(--harbor-error) / 0.2)',
                      }}
                    >
                      {displayError}
                    </div>
                  )}

                  <div className="flex gap-3">
                    <Button
                      type="button"
                      variant="secondary"
                      className="flex-1"
                      size="lg"
                      onClick={() => setStep(1)}
                    >
                      Back
                    </Button>
                    <Button type="submit" className="flex-1" size="lg" loading={loading}>
                      Create Identity
                    </Button>
                  </div>
                </div>
              )}
            </form>

            {/* Security notice */}
            {step === 2 && (
              <div
                className="mt-6 p-4 rounded-xl"
                style={{
                  background: 'hsl(var(--harbor-warning) / 0.1)',
                  border: '1px solid hsl(var(--harbor-warning) / 0.2)',
                }}
              >
                <div className="flex gap-3">
                  <ShieldIcon
                    className="w-5 h-5 flex-shrink-0 mt-0.5"
                    style={{ color: 'hsl(var(--harbor-warning))' }}
                  />
                  <div>
                    <p
                      className="text-sm font-medium mb-1"
                      style={{ color: 'hsl(var(--harbor-warning))' }}
                    >
                      Important
                    </p>
                    <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                      Your passphrase encrypts your private keys. If you lose it, you cannot recover
                      your identity. Store it safely!
                    </p>
                  </div>
                </div>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
