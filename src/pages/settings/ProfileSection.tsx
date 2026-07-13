import { useState, useRef, useEffect } from 'react';
import toast from 'react-hot-toast';
import { useIdentityStore, useSettingsStore } from '../../stores';
import { getInitials } from '../../utils/formatting';
import { SectionHeader, SettingsCard } from './shared';
import { safeIdentityLabel } from '../../utils/relayName';

export function ProfileSection() {
  const { state, updateBio } = useIdentityStore();
  const { avatarUrl, setAvatarUrl } = useSettingsStore();

  const [bio, setBio] = useState('');
  const [hasUnsavedChanges, setHasUnsavedChanges] = useState(false);
  const avatarInputRef = useRef<HTMLInputElement>(null);

  const identity = state.status === 'unlocked' ? state.identity : null;

  useEffect(() => {
    if (identity) {
      setBio(identity.bio || '');
    }
  }, [identity]);

  const handleAvatarUpload = () => {
    avatarInputRef.current?.click();
  };

  const handleAvatarChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    if (file.size > 5 * 1024 * 1024) {
      toast.error('Image must be less than 5MB');
      return;
    }

    const reader = new FileReader();
    reader.onload = () => {
      setAvatarUrl(String(reader.result));
      toast.success('Profile photo updated!');
    };
    reader.onerror = () => toast.error('Could not read that image');
    reader.readAsDataURL(file);

    if (avatarInputRef.current) {
      avatarInputRef.current.value = '';
    }
  };

  const handleCopyHarborName = () => {
    if (identity?.relayNameVerified && identity.relayNameClaim) {
      navigator.clipboard.writeText(safeIdentityLabel(identity));
      toast.success('Harbor name copied to clipboard!');
    } else {
      toast.error('Claim a verified Harbor name before sharing this account.');
    }
  };

  const handleSaveProfile = async () => {
    if (!identity) return;

    try {
      const trimmedBio = bio.trim() || null;

      if (trimmedBio !== identity.bio) {
        await updateBio(trimmedBio);
      }

      setHasUnsavedChanges(false);
      toast.success('Profile saved!');
    } catch {
      toast.error('Failed to save profile');
    }
  };

  return (
    <div className="space-y-6">
      <input
        ref={avatarInputRef}
        type="file"
        accept="image/*"
        onChange={handleAvatarChange}
        className="hidden"
      />

      <SectionHeader title="Profile" description="Manage your identity and how others see you" />

      {/* Avatar section */}
      <SettingsCard>
        <div className="flex items-center gap-6">
          {identity && (
            <div
              className="w-24 h-24 rounded-full flex items-center justify-center text-2xl font-semibold text-white flex-shrink-0 overflow-hidden"
              style={{
                background: avatarUrl ? 'transparent' : 'hsl(var(--harbor-surface-3))',
              }}
            >
              {avatarUrl ? (
                <img src={avatarUrl} alt="Avatar" className="w-full h-full object-cover" />
              ) : identity.avatarHash ? (
                <img
                  src={`/media/${identity.avatarHash}`}
                  alt="Avatar"
                  className="w-full h-full object-cover"
                />
              ) : (
                getInitials(safeIdentityLabel(identity))
              )}
            </div>
          )}
          <div className="flex-1">
            <h4 className="font-medium mb-2" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
              Profile Photo
            </h4>
            <p className="text-sm mb-3" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
              Upload a photo to personalize your profile
            </p>
            <div className="flex gap-2">
              <button
                onClick={handleAvatarUpload}
                className="px-4 py-2 rounded-lg text-sm font-medium transition-colors duration-200"
                style={{
                  background: 'hsl(var(--harbor-surface-1))',
                  color: 'hsl(var(--harbor-text-primary))',
                  border: '1px solid hsl(var(--harbor-border-subtle))',
                }}
              >
                Upload Photo
              </button>
              {avatarUrl && (
                <button
                  onClick={() => {
                    setAvatarUrl(null);
                    toast.success('Photo removed');
                  }}
                  className="px-4 py-2 rounded-lg text-sm font-medium transition-colors duration-200"
                  style={{
                    color: 'hsl(var(--harbor-error))',
                  }}
                >
                  Remove
                </button>
              )}
            </div>
          </div>
        </div>
      </SettingsCard>

      {/* Display name */}
      <SettingsCard>
        <label
          className="block text-sm font-medium mb-2"
          style={{ color: 'hsl(var(--harbor-text-primary))' }}
        >
          Verified Harbor name
        </label>
        <input
          type="text"
          value={identity ? safeIdentityLabel(identity) : ''}
          disabled
          className="w-full px-4 py-3 rounded-lg text-sm"
          style={{
            background: 'hsl(var(--harbor-surface-1))',
            border: '1px solid hsl(var(--harbor-border-subtle))',
            color: 'hsl(var(--harbor-text-primary))',
          }}
        />
      </SettingsCard>

      {/* Bio */}
      <SettingsCard>
        <label
          className="block text-sm font-medium mb-2"
          style={{ color: 'hsl(var(--harbor-text-primary))' }}
        >
          Bio
        </label>
        <textarea
          value={bio}
          onChange={(e) => {
            setBio(e.target.value);
            setHasUnsavedChanges(true);
          }}
          rows={5}
          placeholder="Tell others about yourself, your interests, what you're working on..."
          className="w-full px-4 py-3 rounded-lg text-sm resize-none"
          style={{
            background: 'hsl(var(--harbor-surface-1))',
            border: '1px solid hsl(var(--harbor-border-subtle))',
            color: 'hsl(var(--harbor-text-primary))',
          }}
        />
        <p className="text-xs mt-2" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
          This will be visible to your contacts
        </p>
      </SettingsCard>

      {/* Your unique ID */}
      <SettingsCard>
        <label
          className="block text-sm font-medium mb-2"
          style={{ color: 'hsl(var(--harbor-text-primary))' }}
        >
          Your Harbor name
        </label>
        <div className="flex gap-2">
          <div
            className="flex-1 px-4 py-3 rounded-lg text-sm font-mono truncate"
            style={{
              background: 'hsl(var(--harbor-surface-1))',
              border: '1px solid hsl(var(--harbor-border-subtle))',
              color: 'hsl(var(--harbor-text-secondary))',
            }}
          >
            {identity ? safeIdentityLabel(identity) : 'No identity'}
          </div>
          <button
            onClick={handleCopyHarborName}
            disabled={!identity?.relayNameVerified}
            className="px-4 py-3 rounded-lg text-sm font-medium transition-colors duration-200"
            style={{
              background: 'hsl(var(--harbor-surface-1))',
              color: 'hsl(var(--harbor-text-primary))',
              border: '1px solid hsl(var(--harbor-border-subtle))',
            }}
          >
            Copy
          </button>
        </div>
        <p className="text-xs mt-2" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
          This relay-qualified name is the public address people use to identify you.
        </p>
      </SettingsCard>

      {/* Save button */}
      <div className="flex justify-end">
        <button
          onClick={handleSaveProfile}
          disabled={!hasUnsavedChanges}
          className="px-6 py-3 rounded-lg text-sm font-medium transition-all duration-200 disabled:opacity-50 disabled:cursor-not-allowed"
          style={{
            background: hasUnsavedChanges
              ? 'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))'
              : 'hsl(var(--harbor-surface-2))',
            color: hasUnsavedChanges ? 'white' : 'hsl(var(--harbor-text-tertiary))',
            boxShadow: hasUnsavedChanges ? '0 4px 12px hsl(var(--harbor-primary) / 0.3)' : 'none',
          }}
        >
          Save Changes
        </button>
      </div>
    </div>
  );
}
