import type { RelayNamePresentation } from '../../types';

export function VerifiedRelayName({ name }: { name: RelayNamePresentation }) {
  const verified = name.trust === 'verified';
  const warning =
    name.trust === 'expired'
      ? 'claim expired'
      : name.trust === 'untrusted'
        ? 'untrusted relay'
        : 'legacy name';
  return (
    <span
      className="inline-flex items-center gap-1"
      title={verified ? 'Verified by this relay' : 'Not relay verified'}
    >
      <span>{name.label}</span>
      {verified ? (
        <span aria-label="Relay verified" style={{ color: 'hsl(var(--harbor-success))' }}>
          ✓
        </span>
      ) : (
        <span className="text-xs" style={{ color: 'hsl(var(--harbor-warning))' }}>
          {warning}
        </span>
      )}
    </span>
  );
}
