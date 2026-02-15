import { useState } from 'react';
import toast from 'react-hot-toast';
import { checkForUpdate, downloadAndInstallUpdate } from '../../services/updater';
import type { UpdateInfo } from '../../services/updater';
import { SectionHeader, SettingsCard } from './shared';

export function AboutSection() {
  const [isCheckingUpdate, setIsCheckingUpdate] = useState(false);
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [updateError, setUpdateError] = useState('');
  const [isUpdating, setIsUpdating] = useState(false);

  const handleCheckForUpdate = async () => {
    setIsCheckingUpdate(true);
    setUpdateError('');
    setUpdateInfo(null);

    try {
      const info = await checkForUpdate();
      setUpdateInfo(info);

      if (info.available) {
        toast.success(`Update available: v${info.version}`);
      } else {
        toast.success('You are running the latest version!');
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Failed to check for updates';
      setUpdateError(message);
      toast.error(message);
    } finally {
      setIsCheckingUpdate(false);
    }
  };

  const handleInstallUpdate = async () => {
    setIsUpdating(true);
    setUpdateError('');

    try {
      await downloadAndInstallUpdate();
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Failed to install update';
      setUpdateError(message);
      toast.error(message);
    } finally {
      setIsUpdating(false);
    }
  };

  return (
    <div className="space-y-6">
      <SectionHeader title="About" description="App version, updates, and links" />

      {/* Current version */}
      <SettingsCard>
        <h4 className="font-medium mb-2" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
          Current Version
        </h4>
        <p
          className="text-2xl font-mono font-semibold"
          style={{ color: 'hsl(var(--harbor-primary))' }}
        >
          v0.1.0
        </p>
        <p className="text-sm mt-2" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
          Installed on this device
        </p>
      </SettingsCard>

      {/* Check for updates */}
      <SettingsCard>
        <h4 className="font-medium mb-2" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
          Check for Updates
        </h4>
        <p className="text-sm mb-4" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
          Updates are downloaded from the official Harbor GitHub releases
        </p>

        {updateError && (
          <div
            className="mb-4 p-3 rounded-lg text-sm"
            style={{
              background: 'hsl(var(--harbor-error) / 0.1)',
              border: '1px solid hsl(var(--harbor-error) / 0.2)',
              color: 'hsl(var(--harbor-error))',
            }}
          >
            {updateError}
          </div>
        )}

        {updateInfo && (
          <div
            className="mb-4 p-4 rounded-lg"
            style={{
              background: updateInfo.available
                ? 'hsl(var(--harbor-success) / 0.1)'
                : 'hsl(var(--harbor-surface-1))',
              border: updateInfo.available
                ? '1px solid hsl(var(--harbor-success) / 0.2)'
                : '1px solid hsl(var(--harbor-border-subtle))',
            }}
          >
            {updateInfo.available ? (
              <>
                <div className="flex items-center gap-2 mb-2">
                  <svg
                    className="w-5 h-5"
                    fill="none"
                    viewBox="0 0 24 24"
                    stroke="currentColor"
                    style={{ color: 'hsl(var(--harbor-success))' }}
                  >
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={2}
                      d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"
                    />
                  </svg>
                  <span className="font-medium" style={{ color: 'hsl(var(--harbor-success))' }}>
                    Update Available!
                  </span>
                </div>
                <p className="text-sm mb-1" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
                  Version <span className="font-mono font-semibold">v{updateInfo.version}</span>
                </p>
                {updateInfo.date && (
                  <p className="text-xs mb-2" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                    Released: {new Date(updateInfo.date).toLocaleDateString()}
                  </p>
                )}
                {updateInfo.body && (
                  <div
                    className="mt-3 p-3 rounded text-sm"
                    style={{
                      background: 'hsl(var(--harbor-surface-1))',
                      color: 'hsl(var(--harbor-text-secondary))',
                    }}
                  >
                    <p
                      className="font-medium mb-1"
                      style={{ color: 'hsl(var(--harbor-text-primary))' }}
                    >
                      Release Notes:
                    </p>
                    <p className="whitespace-pre-wrap">{updateInfo.body}</p>
                  </div>
                )}
              </>
            ) : (
              <div className="flex items-center gap-2">
                <svg
                  className="w-5 h-5"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                  style={{ color: 'hsl(var(--harbor-text-secondary))' }}
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M5 13l4 4L19 7"
                  />
                </svg>
                <span style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                  You're running the latest version
                </span>
              </div>
            )}
          </div>
        )}

        <div className="flex gap-3">
          <button
            onClick={handleCheckForUpdate}
            disabled={isCheckingUpdate}
            className="px-4 py-3 rounded-lg text-sm font-medium transition-colors duration-200 disabled:opacity-50 flex items-center gap-2"
            style={{
              background: 'hsl(var(--harbor-surface-1))',
              color: 'hsl(var(--harbor-text-primary))',
              border: '1px solid hsl(var(--harbor-border-subtle))',
            }}
          >
            {isCheckingUpdate ? (
              <>
                <svg className="w-4 h-4 animate-spin" viewBox="0 0 24 24" fill="none">
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
                Checking...
              </>
            ) : (
              <>
                <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
                  />
                </svg>
                Check for Updates
              </>
            )}
          </button>

          {updateInfo?.available && (
            <button
              onClick={handleInstallUpdate}
              disabled={isUpdating}
              className="px-4 py-3 rounded-lg text-sm font-medium transition-all duration-200 disabled:opacity-50 flex items-center gap-2"
              style={{
                background:
                  'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
                color: 'white',
                boxShadow: '0 4px 12px hsl(var(--harbor-primary) / 0.3)',
              }}
            >
              {isUpdating ? (
                <>
                  <svg className="w-4 h-4 animate-spin" viewBox="0 0 24 24" fill="none">
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
                  Installing...
                </>
              ) : (
                <>
                  <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={2}
                      d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4"
                    />
                  </svg>
                  Install Update
                </>
              )}
            </button>
          )}
        </div>
      </SettingsCard>

      {/* About Harbor */}
      <SettingsCard>
        <h4 className="font-medium mb-2" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
          About Harbor
        </h4>
        <p className="text-sm mb-3" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
          A decentralized, peer-to-peer social platform built with privacy in mind.
        </p>
        <a
          href="https://github.com/bakobiibizo/harbor"
          target="_blank"
          rel="noopener noreferrer"
          className="inline-flex items-center gap-2 text-sm transition-colors duration-200"
          style={{ color: 'hsl(var(--harbor-primary))' }}
        >
          <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 24 24">
            <path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z" />
          </svg>
          View on GitHub
        </a>
      </SettingsCard>
    </div>
  );
}
