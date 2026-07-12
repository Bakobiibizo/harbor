import { useEffect, useState } from 'react';
import toast from 'react-hot-toast';
import { mentionsService } from '../../services';
import type { MentionReceipt } from '../../types';

export function MentionInbox() {
  const [items, setItems] = useState<MentionReceipt[]>([]);
  useEffect(() => {
    mentionsService
      .listPending()
      .then(setItems)
      .catch(() => undefined);
  }, []);
  async function decide(
    id: string,
    decision: 'accept-notification' | 'accept-repost' | 'decline' | 'block',
  ) {
    await mentionsService.review(id, decision);
    setItems((all) => all.filter((item) => item.mentionId !== id));
    toast.success('Mention preference saved');
  }
  if (!items.length) return null;
  return (
    <section className="space-y-3">
      <h4 className="font-medium">Mentions awaiting review</h4>
      {items.map((item) => (
        <article
          key={item.mentionId}
          className="rounded-lg p-4"
          style={{ border: '1px solid hsl(var(--harbor-border-subtle))' }}
        >
          <strong>{item.qualifiedName}</strong>
          <p className="text-sm my-2">{item.preview}</p>
          <div className="flex flex-wrap gap-2">
            <button onClick={() => decide(item.mentionId, 'accept-notification')}>
              Accept notification
            </button>
            {item.intent === 'repost-request' && (
              <button onClick={() => decide(item.mentionId, 'accept-repost')}>
                Repost on my wall
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
