import { useState } from 'react';
import type { ContactRequest } from '../../types';
import { safePeerLabel } from '../../utils/relayName';
import { Button } from './Button';

const STATUS_LABEL: Record<ContactRequest['status'], string> = {
  pending: 'Pending',
  review: 'Needs review',
  accepted: 'Accepted',
  declined: 'Declined',
  failed: 'Failed',
  revoked: 'Revoked',
};

interface Props {
  requests: ContactRequest[];
  onDecision: (requestId: string, decision: 'accepted' | 'declined') => Promise<void>;
  onRetry: (requestId: string) => Promise<void>;
}

export function ContactRequestsPanel({ requests, onDecision, onRetry }: Props) {
  const [pendingRequestId, setPendingRequestId] = useState<string | null>(null);
  const decide = async (requestId: string, decision: 'accepted' | 'declined') => {
    if (pendingRequestId) return;
    setPendingRequestId(requestId);
    try {
      await onDecision(requestId, decision);
    } catch {
      // The owning screen reports the command error. This component only owns
      // pending interaction state and must always restore the review controls.
    } finally {
      setPendingRequestId(null);
    }
  };
  if (requests.length === 0) return null;
  return (
    <section
      aria-labelledby="contact-requests-heading"
      className="rounded-2xl p-6 space-y-3"
      style={{
        background: 'hsl(var(--harbor-bg-elevated))',
        border: '1px solid hsl(var(--harbor-border-subtle))',
      }}
    >
      <div className="flex items-center justify-between gap-3">
        <h3 id="contact-requests-heading" className="font-semibold">
          Contact requests
        </h3>
        <span className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
          {requests.filter((request) => request.status === 'review').length} awaiting review
        </span>
      </div>
      {requests.map((request) => {
        const label = safePeerLabel(request.peerId, undefined, request.displayName);
        return (
          <article
            key={`${request.direction}:${request.requestId}`}
            className="rounded-xl p-4 flex flex-wrap items-center gap-3"
            style={{ background: 'hsl(var(--harbor-surface-1))' }}
          >
            <div className="min-w-0 flex-1">
              <p className="font-medium truncate">{label}</p>
              <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                {request.direction === 'incoming' ? 'Incoming' : 'Outgoing'} ·{' '}
                {STATUS_LABEL[request.status]}
              </p>
              {request.error && (
                <p
                  role="alert"
                  className="text-xs mt-1"
                  style={{ color: 'hsl(var(--harbor-error))' }}
                >
                  {request.error}
                </p>
              )}
              <details className="text-xs mt-2">
                <summary className="cursor-pointer">Inspect request</summary>
                <p className="mt-1" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                  {request.status === 'review'
                    ? 'Accept only if you recognize this person. Harbor never accepts contact requests automatically.'
                    : `This request is ${STATUS_LABEL[request.status].toLowerCase()}.`}
                </p>
              </details>
            </div>
            {request.direction === 'incoming' && request.status === 'review' && (
              <div className="flex gap-2">
                <Button
                  size="sm"
                  variant="secondary"
                  disabled={pendingRequestId !== null}
                  onClick={() => void decide(request.requestId, 'declined')}
                >
                  Decline
                </Button>
                <Button
                  size="sm"
                  disabled={pendingRequestId !== null}
                  onClick={() => void decide(request.requestId, 'accepted')}
                >
                  {pendingRequestId === request.requestId ? 'Saving...' : 'Accept'}
                </Button>
              </div>
            )}
            {request.status === 'failed' && (
              <Button size="sm" variant="secondary" onClick={() => onRetry(request.requestId)}>
                Retry
              </Button>
            )}
          </article>
        );
      })}
    </section>
  );
}
