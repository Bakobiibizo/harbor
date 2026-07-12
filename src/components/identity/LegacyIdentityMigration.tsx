import { useEffect, useState, type ReactNode } from 'react';
import { identityService } from '../../services';
import type { IdentityInfo, RelayNameClaim } from '../../types';
import { qualifiedRelayName } from '../../types';
import { Button, Input } from '../common';
import { useIdentityStore } from '../../stores';
import { configuredRelayNamespace, validateRelayLocalName } from '../../utils/relayNameInput';
import { publishingPolicy } from '../../services/publishingPolicy';

export function LegacyIdentityMigration({
  identity,
  children,
}: {
  identity: IdentityInfo;
  children: ReactNode;
}) {
  const attachVerifiedRelayName = useIdentityStore((state) => state.attachVerifiedRelayName);
  const [claim, setClaim] = useState<RelayNameClaim | null>(identity.relayNameClaim ?? null);
  const [checked, setChecked] = useState(false);
  const [compatible, setCompatible] = useState(false);
  const [name, setName] = useState(
    identity.displayName
      .toLowerCase()
      .replace(/[^a-z0-9_-]/g, '')
      .slice(0, 32),
  );
  const [namespace] = useState(configuredRelayNamespace);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let active = true;
    Promise.all([identityService.getLocalNameClaim(), identityService.getMigrationState()])
      .then(async ([local, mode]) => {
        setCompatible(mode === 'compatibility');
        publishingPolicy.setMode(mode);
        if (
          active &&
          local?.request.peerId === identity.peerId &&
          (await identityService.verifyNameClaim(local))
        ) {
          setClaim(local);
          attachVerifiedRelayName(local);
          await identityService.setMigrationMode('verified');
          publishingPolicy.setMode('verified');
        }
      })
      .catch((err) => setError(err instanceof Error ? err.message : String(err)))
      .finally(() => {
        if (active) setChecked(true);
      });
    return () => {
      active = false;
    };
  }, [attachVerifiedRelayName, identity.peerId]);

  if (!checked)
    return <div className="min-h-screen grid place-items-center">Checking your Harbor name…</div>;
  if (claim || compatible) return <>{children}</>;

  async function register() {
    setBusy(true);
    setError(null);
    try {
      const validation = validateRelayLocalName(name);
      if (validation) throw new Error(validation);
      if (!namespace) throw new Error('Connect to or configure a relay before claiming a name.');
      const next = await identityService.registerRelayName({ name, namespace });
      if (next.request.peerId !== identity.peerId || !(await identityService.verifyNameClaim(next)))
        throw new Error('The relay returned a claim Harbor could not verify.');
      setClaim(next);
      attachVerifiedRelayName(next);
      await identityService.setMigrationMode('verified');
      publishingPolicy.setMode('verified');
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <main
      className="min-h-screen grid place-items-center p-6"
      style={{ background: 'hsl(var(--harbor-bg-base))' }}
    >
      <section
        className="w-full max-w-lg rounded-2xl p-8 space-y-5"
        style={{
          background: 'hsl(var(--harbor-bg-elevated))',
          border: '1px solid hsl(var(--harbor-border-subtle))',
        }}
      >
        <div>
          <h1 className="text-2xl font-bold">Choose your verified Harbor name</h1>
          <p className="mt-2 text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
            Your old name, “{identity.displayName}”, is only an unverified migration hint. Claim a
            relay-unique address so people can tell identities apart.
          </p>
        </div>
        <Input
          label="Name"
          value={name}
          onChange={(e) => setName(e.target.value.toLowerCase().replace(/[^a-z0-9_-]/g, ''))}
        />
        <Input label="Relay namespace" value={namespace} disabled />
        <p className="text-sm">
          Your address will be{' '}
          <strong>
            {qualifiedRelayName({ name: name || 'name', namespace: namespace || 'relay' })}
          </strong>
          .
        </p>
        {error && (
          <div
            role="alert"
            className="rounded-lg p-3 text-sm"
            style={{ color: 'hsl(var(--harbor-error))' }}
          >
            {error} If the name is taken, choose another. If offline, reconnect and retry.
          </div>
        )}
        <Button className="w-full" disabled={busy || !name || !namespace} onClick={register}>
          {busy ? 'Claiming name…' : 'Claim this name'}
        </Button>
        <button
          className="w-full text-sm underline"
          onClick={async () => {
            try {
              await identityService.setMigrationMode('compatibility');
              publishingPolicy.setMode('compatibility');
              setCompatible(true);
            } catch (err) {
              setError(err instanceof Error ? err.message : String(err));
            }
          }}
        >
          Continue in beta compatibility mode
        </button>
        <p className="text-xs" style={{ color: 'hsl(var(--harbor-warning))' }}>
          Compatibility mode preserves your peer ID, keys, contacts, and history, but your old name
          stays unverified. Publishing remains beta-only until you claim a name.
        </p>
      </section>
    </main>
  );
}
