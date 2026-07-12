import { useEffect, useState } from 'react';
import { mentionsService } from '../../services';
import type { ResolvedMention } from '../../types';
import { extractQualifiedMentions } from '../../utils/mentions';

export function MentionResolution({
  text,
  onResolved,
}: {
  text: string;
  onResolved: (mentions: ResolvedMention[]) => void;
}) {
  const [mentions, setMentions] = useState<ResolvedMention[]>([]);
  useEffect(() => {
    let active = true;
    const names = extractQualifiedMentions(text);
    Promise.all(
      names.map(async (qualifiedName) => {
        try {
          return await mentionsService.resolve(qualifiedName);
        } catch {
          return { qualifiedName, status: 'unknown' as const };
        }
      }),
    ).then((next) => {
      if (active) {
        setMentions(next);
        onResolved(next);
      }
    });
    return () => {
      active = false;
    };
  }, [text, onResolved]);
  if (!mentions.length) return null;
  return (
    <div className="mt-2 flex flex-wrap gap-2" aria-label="Mention recipients">
      {mentions.map((mention) => (
        <span
          key={mention.qualifiedName}
          className="rounded-full px-2 py-1 text-xs"
          style={{
            background: 'hsl(var(--harbor-surface-1))',
            color:
              mention.status === 'blocked'
                ? 'hsl(var(--harbor-error))'
                : 'hsl(var(--harbor-text-secondary))',
          }}
        >
          {mention.qualifiedName} ·{' '}
          {mention.status === 'known'
            ? 'contact'
            : mention.status === 'private'
              ? 'private introduction'
              : mention.status}
        </span>
      ))}
    </div>
  );
}
