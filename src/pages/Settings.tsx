import { useState, useRef, useEffect } from 'react';
import toast from 'react-hot-toast';
import { useIdentityStore, useSettingsStore } from '../stores';
import {
  UserIcon,
  LockIcon,
  NetworkIcon,
  ShieldIcon,
  ChevronRightIcon,
  XIcon,
  PlusIcon,
  TrashIcon,
} from '../components/icons';

// Toggle component
function Toggle({ enabled, onChange }: { enabled: boolean; onChange: (v: boolean) => void }) {
  return (
    <button
      onClick={() => onChange(!enabled)}
      className="w-12 h-6 rounded-full relative transition-colors duration-200"
      style={{
        background: enabled ? 'hsl(var(--harbor-primary))' : 'hsl(var(--harbor-surface-2))',
      }}
    >
      <div
        className="w-5 h-5 rounded-full absolute top-0.5 transition-all duration-200"
        style={{
          background: 'white',
          left: enabled ? 'calc(100% - 22px)' : '2px',
        }}
      />
    </button>
  );
}

// Password input with reveal
function PasswordInput({
  placeholder,
  value,
  onChange,
}: {
  placeholder: string;
  value: string;
  onChange: (v: string) => void;
}) {
  const [show, setShow] = useState(false);

  return (
    <div className="relative">
      <input
        type={show ? 'text' : 'password'}
        placeholder={placeholder}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="w-full px-4 py-3 pr-12 rounded-lg text-sm"
        style={{
          background: 'hsl(var(--harbor-surface-1))',
          border: '1px solid hsl(var(--harbor-border-subtle))',
          color: 'hsl(var(--harbor-text-primary))',
        }}
      />
      <button
        type="button"
        onClick={() => setShow(!show)}
        className="absolute right-3 top-1/2 -translate-y-1/2 p-1"
        style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
      >
        {show ? (
          <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={1.5}
              d="M3.98 8.223A10.477 10.477 0 001.934 12C3.226 16.338 7.244 19.5 12 19.5c.993 0 1.953-.138 2.863-.395M6.228 6.228A10.45 10.45 0 0112 4.5c4.756 0 8.773 3.162 10.065 7.498a10.523 10.523 0 01-4.293 5.774M6.228 6.228L3 3m3.228 3.228l3.65 3.65m7.894 7.894L21 21m-3.228-3.228l-3.65-3.65m0 0a3 3 0 10-4.243-4.243m4.242 4.242L9.88 9.88"
            />
          </svg>
        ) : (
          <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={1.5}
              d="M2.036 12.322a1.012 1.012 0 010-.639C3.423 7.51 7.36 4.5 12 4.5c4.638 0 8.573 3.007 9.963 7.178.07.207.07.431 0 .639C20.577 16.49 16.64 19.5 12 19.5c-4.638 0-8.573-3.007-9.963-7.178z"
            />
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={1.5}
              d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
            />
          </svg>
        )}
      </button>
    </div>
  );
}

export function SettingsPage() {
  const { state, updateDisplayName, updateBio } = useIdentityStore();
  const {
    autoStartNetwork,
    localDiscovery,
    bootstrapNodes,
    showReadReceipts,
    showOnlineStatus,
    defaultVisibility,
    avatarUrl,
    setAutoStartNetwork,
    setLocalDiscovery,
    addBootstrapNode,
    removeBootstrapNode,
    setShowReadReceipts,
    setShowOnlineStatus,
    setDefaultVisibility,
    setAvatarUrl,
  } = useSettingsStore();

  const [activeSection, setActiveSection] = useState<string>('profile');

  // Bootstrap node state
  const [newBootstrapNode, setNewBootstrapNode] = useState('');
  const [bootstrapError, setBootstrapError] = useState('');

  // Profile edit state
  const [displayName, setDisplayName] = useState('');
  const [bio, setBio] = useState('');
  const [hasUnsavedChanges, setHasUnsavedChanges] = useState(false);
  const avatarInputRef = useRef<HTMLInputElement>(null);

  // Passphrase change state
  const [currentPass, setCurrentPass] = useState('');
  const [newPass, setNewPass] = useState('');
  const [confirmPass, setConfirmPass] = useState('');
  const [passError, setPassError] = useState('');
  const [isChangingPass, setIsChangingPass] = useState(false);

  // Delete account modal state
  const [showDeleteModal, setShowDeleteModal] = useState(false);
  const [deleteConfirmText, setDeleteConfirmText] = useState('');
  const [deletePassphrase, setDeletePassphrase] = useState('');
  const [deleteError, setDeleteError] = useState('');
  const [isDeleting, setIsDeleting] = useState(false);

  // Import identity state
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [importError, setImportError] = useState('');
  const [importPassphrase, setImportPassphrase] = useState('');
  const [showImportModal, setShowImportModal] = useState(false);
  const [importFile, setImportFile] = useState<File | null>(null);

  const identity = state.status === 'unlocked' ? state.identity : null;

  // Initialize form values when identity changes
  useEffect(() => {
    if (identity) {
      setDisplayName(identity.displayName);
      setBio(identity.bio || '');
    }
  }, [identity]);

  const getInitials = (name: string) => {
    return name
      .split(' ')
      .map((n) => n[0])
      .join('')
      .toUpperCase()
      .slice(0, 2);
  };

  const handleAvatarUpload = () => {
    avatarInputRef.current?.click();
  };

  const handleAvatarChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    // Check file size (max 5MB)
    if (file.size > 5 * 1024 * 1024) {
      toast.error('Image must be less than 5MB');
      return;
    }

    // Create object URL for preview
    const url = URL.createObjectURL(file);
    setAvatarUrl(url);
    toast.success('Profile photo updated!');

    // Reset file input
    if (avatarInputRef.current) {
      avatarInputRef.current.value = '';
    }
  };

  const handleCopyPeerId = () => {
    if (identity) {
      navigator.clipboard.writeText(identity.peerId);
      toast.success('Peer ID copied to clipboard!');
    }
  };

  const handleSaveProfile = async () => {
    if (!identity) return;

    try {
      const trimmedName = displayName.trim() || identity.displayName;
      const trimmedBio = bio.trim() || null;

      if (trimmedName !== identity.displayName) {
        await updateDisplayName(trimmedName);
      }

      if (trimmedBio !== identity.bio) {
        await updateBio(trimmedBio);
      }

      setHasUnsavedChanges(false);
      toast.success('Profile saved!');
    } catch {
      toast.error('Failed to save profile');
    }
  };

  const handlePassphraseChange = async () => {
    setPassError('');
    if (!currentPass || !newPass || !confirmPass) {
      setPassError('All fields are required');
      return;
    }
    if (newPass !== confirmPass) {
      setPassError('New passphrases do not match');
      return;
    }
    if (newPass.length < 8) {
      setPassError('Passphrase must be at least 8 characters');
      return;
    }

    setIsChangingPass(true);

    // Simulate passphrase change (in a real app, this would call the backend)
    await new Promise((resolve) => setTimeout(resolve, 1500));

    setIsChangingPass(false);
    setCurrentPass('');
    setNewPass('');
    setConfirmPass('');
    toast.success('Passphrase changed successfully!');
  };

  const handleExportIdentity = () => {
    if (!identity) return;

    // Create export data (encrypted identity blob)
    const exportData = {
      version: 1,
      type: 'harbor-identity-backup',
      peerId: identity.peerId,
      displayName: identity.displayName,
      bio: identity.bio,
      createdAt: new Date().toISOString(),
      encryptedKeys: 'ENCRYPTED_KEY_DATA_PLACEHOLDER',
      note: "Keep this file safe. You'll need your passphrase to restore it.",
    };

    // Create and download the file
    const blob = new Blob([JSON.stringify(exportData, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `harbor-backup-${identity.displayName.replace(/\s+/g, '-').toLowerCase()}-${new Date().toISOString().split('T')[0]}.json`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);

    toast.success('Backup exported! Keep it safe.');
  };

  const handleImportClick = () => {
    fileInputRef.current?.click();
  };

  const handleFileSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) {
      setImportFile(file);
      setShowImportModal(true);
      setImportError('');
      setImportPassphrase('');
    }
    // Reset the input
    e.target.value = '';
  };

  const handleImportIdentity = async () => {
    if (!importFile) return;

    if (!importPassphrase) {
      setImportError('Passphrase is required to decrypt the backup');
      return;
    }

    // Read and parse the file
    const reader = new FileReader();
    reader.onload = async (e) => {
      try {
        const data = JSON.parse(e.target?.result as string);

        if (data.type !== 'harbor-identity-backup') {
          setImportError('Invalid backup file format');
          return;
        }

        // Simulate recovery process
        await new Promise((resolve) => setTimeout(resolve, 2000));

        toast.success(`Account recovered! Welcome back, ${data.displayName}`);
        setShowImportModal(false);
        setImportFile(null);
        setImportPassphrase('');
      } catch {
        setImportError('Failed to parse backup file');
      }
    };
    reader.readAsText(importFile);
  };

  const handleDeleteIdentity = () => {
    setShowDeleteModal(true);
    setDeleteConfirmText('');
    setDeletePassphrase('');
    setDeleteError('');
  };

  const confirmDeleteIdentity = async () => {
    if (deleteConfirmText !== 'DELETE') {
      setDeleteError('Please type DELETE to confirm');
      return;
    }

    if (!deletePassphrase) {
      setDeleteError('Passphrase is required');
      return;
    }

    setIsDeleting(true);

    // Simulate deletion process
    await new Promise((resolve) => setTimeout(resolve, 2000));

    setIsDeleting(false);
    toast.success('Account deleted. Goodbye!');
    setShowDeleteModal(false);

    // In a real app, this would clear all data and redirect to onboarding
  };

  const handleOnlineStatusChange = (value: boolean) => {
    setShowOnlineStatus(value);
    toast.success(value ? 'Online status visible to contacts' : 'Online status hidden');
  };

  const handleAddBootstrapNode = () => {
    setBootstrapError('');
    const address = newBootstrapNode.trim();

    if (!address) {
      setBootstrapError('Please enter an address');
      return;
    }

    // Basic validation for multiaddress format
    if (!address.startsWith('/')) {
      setBootstrapError('Address must start with / (multiaddress format)');
      return;
    }

    // Check if it contains /p2p/ component
    if (!address.includes('/p2p/')) {
      setBootstrapError('Address must include peer ID (/p2p/...)');
      return;
    }

    // Check for duplicates
    if (bootstrapNodes.includes(address)) {
      setBootstrapError('This address is already in your list');
      return;
    }

    addBootstrapNode(address);
    setNewBootstrapNode('');
    toast.success('Bootstrap node added!');
  };

  const handleRemoveBootstrapNode = (address: string) => {
    removeBootstrapNode(address);
    toast.success('Bootstrap node removed');
  };

  const sections = [
    { id: 'profile', label: 'Profile', icon: UserIcon, description: 'Your identity and bio' },
    { id: 'security', label: 'Security', icon: LockIcon, description: 'Passphrase and keys' },
    { id: 'network', label: 'Network', icon: NetworkIcon, description: 'Connection settings' },
    { id: 'privacy', label: 'Privacy', icon: ShieldIcon, description: 'Visibility controls' },
  ];

  return (
    <div className="h-full flex" style={{ background: 'hsl(var(--harbor-bg-primary))' }}>
      {/* Hidden file inputs */}
      <input
        ref={avatarInputRef}
        type="file"
        accept="image/*"
        onChange={handleAvatarChange}
        className="hidden"
      />
      <input
        ref={fileInputRef}
        type="file"
        accept=".json"
        onChange={handleFileSelect}
        className="hidden"
      />

      {/* Settings sidebar - 33% width */}
      <div
        className="w-1/3 max-w-xs flex flex-col border-r flex-shrink-0"
        style={{
          borderColor: 'hsl(var(--harbor-border-subtle))',
          background: 'hsl(var(--harbor-bg-elevated))',
        }}
      >
        <div className="p-4 border-b" style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}>
          <h2 className="text-lg font-bold" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
            Settings
          </h2>
          <p className="text-sm mt-0.5" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
            Customize your experience
          </p>
        </div>

        <nav className="flex-1 p-2 space-y-1">
          {sections.map((section) => {
            const Icon = section.icon;
            const isActive = activeSection === section.id;

            return (
              <button
                key={section.id}
                onClick={() => setActiveSection(section.id)}
                className="w-full flex items-center gap-3 p-3 rounded-lg text-left transition-all duration-200"
                style={{
                  background: isActive
                    ? 'linear-gradient(135deg, hsl(var(--harbor-primary) / 0.15), hsl(var(--harbor-accent) / 0.1))'
                    : 'transparent',
                  border: isActive
                    ? '1px solid hsl(var(--harbor-primary) / 0.2)'
                    : '1px solid transparent',
                }}
              >
                <div
                  className="w-9 h-9 rounded-lg flex items-center justify-center"
                  style={{
                    background: isActive
                      ? 'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))'
                      : 'hsl(var(--harbor-surface-2))',
                  }}
                >
                  <Icon
                    className="w-5 h-5"
                    style={{
                      color: isActive ? 'white' : 'hsl(var(--harbor-text-secondary))',
                    }}
                  />
                </div>
                <div className="flex-1 min-w-0">
                  <p
                    className="font-medium text-sm"
                    style={{
                      color: isActive
                        ? 'hsl(var(--harbor-primary))'
                        : 'hsl(var(--harbor-text-primary))',
                    }}
                  >
                    {section.label}
                  </p>
                  <p
                    className="text-xs truncate"
                    style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                  >
                    {section.description}
                  </p>
                </div>
                <ChevronRightIcon
                  className="w-4 h-4"
                  style={{
                    color: isActive
                      ? 'hsl(var(--harbor-primary))'
                      : 'hsl(var(--harbor-text-tertiary))',
                  }}
                />
              </button>
            );
          })}
        </nav>

        {/* Version info */}
        <div className="p-4 border-t" style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}>
          <p className="text-xs text-center" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
            Harbor v1.0.0
          </p>
        </div>
      </div>

      {/* Settings content - 66% width */}
      <div className="flex-1 overflow-y-auto p-8">
        <div className="max-w-2xl">
          {activeSection === 'profile' && (
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

              {/* Avatar section */}
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
                    <h4
                      className="font-medium mb-2"
                      style={{ color: 'hsl(var(--harbor-text-primary))' }}
                    >
                      Profile Photo
                    </h4>
                    <p
                      className="text-sm mb-3"
                      style={{ color: 'hsl(var(--harbor-text-secondary))' }}
                    >
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

              {/* Bio - now 5 lines tall */}
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
              </div>

              {/* Your unique ID */}
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
                    onClick={handleCopyPeerId}
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
                  Share this ID with others so they can add you as a contact
                </p>
              </div>

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
                    boxShadow: hasUnsavedChanges
                      ? '0 4px 12px hsl(var(--harbor-primary) / 0.3)'
                      : 'none',
                  }}
                >
                  Save Changes
                </button>
              </div>
            </div>
          )}

          {activeSection === 'security' && (
            <div className="space-y-6">
              <div>
                <h3
                  className="text-xl font-semibold mb-1"
                  style={{ color: 'hsl(var(--harbor-text-primary))' }}
                >
                  Security
                </h3>
                <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                  Manage your passphrase and encryption keys
                </p>
              </div>

              {/* Change passphrase */}
              <div
                className="rounded-lg p-6"
                style={{
                  background: 'hsl(var(--harbor-bg-elevated))',
                  border: '1px solid hsl(var(--harbor-border-subtle))',
                }}
              >
                <h4
                  className="font-medium mb-2"
                  style={{ color: 'hsl(var(--harbor-text-primary))' }}
                >
                  Change Passphrase
                </h4>
                <p className="text-sm mb-4" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                  Update your passphrase to keep your identity secure
                </p>

                <div className="space-y-3">
                  <PasswordInput
                    placeholder="Current passphrase"
                    value={currentPass}
                    onChange={setCurrentPass}
                  />
                  <PasswordInput
                    placeholder="New passphrase"
                    value={newPass}
                    onChange={setNewPass}
                  />
                  <PasswordInput
                    placeholder="Confirm new passphrase"
                    value={confirmPass}
                    onChange={setConfirmPass}
                  />
                </div>

                {passError && (
                  <p className="text-sm mt-2" style={{ color: 'hsl(var(--harbor-error))' }}>
                    {passError}
                  </p>
                )}

                <button
                  onClick={handlePassphraseChange}
                  disabled={isChangingPass}
                  className="mt-4 px-4 py-2 rounded-lg text-sm font-medium transition-colors duration-200 disabled:opacity-50"
                  style={{
                    background:
                      'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
                    color: 'white',
                  }}
                >
                  {isChangingPass ? 'Updating...' : 'Update Passphrase'}
                </button>
              </div>

              {/* Backup & Recovery */}
              <div
                className="rounded-lg p-6"
                style={{
                  background: 'hsl(var(--harbor-bg-elevated))',
                  border: '1px solid hsl(var(--harbor-border-subtle))',
                }}
              >
                <h4
                  className="font-medium mb-2"
                  style={{ color: 'hsl(var(--harbor-text-primary))' }}
                >
                  Backup & Recovery
                </h4>
                <p className="text-sm mb-4" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                  Export your identity to create a backup, or import an existing backup to recover
                  your account
                </p>

                <div className="flex gap-3">
                  <button
                    onClick={handleExportIdentity}
                    className="flex-1 px-4 py-3 rounded-lg text-sm font-medium transition-colors duration-200 flex items-center justify-center gap-2"
                    style={{
                      background:
                        'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
                      color: 'white',
                    }}
                  >
                    <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        strokeWidth={2}
                        d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12"
                      />
                    </svg>
                    Export Backup
                  </button>
                  <button
                    onClick={handleImportClick}
                    className="flex-1 px-4 py-3 rounded-lg text-sm font-medium transition-colors duration-200 flex items-center justify-center gap-2"
                    style={{
                      background: 'hsl(var(--harbor-surface-1))',
                      color: 'hsl(var(--harbor-text-primary))',
                      border: '1px solid hsl(var(--harbor-border-subtle))',
                    }}
                  >
                    <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        strokeWidth={2}
                        d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4"
                      />
                    </svg>
                    Recover Account
                  </button>
                </div>

                <p className="text-xs mt-3" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                  Your backup file is encrypted with your passphrase. Keep it safe and never share
                  it.
                </p>
              </div>

              {/* Danger zone */}
              <div
                className="rounded-lg p-6"
                style={{
                  background: 'hsl(var(--harbor-error) / 0.05)',
                  border: '1px solid hsl(var(--harbor-error) / 0.2)',
                }}
              >
                <h4 className="font-medium mb-2" style={{ color: 'hsl(var(--harbor-error))' }}>
                  Delete Account
                </h4>
                <p className="text-sm mb-4" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                  Permanently delete your identity, messages, posts, and all associated data. This
                  action cannot be undone.
                </p>
                <button
                  onClick={handleDeleteIdentity}
                  className="px-4 py-2 rounded-lg text-sm font-medium transition-colors duration-200"
                  style={{
                    background: 'hsl(var(--harbor-error) / 0.15)',
                    color: 'hsl(var(--harbor-error))',
                    border: '1px solid hsl(var(--harbor-error) / 0.3)',
                  }}
                >
                  Delete Account
                </button>
              </div>
            </div>
          )}

          {activeSection === 'network' && (
            <div className="space-y-6">
              <div>
                <h3
                  className="text-xl font-semibold mb-1"
                  style={{ color: 'hsl(var(--harbor-text-primary))' }}
                >
                  Network
                </h3>
                <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                  Configure connection settings
                </p>
              </div>

              <div
                className="rounded-lg p-6"
                style={{
                  background: 'hsl(var(--harbor-bg-elevated))',
                  border: '1px solid hsl(var(--harbor-border-subtle))',
                }}
              >
                <div className="flex items-center justify-between">
                  <div>
                    <h4
                      className="font-medium"
                      style={{ color: 'hsl(var(--harbor-text-primary))' }}
                    >
                      Auto-connect on startup
                    </h4>
                    <p
                      className="text-sm mt-0.5"
                      style={{ color: 'hsl(var(--harbor-text-secondary))' }}
                    >
                      Automatically connect to the network when app starts
                    </p>
                  </div>
                  <Toggle enabled={autoStartNetwork} onChange={setAutoStartNetwork} />
                </div>
              </div>

              <div
                className="rounded-lg p-6"
                style={{
                  background: 'hsl(var(--harbor-bg-elevated))',
                  border: '1px solid hsl(var(--harbor-border-subtle))',
                }}
              >
                <div className="flex items-center justify-between">
                  <div>
                    <h4
                      className="font-medium"
                      style={{ color: 'hsl(var(--harbor-text-primary))' }}
                    >
                      Local network discovery
                    </h4>
                    <p
                      className="text-sm mt-0.5"
                      style={{ color: 'hsl(var(--harbor-text-secondary))' }}
                    >
                      Automatically find other Harbor users on your local network
                    </p>
                  </div>
                  <Toggle enabled={localDiscovery} onChange={setLocalDiscovery} />
                </div>
              </div>

              {/* Bootstrap Nodes */}
              <div
                className="rounded-lg p-6"
                style={{
                  background: 'hsl(var(--harbor-bg-elevated))',
                  border: '1px solid hsl(var(--harbor-border-subtle))',
                }}
              >
                <h4
                  className="font-medium mb-2"
                  style={{ color: 'hsl(var(--harbor-text-primary))' }}
                >
                  Bootstrap Nodes
                </h4>
                <p className="text-sm mb-4" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                  Bootstrap nodes help you connect to remote peers outside your local network. These
                  will be automatically connected when you start the network.
                </p>

                {/* Add new bootstrap node */}
                <div className="flex gap-2 mb-4">
                  <input
                    type="text"
                    value={newBootstrapNode}
                    onChange={(e) => {
                      setNewBootstrapNode(e.target.value);
                      setBootstrapError('');
                    }}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') {
                        handleAddBootstrapNode();
                      }
                    }}
                    placeholder="/ip4/1.2.3.4/tcp/9000/p2p/12D3KooW..."
                    className="flex-1 px-4 py-3 rounded-lg text-sm font-mono"
                    style={{
                      background: 'hsl(var(--harbor-surface-1))',
                      border: '1px solid hsl(var(--harbor-border-subtle))',
                      color: 'hsl(var(--harbor-text-primary))',
                    }}
                  />
                  <button
                    onClick={handleAddBootstrapNode}
                    className="px-4 py-3 rounded-lg text-sm font-medium transition-colors duration-200 flex items-center gap-2"
                    style={{
                      background:
                        'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
                      color: 'white',
                    }}
                  >
                    <PlusIcon size={16} />
                    Add
                  </button>
                </div>

                {bootstrapError && (
                  <p className="text-sm mb-4" style={{ color: 'hsl(var(--harbor-error))' }}>
                    {bootstrapError}
                  </p>
                )}

                {/* List of bootstrap nodes */}
                {bootstrapNodes.length === 0 ? (
                  <div
                    className="text-sm text-center py-6 rounded-lg"
                    style={{
                      background: 'hsl(var(--harbor-surface-1))',
                      color: 'hsl(var(--harbor-text-tertiary))',
                    }}
                  >
                    No bootstrap nodes configured. Add one to connect to remote peers.
                  </div>
                ) : (
                  <div className="space-y-2">
                    {bootstrapNodes.map((address, index) => (
                      <div
                        key={index}
                        className="flex items-center gap-2 p-3 rounded-lg group"
                        style={{
                          background: 'hsl(var(--harbor-surface-1))',
                          border: '1px solid hsl(var(--harbor-border-subtle))',
                        }}
                      >
                        <div
                          className="flex-1 text-sm font-mono truncate"
                          style={{ color: 'hsl(var(--harbor-text-secondary))' }}
                          title={address}
                        >
                          {address}
                        </div>
                        <button
                          onClick={() => handleRemoveBootstrapNode(address)}
                          className="p-2 rounded-lg transition-colors duration-200 opacity-0 group-hover:opacity-100"
                          style={{ color: 'hsl(var(--harbor-error))' }}
                          title="Remove bootstrap node"
                        >
                          <TrashIcon size={16} />
                        </button>
                      </div>
                    ))}
                  </div>
                )}

                <p className="text-xs mt-4" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                  Get bootstrap node addresses from friends or public Harbor community servers.
                </p>
              </div>
            </div>
          )}

          {activeSection === 'privacy' && (
            <div className="space-y-6">
              <div>
                <h3
                  className="text-xl font-semibold mb-1"
                  style={{ color: 'hsl(var(--harbor-text-primary))' }}
                >
                  Privacy
                </h3>
                <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                  Control who can see your content
                </p>
              </div>

              <div
                className="rounded-lg p-6"
                style={{
                  background: 'hsl(var(--harbor-bg-elevated))',
                  border: '1px solid hsl(var(--harbor-border-subtle))',
                }}
              >
                <h4
                  className="font-medium mb-2"
                  style={{ color: 'hsl(var(--harbor-text-primary))' }}
                >
                  Default post visibility
                </h4>
                <p className="text-sm mb-4" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                  Who can see your new posts by default
                </p>
                <select
                  value={defaultVisibility}
                  onChange={(e) => setDefaultVisibility(e.target.value as 'contacts' | 'public')}
                  className="w-full px-4 py-3 rounded-lg text-sm"
                  style={{
                    background: 'hsl(var(--harbor-surface-1))',
                    border: '1px solid hsl(var(--harbor-border-subtle))',
                    color: 'hsl(var(--harbor-text-primary))',
                  }}
                >
                  <option value="contacts">Contacts only</option>
                  <option value="public">Anyone with the link</option>
                </select>
              </div>

              <div
                className="rounded-lg p-6"
                style={{
                  background: 'hsl(var(--harbor-bg-elevated))',
                  border: '1px solid hsl(var(--harbor-border-subtle))',
                }}
              >
                <div className="flex items-center justify-between">
                  <div>
                    <h4
                      className="font-medium"
                      style={{ color: 'hsl(var(--harbor-text-primary))' }}
                    >
                      Message read notifications
                    </h4>
                    <p
                      className="text-sm mt-0.5"
                      style={{ color: 'hsl(var(--harbor-text-secondary))' }}
                    >
                      Let others know when you've read their messages
                    </p>
                  </div>
                  <Toggle enabled={showReadReceipts} onChange={setShowReadReceipts} />
                </div>
              </div>

              <div
                className="rounded-lg p-6"
                style={{
                  background: 'hsl(var(--harbor-bg-elevated))',
                  border: '1px solid hsl(var(--harbor-border-subtle))',
                }}
              >
                <div className="flex items-center justify-between">
                  <div>
                    <h4
                      className="font-medium"
                      style={{ color: 'hsl(var(--harbor-text-primary))' }}
                    >
                      Show online status
                    </h4>
                    <p
                      className="text-sm mt-0.5"
                      style={{ color: 'hsl(var(--harbor-text-secondary))' }}
                    >
                      Show when you're online to your contacts
                    </p>
                  </div>
                  <Toggle enabled={showOnlineStatus} onChange={handleOnlineStatusChange} />
                </div>
              </div>
            </div>
          )}
        </div>
      </div>

      {/* Delete Account Modal */}
      {showDeleteModal && (
        <div
          className="fixed inset-0 flex items-center justify-center z-50 p-4"
          style={{ background: 'rgba(0, 0, 0, 0.6)' }}
        >
          <div
            className="w-full max-w-md rounded-lg overflow-hidden"
            style={{
              background: 'hsl(var(--harbor-bg-elevated))',
              border: '1px solid hsl(var(--harbor-border-subtle))',
            }}
          >
            {/* Modal header */}
            <div
              className="px-6 py-4 flex items-center justify-between border-b"
              style={{
                borderColor: 'hsl(var(--harbor-border-subtle))',
                background: 'hsl(var(--harbor-error) / 0.05)',
              }}
            >
              <h3 className="text-lg font-semibold" style={{ color: 'hsl(var(--harbor-error))' }}>
                Delete Account
              </h3>
              <button
                onClick={() => setShowDeleteModal(false)}
                className="p-1 rounded-lg transition-colors duration-200"
                style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
              >
                <XIcon className="w-5 h-5" />
              </button>
            </div>

            {/* Modal body */}
            <div className="p-6 space-y-4">
              <div
                className="p-4 rounded-lg"
                style={{
                  background: 'hsl(var(--harbor-error) / 0.1)',
                  border: '1px solid hsl(var(--harbor-error) / 0.2)',
                }}
              >
                <p
                  className="text-sm font-medium mb-2"
                  style={{ color: 'hsl(var(--harbor-error))' }}
                >
                  Warning: This action is irreversible
                </p>
                <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                  Deleting your account will permanently remove:
                </p>
                <ul
                  className="text-sm mt-2 ml-4 space-y-1 list-disc"
                  style={{ color: 'hsl(var(--harbor-text-secondary))' }}
                >
                  <li>Your identity and cryptographic keys</li>
                  <li>All messages and conversations</li>
                  <li>All posts and media</li>
                  <li>Your contacts and permissions</li>
                </ul>
              </div>

              <div>
                <label
                  className="block text-sm font-medium mb-2"
                  style={{ color: 'hsl(var(--harbor-text-primary))' }}
                >
                  Type <span style={{ color: 'hsl(var(--harbor-error))' }}>DELETE</span> to confirm
                </label>
                <input
                  type="text"
                  value={deleteConfirmText}
                  onChange={(e) => setDeleteConfirmText(e.target.value)}
                  placeholder="Type DELETE"
                  className="w-full px-4 py-3 rounded-lg text-sm"
                  style={{
                    background: 'hsl(var(--harbor-surface-1))',
                    border: '1px solid hsl(var(--harbor-border-subtle))',
                    color: 'hsl(var(--harbor-text-primary))',
                  }}
                />
              </div>

              <div>
                <label
                  className="block text-sm font-medium mb-2"
                  style={{ color: 'hsl(var(--harbor-text-primary))' }}
                >
                  Enter your passphrase
                </label>
                <PasswordInput
                  placeholder="Your passphrase"
                  value={deletePassphrase}
                  onChange={setDeletePassphrase}
                />
              </div>

              {deleteError && (
                <p className="text-sm" style={{ color: 'hsl(var(--harbor-error))' }}>
                  {deleteError}
                </p>
              )}
            </div>

            {/* Modal footer */}
            <div
              className="px-6 py-4 flex gap-3 border-t"
              style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
            >
              <button
                onClick={() => setShowDeleteModal(false)}
                className="flex-1 px-4 py-3 rounded-lg text-sm font-medium transition-colors duration-200"
                style={{
                  background: 'hsl(var(--harbor-surface-1))',
                  color: 'hsl(var(--harbor-text-primary))',
                  border: '1px solid hsl(var(--harbor-border-subtle))',
                }}
              >
                Cancel
              </button>
              <button
                onClick={confirmDeleteIdentity}
                disabled={deleteConfirmText !== 'DELETE' || isDeleting}
                className="flex-1 px-4 py-3 rounded-lg text-sm font-medium transition-colors duration-200 disabled:opacity-50 disabled:cursor-not-allowed"
                style={{
                  background: 'hsl(var(--harbor-error))',
                  color: 'white',
                }}
              >
                {isDeleting ? 'Deleting...' : 'Delete Account Forever'}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Import/Recover Modal */}
      {showImportModal && (
        <div
          className="fixed inset-0 flex items-center justify-center z-50 p-4"
          style={{ background: 'rgba(0, 0, 0, 0.6)' }}
        >
          <div
            className="w-full max-w-md rounded-lg overflow-hidden"
            style={{
              background: 'hsl(var(--harbor-bg-elevated))',
              border: '1px solid hsl(var(--harbor-border-subtle))',
            }}
          >
            {/* Modal header */}
            <div
              className="px-6 py-4 flex items-center justify-between border-b"
              style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
            >
              <h3
                className="text-lg font-semibold"
                style={{ color: 'hsl(var(--harbor-text-primary))' }}
              >
                Recover Account
              </h3>
              <button
                onClick={() => {
                  setShowImportModal(false);
                  setImportFile(null);
                }}
                className="p-1 rounded-lg transition-colors duration-200"
                style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
              >
                <XIcon className="w-5 h-5" />
              </button>
            </div>

            {/* Modal body */}
            <div className="p-6 space-y-4">
              {/* Explanation section */}
              <div
                className="p-4 rounded-lg"
                style={{
                  background: 'hsl(var(--harbor-surface-1))',
                  border: '1px solid hsl(var(--harbor-border-subtle))',
                }}
              >
                <h4
                  className="text-sm font-medium mb-2"
                  style={{ color: 'hsl(var(--harbor-text-primary))' }}
                >
                  What is account recovery?
                </h4>
                <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                  If you previously exported a backup of your Harbor identity, you can use it to
                  restore your account on this device. Your backup file contains your encrypted
                  cryptographic keys.
                </p>
              </div>

              <div
                className="p-4 rounded-lg"
                style={{
                  background: 'hsl(var(--harbor-primary) / 0.1)',
                  border: '1px solid hsl(var(--harbor-primary) / 0.2)',
                }}
              >
                <p
                  className="text-sm font-medium mb-1"
                  style={{ color: 'hsl(var(--harbor-primary))' }}
                >
                  Selected backup file
                </p>
                <p
                  className="text-sm truncate"
                  style={{ color: 'hsl(var(--harbor-text-secondary))' }}
                >
                  {importFile?.name}
                </p>
              </div>

              <div>
                <label
                  className="block text-sm font-medium mb-2"
                  style={{ color: 'hsl(var(--harbor-text-primary))' }}
                >
                  Enter backup passphrase
                </label>
                <p className="text-sm mb-3" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                  Enter the passphrase you used when you created this backup. This will decrypt your
                  identity keys.
                </p>
                <PasswordInput
                  placeholder="Backup passphrase"
                  value={importPassphrase}
                  onChange={setImportPassphrase}
                />
              </div>

              {importError && (
                <p className="text-sm" style={{ color: 'hsl(var(--harbor-error))' }}>
                  {importError}
                </p>
              )}

              <p className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                Note: Recovering an account will replace your current identity if you have one.
              </p>
            </div>

            {/* Modal footer */}
            <div
              className="px-6 py-4 flex gap-3 border-t"
              style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
            >
              <button
                onClick={() => {
                  setShowImportModal(false);
                  setImportFile(null);
                }}
                className="flex-1 px-4 py-3 rounded-lg text-sm font-medium transition-colors duration-200"
                style={{
                  background: 'hsl(var(--harbor-surface-1))',
                  color: 'hsl(var(--harbor-text-primary))',
                  border: '1px solid hsl(var(--harbor-border-subtle))',
                }}
              >
                Cancel
              </button>
              <button
                onClick={handleImportIdentity}
                className="flex-1 px-4 py-3 rounded-lg text-sm font-medium transition-colors duration-200"
                style={{
                  background:
                    'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
                  color: 'white',
                }}
              >
                Recover Account
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
