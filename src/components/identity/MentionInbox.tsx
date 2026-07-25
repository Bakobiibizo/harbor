import { useEffect, useState } from 'react';
import toast from 'react-hot-toast';
import { mentionsService } from '../../services';
import type { MentionReceipt } from '../../types';
import { useContactsStore } from '../../stores';
import { safePeerLabel } from '../../utils/relayName';
import { getErrorMessage } from '../../utils/errors';

export function MentionInbox() {
  const contacts = useContactsStore((state) => state.contacts);
  const [items, setItems] = useState<MentionReceipt[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [reviewing, setReviewing] = useState<string | null>(null);
  useEffect(() => {
    mentionsService
      .listPending()
      .then(setItems)
      .catch((err) => setError(getErrorMessage(err)));
  }, []);
  async function decide(
    id: string,
    decision: 'accept-notification' | 'accept-repost' | 'decline' | 'block',
  ) {
    setReviewing(id);
    setError(null);
    try {
      await mentionsService.review(id, decision);
      setItems((all) => all.filter((item) => item.mentionId !== id));
      toast.success('Mention preference saved');
    } catch (err) {
      setError(getErrorMessage(err));
    } finally {
      setReviewing(null);
    }
  }
  if (!items.length && !error) return null;
  return (
    <section className="space-y-3">
      <h4 className="font-medium">Mentions awaiting review</h4>
      {error && (
        <p role="alert" style={{ color: 'hsl(var(--harbor-error))' }}>
          {error}
        </p>
      )}
      {items.map((item) => (
        <article
          key={item.mentionId}
          className="rounded-lg p-4"
          style={{ border: '1px solid hsl(var(--harbor-border-subtle))' }}
        >
          <strong>
            {(() => {
              const contact = contacts.find((entry) => entry.peerId === item.senderPeerId);
              return safePeerLabel(
                item.senderPeerId,
                contact?.verifiedQualifiedName,
                contact?.displayName,
              );
            })()}
          </strong>
          <p className="text-sm my-2">{item.preview}</p>
          <div className="flex flex-wrap gap-2">
            <button
              disabled={reviewing === item.mentionId}
              onClick={() => decide(item.mentionId, 'accept-notification')}
            >
              Accept notification
            </button>
            {item.intent === 'repost-request' && (
              <button
                disabled={reviewing === item.mentionId}
                onClick={() => decide(item.mentionId, 'accept-repost')}
              >
                Repost on my profile
              </button>
            )}
            <button onClick={() => decide(item.mentionId, 'decline')}>Decline</button>
            <button onClick={() => decide(item.mentionId, 'block')}>Block</button>
          </div>
        </article>
      ))}
    </section>
  );
}
