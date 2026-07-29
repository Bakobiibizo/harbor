import { useEffect, useState } from 'react';
import toast from 'react-hot-toast';
import { XIcon } from '../icons';
import { Button } from './Button';
import { addContactFromString } from '../../services/network';
import { identityService } from '../../services/identity';
import { safePeerLabel } from '../../utils/relayName';
import { parseContactInvite } from '../../utils/contactInvite';
import { qualifiedRelayName, type RelayNameClaim } from '../../types';
import { HARBOR_SHORTCUT_EVENTS } from '../../hooks/useKeyboardNavigation';
import { getErrorMessage } from '../../utils/errors';

interface Props {
  contactString: string;
  onClose: () => void;
}

interface ContactPreview {
  displayName: string;
  peerId: string;
  bio?: string;
  relayNameClaim?: RelayNameClaim;
}

function parseContactString(contactString: string): ContactPreview | null {
  try {
    const bundle = parseContactInvite(contactString);
    return {
      displayName: bundle.displayName,
      peerId: bundle.peerId,
      bio: bundle.bio ?? undefined,
      relayNameClaim: bundle.relayNameClaim,
    };
  } catch {
    return null;
  }
}

export function AddContactDialog({ contactString, onClose }: Props) {
  const [isLoading, setIsLoading] = useState(false);
  const [verifiedQualifiedName, setVerifiedQualifiedName] = useState<string | null>(null);
  const preview = parseContactString(contactString);

  useEffect(() => {
    window.addEventListener(HARBOR_SHORTCUT_EVENTS.escape, onClose);
    return () => window.removeEventListener(HARBOR_SHORTCUT_EVENTS.escape, onClose);
  }, [onClose]);

  useEffect(() => {
    let current = true;
    setVerifiedQualifiedName(null);
    const claim = preview?.relayNameClaim;
    if (!claim) {
      return () => {
        current = false;
      };
    }
    void identityService.verifyNameClaim(claim).then(
      (verified) => {
        if (current && verified) setVerifiedQualifiedName(qualifiedRelayName(claim));
      },
      (error) => {
        if (current) console.warn('Could not verify relay name claim from contact invite:', error);
      },
    );
    return () => {
      current = false;
    };
  }, [contactString]);

  async function handleConfirm() {
    setIsLoading(true);
    try {
      await addContactFromString(contactString);
      toast.success(
        'Contact request sent. Keys and sharing access will be added after acceptance.',
      );
      onClose();
    } catch (err) {
      toast.error(`Failed to add contact: ${getErrorMessage(err)}`);
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
            {preview
              ? 'Send this person a contact request? Their keys and sharing access are added only after they accept.'
              : 'This contact invite is malformed or no longer supported.'}
          </p>

          <div
            className="rounded-lg p-4 space-y-2"
            style={{ background: 'hsl(var(--harbor-surface-1))' }}
          >
            <p
              className="font-semibold text-base"
              style={{ color: 'hsl(var(--harbor-text-primary))' }}
            >
              {preview
                ? safePeerLabel(preview.peerId, verifiedQualifiedName, preview.displayName)
                : 'Unknown contact'}
            </p>
            {preview?.bio && (
              <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                {preview.bio}
              </p>
            )}
            <p className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
              {verifiedQualifiedName
                ? 'Relay-qualified name verified against your pinned relay key.'
                : 'Harbor will verify this person’s relay-qualified name before displaying it.'}
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
          <Button
            variant="primary"
            size="sm"
            loading={isLoading}
            disabled={isLoading || !preview}
            onClick={handleConfirm}
          >
            Send Request
          </Button>
        </div>
      </div>
    </div>
  );
}
