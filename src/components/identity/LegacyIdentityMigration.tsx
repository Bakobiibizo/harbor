import { useEffect, useState, type ReactNode } from 'react';
import { identityService } from '../../services';
import type { IdentityInfo, RelayNameClaim } from '../../types';
import { Button, Input } from '../common';
import { useIdentityStore } from '../../stores';
import {
  configuredRelayNamespace,
  relayAddressPreview,
  validateRelayLocalName,
} from '../../utils/relayNameInput';
import { publishingPolicy } from '../../services/publishingPolicy';
import { HarborIcon } from '../icons';

export function LegacyIdentityMigration({
  identity,
  children,
}: {
  identity: IdentityInfo;
  children: ReactNode;
}) {
  const attachVerifiedRelayName = useIdentityStore((state) => state.attachVerifiedRelayName);
  const completeOnboarding = useIdentityStore((state) => state.completeOnboarding);
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
      const completed = await completeOnboarding(
        {
          displayName: identity.displayName,
          relayName: name,
          relayNamespace: namespace,
          // Existing identities are already unlocked before this gate. The store never uses this
          // placeholder unless it has to create a missing identity, which this migration path does
          // not permit.
          passphrase: 'existing-unlocked-identity',
          bio: identity.bio ?? undefined,
          passphraseHint: identity.passphraseHint ?? undefined,
        },
        name,
        namespace,
      );
      const next = completed.relayNameClaim;
      if (!next) throw new Error('Harbor completed registration without a verified name claim.');
      setClaim(next);
      attachVerifiedRelayName(next);
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
      style={{
        background: 'linear-gradient(145deg, hsl(216 70% 10%), hsl(216 58% 17%))',
        color: 'white',
      }}
    >
      <section
        className="w-full max-w-lg rounded-2xl p-8 space-y-5"
        style={{
          background: 'hsl(216 52% 16%)',
          border: '1px solid hsl(210 40% 92% / .22)',
          boxShadow: '0 24px 60px hsl(216 80% 4% / .45)',
        }}
      >
        <div className="flex items-center gap-4">
          <HarborIcon size={64} alt="Harbor" className="shrink-0" />
          <div>
            <h1 className="text-2xl font-bold">Choose your verified Harbor name</h1>
            <p className="mt-2 text-sm" style={{ color: 'hsl(210 40% 92%)' }}>
              Your old name, “{identity.displayName}”, is only an unverified migration hint. Claim a
              relay-unique address so people can tell identities apart.
            </p>
          </div>
        </div>
        <Input
          label="Name"
          value={name}
          onChange={(e) => setName(e.target.value.toLowerCase().replace(/[^a-z0-9_-]/g, ''))}
        />
        <Input label="Relay namespace" value={namespace} disabled />
        {relayAddressPreview(name || 'name', namespace) ? (
          <p className="text-sm">
            Your address will be <strong>{relayAddressPreview(name || 'name', namespace)}</strong>.
          </p>
        ) : (
          <p role="status" className="text-sm" style={{ color: 'hsl(var(--harbor-warning))' }}>
            No relay namespace is configured. Harbor will not invent or display an address.
          </p>
        )}
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
