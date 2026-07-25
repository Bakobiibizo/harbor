import { useState } from 'react';
import { mentionsService } from '../../services';
import { Button } from '../common';
import { namedWallPath } from '../../utils/namedWall';
import { getErrorMessage } from '../../utils/errors';

export const HARBOR_BUGS_NAME = import.meta.env.VITE_HARBOR_BUGS_NAME || '@bugs@harbor.social';

export function BugReportForm() {
  const [summary, setSummary] = useState('');
  const [details, setDetails] = useState('');
  const [busy, setBusy] = useState(false);
  const [submitted, setSubmitted] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function submit() {
    setBusy(true);
    setError(null);
    try {
      const target = await mentionsService.resolve(HARBOR_BUGS_NAME);
      if (target.status === 'blocked')
        throw new Error('Bug reports are currently unavailable from this relay.');
      await mentionsService.publish({
        contentType: 'text',
        visibility: 'public',
        contentText: `Bug report: ${summary}\n\n${details}\n\n${HARBOR_BUGS_NAME}`,
        mentions: [
          {
            qualifiedName: HARBOR_BUGS_NAME,
            intent: 'repost-request',
            authorizedPeerId: target.status === 'known' ? target.peerId : undefined,
            claimDigest: target.claimDigest,
          },
        ],
      });
      // Navigation is name-based. Never expose the resolved peer ID or accept an
      // arbitrary tracking URI returned across the command boundary.
      setSubmitted(true);
      setSummary('');
      setDetails('');
    } catch (err) {
      setError(getErrorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  if (submitted) {
    const trackingPath = namedWallPath(HARBOR_BUGS_NAME);
    return (
      <div className="rounded-lg p-6" style={{ background: 'hsl(var(--harbor-success) / .1)' }}>
        <h4 className="font-semibold">Bug report submitted</h4>
        <p className="text-sm my-2">
          The Harbor Bugs account can review and repost it without gaining access to your contacts
          or private content.
        </p>
        <a className="underline" href={`#${trackingPath}`}>
          Track this report on {HARBOR_BUGS_NAME}’s profile
        </a>
        <button className="block mt-4 text-sm underline" onClick={() => setSubmitted(false)}>
          Report another bug
        </button>
      </div>
    );
  }
  return (
    <div
      className="rounded-lg p-6 space-y-4"
      style={{
        background: 'hsl(var(--harbor-bg-elevated))',
        border: '1px solid hsl(var(--harbor-border-subtle))',
      }}
    >
      <div>
        <h4 className="font-medium">Report a bug</h4>
        <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
          This publishes a signed report tagging {HARBOR_BUGS_NAME}. Nothing appears on its profile
          unless that account approves the repost.
        </p>
      </div>
      <input
        aria-label="Bug summary"
        value={summary}
        onChange={(e) => setSummary(e.target.value)}
        placeholder="Short summary"
        className="w-full px-4 py-3 rounded-lg"
        style={{ background: 'hsl(var(--harbor-surface-1))' }}
      />
      <textarea
        aria-label="Bug details"
        value={details}
        onChange={(e) => setDetails(e.target.value)}
        placeholder="What happened, and what did you expect?"
        rows={6}
        className="w-full px-4 py-3 rounded-lg"
        style={{ background: 'hsl(var(--harbor-surface-1))' }}
      />
      {error && (
        <p role="alert" style={{ color: 'hsl(var(--harbor-error))' }}>
          {error}
        </p>
      )}
      <Button disabled={busy || !summary.trim() || !details.trim()} onClick={submit}>
        {busy ? 'Submitting…' : 'Submit signed bug report'}
      </Button>
    </div>
  );
}
