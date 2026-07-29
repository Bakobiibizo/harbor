import type { MessageContentState } from '../../types/messaging';
import { ShieldIcon } from '../icons';

type UnreadableMessageState = Exclude<MessageContentState, { kind: 'plaintext' }>;

const COPY: Record<
  Exclude<UnreadableMessageState['kind'], 'unsupported_version'>,
  { title: string; guidance: string }
> = {
  tampered: {
    title: 'Message authenticity check failed',
    guidance: 'This message may have changed after it was sent. Ask the sender to resend it.',
  },
  wrong_key: {
    title: 'Message key no longer matches',
    guidance: 'Re-establish this secure contact, then ask the sender to resend the message.',
  },
  corrupt_payload: {
    title: 'Message data is incomplete',
    guidance: 'Harbor could not read the complete message. Ask the sender to resend it.',
  },
};

export function MessageSecurityState({ state }: { state: UnreadableMessageState }) {
  const copy =
    state.kind === 'unsupported_version'
      ? {
          title: 'Update Harbor to read this message',
          guidance: `This message uses encryption format ${state.version}, which this version cannot open.`,
        }
      : COPY[state.kind];

  return (
    <div
      role="alert"
      data-testid={`message-security-${state.kind}`}
      className="flex gap-2.5 text-left"
      style={{ color: 'hsl(var(--harbor-text-primary))' }}
    >
      <ShieldIcon
        size={18}
        aria-hidden="true"
        className="mt-0.5 shrink-0"
        style={{ color: 'hsl(var(--harbor-warning))' }}
      />
      <div>
        <p className="text-sm font-semibold">{copy.title}</p>
        <p
          className="mt-0.5 text-xs leading-relaxed"
          style={{ color: 'hsl(var(--harbor-text-secondary))' }}
        >
          {copy.guidance}
        </p>
      </div>
    </div>
  );
}
