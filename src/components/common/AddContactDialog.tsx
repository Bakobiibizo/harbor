import { useState } from 'react';
import toast from 'react-hot-toast';
import { XIcon } from '../icons';
import { Button } from './Button';
import { addContactFromString } from '../../services/network';

interface Props {
  contactString: string;
  onClose: () => void;
}

interface ContactPreview {
  displayName: string;
  peerId: string;
  bio?: string;
}

function parseContactString(contactString: string): ContactPreview | null {
  try {
    const base64 = contactString.replace('harbor://', '');
    // URL-safe base64 uses - and _ instead of + and /; atob needs standard base64 with padding
    const b64 = base64.replace(/-/g, '+').replace(/_/g, '/');
    const padded = b64 + '='.repeat((4 - (b64.length % 4)) % 4);
    const json = atob(padded);
    const bundle = JSON.parse(json);
    // Peer ID is the last segment of the multiaddr: /ip4/.../p2p/<peer_id>
    const parts: string[] = bundle.multiaddr.split('/');
    const peerId = parts[parts.length - 1];
    return {
      displayName: bundle.displayName,
      peerId,
      bio: bundle.bio ?? undefined,
    };
  } catch {
    return null;
  }
}

export function AddContactDialog({ contactString, onClose }: Props) {
  const [isLoading, setIsLoading] = useState(false);
  const preview = parseContactString(contactString);

  async function handleConfirm() {
    setIsLoading(true);
    try {
      await addContactFromString(contactString);
      // contact_added event in useTauriEvents handles refreshContacts() and success toast
      onClose();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : 'Failed to add contact');
    } finally {
      setIsLoading(false);
    }
  }

  return (
    <div
      className="fixed inset-0 flex items-center justify-center z-50 p-4"
      style={{ background: 'rgba(0, 0, 0, 0.6)' }}
      onClick={onClose}
    >
      <div
        className="w-full max-w-md rounded-lg overflow-hidden"
        style={{
          background: 'hsl(var(--harbor-bg-elevated))',
          border: '1px solid hsl(var(--harbor-border-subtle))',
        }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div
          className="px-6 py-4 flex items-center justify-between border-b"
          style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
        >
          <h3
            className="text-lg font-semibold"
            style={{ color: 'hsl(var(--harbor-text-primary))' }}
          >
            Add Contact
          </h3>
          <button
            onClick={onClose}
            className="p-1 rounded-lg transition-colors duration-200"
            style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
          >
            <XIcon className="w-5 h-5" />
          </button>
        </div>

        {/* Content */}
        <div className="px-6 py-5 space-y-4">
          <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
            Do you want to add this person to your contacts?
          </p>

          <div
            className="rounded-lg p-4 space-y-2"
            style={{ background: 'hsl(var(--harbor-surface-1))' }}
          >
            <p
              className="font-semibold text-base"
              style={{ color: 'hsl(var(--harbor-text-primary))' }}
            >
              {preview?.displayName ?? 'Unknown contact'}
            </p>
            {preview?.bio && (
              <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                {preview.bio}
              </p>
            )}
            <p
              className="text-xs font-mono break-all"
              style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
            >
              {preview?.peerId ?? '—'}
            </p>
          </div>
        </div>

        {/* Footer */}
        <div
          className="px-6 py-4 flex justify-end gap-3 border-t"
          style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
        >
          <Button variant="secondary" size="sm" onClick={onClose} disabled={isLoading}>
            Cancel
          </Button>
          <Button variant="primary" size="sm" loading={isLoading} onClick={handleConfirm}>
            Add Contact
          </Button>
        </div>
      </div>
    </div>
  );
}
