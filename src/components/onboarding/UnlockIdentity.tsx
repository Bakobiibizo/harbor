import { useState, type FormEvent } from "react";
import { Button, Input } from "../common";
import { useIdentityStore } from "../../stores";

export function UnlockIdentity() {
  const { state, unlock, error, clearError } = useIdentityStore();

  const [passphrase, setPassphrase] = useState("");
  const [loading, setLoading] = useState(false);

  const identity = state.status === "locked" ? state.identity : null;

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
    <div className="min-h-screen flex items-center justify-center bg-gradient-to-br from-blue-50 to-indigo-100 dark:from-gray-900 dark:to-gray-800 p-4">
      <div className="w-full max-w-md">
        <div className="bg-white dark:bg-gray-800 rounded-2xl shadow-xl p-8">
          {/* Header */}
          <div className="text-center mb-8">
            <div className="inline-flex items-center justify-center w-16 h-16 rounded-full bg-blue-100 dark:bg-blue-900 mb-4">
              <span className="text-3xl">🔓</span>
            </div>
            <h1 className="text-2xl font-bold text-gray-900 dark:text-white">
              Welcome Back
            </h1>
            {identity && (
              <p className="text-gray-600 dark:text-gray-400 mt-2">
                Unlock as <strong>{identity.displayName}</strong>
              </p>
            )}
          </div>

          {/* Form */}
          <form onSubmit={handleSubmit} className="space-y-4">
            <Input
              label="Passphrase"
              type="password"
              value={passphrase}
              onChange={(e) => setPassphrase(e.target.value)}
              placeholder="Enter your passphrase"
              autoFocus
            />

            {error && (
              <div className="p-3 rounded-lg bg-red-50 dark:bg-red-900/20 text-red-600 dark:text-red-400 text-sm">
                {error}
              </div>
            )}

            <Button
              type="submit"
              className="w-full"
              size="lg"
              loading={loading}
            >
              Unlock
            </Button>
          </form>

          {/* Peer ID display */}
          {identity && (
            <div className="mt-6 p-4 rounded-lg bg-gray-50 dark:bg-gray-700/50">
              <p className="text-xs text-gray-500 dark:text-gray-400 mb-1">
                Your Peer ID
              </p>
              <p className="text-xs font-mono text-gray-700 dark:text-gray-300 break-all">
                {identity.peerId}
              </p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
