import { useState, type FormEvent } from "react";
import { Button, Input } from "../common";
import { useIdentityStore } from "../../stores";

export function CreateIdentity() {
  const { createIdentity, error, clearError } = useIdentityStore();

  const [displayName, setDisplayName] = useState("");
  const [passphrase, setPassphrase] = useState("");
  const [confirmPassphrase, setConfirmPassphrase] = useState("");
  const [bio, setBio] = useState("");
  const [loading, setLoading] = useState(false);
  const [localError, setLocalError] = useState<string | null>(null);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    clearError();
    setLocalError(null);

    // Validation
    if (!displayName.trim()) {
      setLocalError("Display name is required");
      return;
    }

    if (passphrase.length < 8) {
      setLocalError("Passphrase must be at least 8 characters");
      return;
    }

    if (passphrase !== confirmPassphrase) {
      setLocalError("Passphrases do not match");
      return;
    }

    setLoading(true);
    try {
      await createIdentity({
        displayName: displayName.trim(),
        passphrase,
        bio: bio.trim() || undefined,
      });
    } catch {
      // Error is handled by store
    } finally {
      setLoading(false);
    }
  };

  const displayError = localError || error;

  return (
    <div className="min-h-screen flex items-center justify-center bg-gradient-to-br from-blue-50 to-indigo-100 dark:from-gray-900 dark:to-gray-800 p-4">
      <div className="w-full max-w-md">
        <div className="bg-white dark:bg-gray-800 rounded-2xl shadow-xl p-8">
          {/* Header */}
          <div className="text-center mb-8">
            <div className="inline-flex items-center justify-center w-16 h-16 rounded-full bg-blue-100 dark:bg-blue-900 mb-4">
              <span className="text-3xl">🔐</span>
            </div>
            <h1 className="text-2xl font-bold text-gray-900 dark:text-white">
              Create Your Identity
            </h1>
            <p className="text-gray-600 dark:text-gray-400 mt-2">
              Your identity is stored locally and secured with your passphrase
            </p>
          </div>

          {/* Form */}
          <form onSubmit={handleSubmit} className="space-y-4">
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

            <Input
              label="Passphrase"
              type="password"
              value={passphrase}
              onChange={(e) => setPassphrase(e.target.value)}
              placeholder="At least 8 characters"
            />

            <Input
              label="Confirm Passphrase"
              type="password"
              value={confirmPassphrase}
              onChange={(e) => setConfirmPassphrase(e.target.value)}
              placeholder="Enter passphrase again"
            />

            {displayError && (
              <div className="p-3 rounded-lg bg-red-50 dark:bg-red-900/20 text-red-600 dark:text-red-400 text-sm">
                {displayError}
              </div>
            )}

            <Button
              type="submit"
              className="w-full"
              size="lg"
              loading={loading}
            >
              Create Identity
            </Button>
          </form>

          {/* Info */}
          <div className="mt-6 p-4 rounded-lg bg-amber-50 dark:bg-amber-900/20 text-amber-800 dark:text-amber-200 text-sm">
            <strong>Important:</strong> Your passphrase encrypts your private keys.
            If you lose it, you cannot recover your identity. Store it safely!
          </div>
        </div>
      </div>
    </div>
  );
}
