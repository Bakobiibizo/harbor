import { useEffect, useState } from 'react';
import toast from 'react-hot-toast';
import type { GameSigningRequest, IdentityInfo } from '../../types';
import { approveGameSigningRequest } from '../../services/games';
import { getErrorMessage } from '../../utils/errors';
import { HARBOR_SHORTCUT_EVENTS } from '../../hooks/useKeyboardNavigation';
import { Button } from '../common/Button';
import { XIcon } from '../icons';

interface Props {
  identity: IdentityInfo;
  request: GameSigningRequest;
  onClose: () => void;
}

export function GameSigningApprovalDialog({ identity, request, onClose }: Props) {
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    window.addEventListener(HARBOR_SHORTCUT_EVENTS.escape, onClose);
    return () => window.removeEventListener(HARBOR_SHORTCUT_EVENTS.escape, onClose);
  }, [onClose]);

  async function approve() {
    setSubmitting(true);
    try {
      const delivery = await approveGameSigningRequest(request.approvalId);
      if (delivery.status === 'delivered') {
        toast.success(
          request.kind === 'auth' ? 'Harbor sign-in approved' : 'Game package signature delivered',
        );
        onClose();
      } else {
        toast.error(`${delivery.error}. The signed proof is retained; retry to deliver it.`);
      }
    } catch (error) {
      toast.error(getErrorMessage(error));
    } finally {
      setSubmitting(false);
    }
  }

  const expiresAt = new Date(request.expiresAt * 1000).toLocaleString();

  return (
    <div
      className="fixed inset-0 z-[70] flex items-center justify-center p-4"
      style={{ background: 'rgba(0, 0, 0, 0.72)' }}
      role="presentation"
      onClick={onClose}
    >
      <section
        aria-labelledby="game-signing-title"
        aria-modal="true"
        className="w-full max-w-xl overflow-hidden rounded-xl"
        role="dialog"
        style={{
          background: 'hsl(var(--harbor-bg-elevated))',
          border: '1px solid hsl(var(--harbor-border-subtle))',
        }}
        onClick={(event) => event.stopPropagation()}
      >
        <header
          className="flex items-center justify-between border-b px-6 py-4"
          style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
        >
          <div>
            <p className="text-xs font-medium uppercase tracking-wide text-cyan-400">
              Neo Grounds request
            </p>
            <h2
              id="game-signing-title"
              className="text-lg font-semibold"
              style={{ color: 'hsl(var(--harbor-text-primary))' }}
            >
              {request.kind === 'auth' ? 'Sign in with Harbor' : 'Sign game package'}
            </h2>
          </div>
          <button
            aria-label="Cancel signing"
            className="rounded-lg p-1"
            disabled={submitting}
            onClick={onClose}
            style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
          >
            <XIcon className="h-5 w-5" />
          </button>
        </header>

        <div className="space-y-4 px-6 py-5">
          <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
            {request.kind === 'auth'
              ? 'Approve linking this Harbor public identity to the named Neo Grounds account.'
              : 'Approve creator authorship for this exact finalized package digest. This does not approve the game for the store.'}
          </p>

          <dl
            className="grid grid-cols-[minmax(8rem,auto)_1fr] gap-x-4 gap-y-3 rounded-lg p-4 text-sm"
            style={{ background: 'hsl(var(--harbor-surface-1))' }}
          >
            <Field label="Harbor identity" value={identity.displayName} />
            <Field label="Peer ID" value={identity.peerId} mono />
            {request.kind === 'auth' ? (
              <>
                <Field label="Neo Grounds account" value={request.accountId} />
                <Field label="Origin" value={request.audience} mono />
                <Field label="Challenge" value={request.challengeId} mono />
              </>
            ) : (
              <>
                <Field label="Game" value={request.title} />
                <Field label="Game ID" value={request.gameId} mono />
                <Field label="Version" value={request.versionId} mono />
                <Field label="Package SHA-256" value={request.packageDigest} mono />
                <Field
                  label="Permissions"
                  value={request.permissions.length ? request.permissions.join(', ') : 'None'}
                />
              </>
            )}
            <Field label="Request ID" value={request.requestId} mono />
            <Field label="Expires" value={expiresAt} />
          </dl>

          <p className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
            Harbor signs only this structured, domain-separated request. Your private identity key
            stays inside Harbor.
          </p>
        </div>

        <footer
          className="flex justify-end gap-3 border-t px-6 py-4"
          style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
        >
          <Button variant="secondary" size="sm" disabled={submitting} onClick={onClose}>
            Cancel
          </Button>
          <Button size="sm" loading={submitting} disabled={submitting} onClick={approve}>
            {request.kind === 'auth' ? 'Approve Sign-in' : 'Sign Package'}
          </Button>
        </footer>
      </section>
    </div>
  );
}

function Field({ label, value, mono = false }: { label: string; value: string; mono?: boolean }) {
  return (
    <>
      <dt style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>{label}</dt>
      <dd
        className={`${mono ? 'break-all font-mono text-xs' : ''}`}
        style={{ color: 'hsl(var(--harbor-text-primary))' }}
      >
        {value}
      </dd>
    </>
  );
}
