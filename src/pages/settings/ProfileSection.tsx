import { useState, useRef, useEffect } from 'react';
import toast from 'react-hot-toast';
import { useIdentityStore, useSettingsStore } from '../../stores';
import { getInitials } from '../../utils/formatting';

export function ProfileSection() {
  const { state, updateDisplayName, updateBio } = useIdentityStore();
  const { avatarUrl, setAvatarUrl } = useSettingsStore();

  const [displayName, setDisplayName] = useState('');
  const [bio, setBio] = useState('');
  const [hasUnsavedChanges, setHasUnsavedChanges] = useState(false);
  const avatarInputRef = useRef<HTMLInputElement>(null);

  const identity = state.status === 'unlocked' ? state.identity : null;

  useEffect(() => {
    if (identity) {
      setDisplayName(identity.displayName);
      setBio(identity.bio || '');
    }
  }, [identity]);

  const handleAvatarChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    if (file.size > 5 * 1024 * 1024) {
      toast.error('Image must be less than 5MB');
      return;
    }
    const url = URL.createObjectURL(file);
    setAvatarUrl(url);
    toast.success('Profile photo updated!');
    if (avatarInputRef.current) avatarInputRef.current.value = '';
  };

  const handleSaveProfile = async () => {
    if (!identity) return;
    try {
      const trimmedName = displayName.trim() || identity.displayName;
      const trimmedBio = bio.trim() || null;
      if (trimmedName !== identity.displayName) await updateDisplayName(trimmedName);
      if (trimmedBio !== identity.bio) await updateBio(trimmedBio);
      setHasUnsavedChanges(false);
      toast.success('Profile saved!');
    } catch {
      toast.error('Failed to save profile');
    }
  };

  return (
    <div className="space-y-6">
      <div>
        <h3
          className="text-xl font-semibold mb-1"
          style={{ color: 'hsl(var(--harbor-text-primary))' }}
        >
          Profile
        </h3>
        <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
          Manage your identity and how others see you
        </p>
      </div>

      <input
        ref={avatarInputRef}
        type="file"
        accept="image/*"
        onChange={handleAvatarChange}
        className="hidden"
      />

      {/* Avatar */}
      <div
        className="rounded-lg p-6"
        style={{
          background: 'hsl(var(--harbor-bg-elevated))',
          border: '1px solid hsl(var(--harbor-border-subtle))',
        }}
      >
        <div className="flex items-center gap-6">
          {identity && (
            <div
              className="w-24 h-24 rounded-full flex items-center justify-center text-2xl font-semibold text-white flex-shrink-0 overflow-hidden"
              style={{
                background: avatarUrl
                  ? 'transparent'
                  : 'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
              }}
            >
              {avatarUrl ? (
                <img src={avatarUrl} alt="Avatar" className="w-full h-full object-cover" />
              ) : (
                getInitials(identity.displayName)
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
                onClick={() => avatarInputRef.current?.click()}
                className="px-4 py-2 rounded-lg text-sm font-medium"
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
                  className="px-4 py-2 rounded-lg text-sm font-medium"
                  style={{ color: 'hsl(var(--harbor-error))' }}
                >
                  Remove
                </button>
              )}
            </div>
          </div>
        </div>
      </div>

      {/* Display name */}
      <div
        className="rounded-lg p-6"
        style={{
          background: 'hsl(var(--harbor-bg-elevated))',
          border: '1px solid hsl(var(--harbor-border-subtle))',
        }}
      >
        <label
          className="block text-sm font-medium mb-2"
          style={{ color: 'hsl(var(--harbor-text-primary))' }}
        >
          Display Name
        </label>
        <input
          type="text"
          value={displayName}
          onChange={(e) => {
            setDisplayName(e.target.value);
            setHasUnsavedChanges(true);
          }}
          className="w-full px-4 py-3 rounded-lg text-sm"
          style={{
            background: 'hsl(var(--harbor-surface-1))',
            border: '1px solid hsl(var(--harbor-border-subtle))',
            color: 'hsl(var(--harbor-text-primary))',
          }}
        />
      </div>

      {/* Bio */}
      <div
        className="rounded-lg p-6"
        style={{
          background: 'hsl(var(--harbor-bg-elevated))',
          border: '1px solid hsl(var(--harbor-border-subtle))',
        }}
      >
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
          placeholder="Tell others about yourself..."
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
      </div>

      {/* Unique ID */}
      <div
        className="rounded-lg p-6"
        style={{
          background: 'hsl(var(--harbor-bg-elevated))',
          border: '1px solid hsl(var(--harbor-border-subtle))',
        }}
      >
        <label
          className="block text-sm font-medium mb-2"
          style={{ color: 'hsl(var(--harbor-text-primary))' }}
        >
          Your Unique ID
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
            {identity?.peerId || 'No identity'}
          </div>
          <button
            onClick={() => {
              if (identity) {
                navigator.clipboard.writeText(identity.peerId);
                toast.success('Peer ID copied!');
              }
            }}
            className="px-4 py-3 rounded-lg text-sm font-medium"
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
          Share this ID with others so they can add you as a contact
        </p>
      </div>

      {/* Save */}
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
