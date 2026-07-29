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
import type { IdentityClaimProgress } from '../../stores/identity';
import { getErrorMessage, HarborError } from '../../utils/errors';

const progressLabels: Record<IdentityClaimProgress, string> = {
  preparing: 'Preparing your identity…',
  connecting: 'Connecting to Harbor…',
  'waiting-for-relay': 'Waiting for the relay…',
  registering: 'Claiming your name…',
  verifying: 'Verifying the signed claim…',
  saving: 'Saving your Harbor name…',
};

export const IDENTITY_VERIFICATION_REQUEST_EVENT = 'harbor:request-identity-verification';

export function requestIdentityVerification() {
  window.dispatchEvent(new Event(IDENTITY_VERIFICATION_REQUEST_EVENT));
}

function getNameClaimErrorMessage(error: unknown): string {
  const harborError = HarborError.fromUnknown(error);
  const detail = harborError.details?.replace(/^Network error:\s*/i, '').trim();

  if (detail === 'NAME_REGISTRATION_REJECTED') {
    return 'The relay could not grant this name. It may already be claimed or reserved by another account.';
  }
  if (detail === 'NAME_REGISTRATION_IN_PROGRESS') {
    return 'Harbor is still finishing the previous name request. Wait a moment, then retry.';
  }
  if (detail?.startsWith('Name registration timed out')) return detail;
  if (detail?.startsWith('Name registration failed while contacting the relay')) return detail;
  if (detail?.startsWith('RELAY_AUTH_') || detail?.startsWith('AUTH_')) {
    return 'The relay could not authenticate this account. Reconnect to the relay and retry.';
  }
  return getErrorMessage(error);
}

export function IdentityPublishingGate({
  identity,
  children,
}: {
  identity: IdentityInfo;
  children: ReactNode;
}) {
  const attachVerifiedRelayName = useIdentityStore((state) => state.attachVerifiedRelayName);
  const completeOnboarding = useIdentityStore((state) => state.completeOnboarding);
  const [claim, setClaim] = useState<RelayNameClaim | null>(identity.relayNameClaim ?? null);
  const restoredVerifiedClaim = identity.relayNameVerified === true && !!identity.relayNameClaim;
  const [checked, setChecked] = useState(restoredVerifiedClaim);
  const [unverified, setUnverified] = useState(false);
  const [name, setName] = useState(
    identity.displayName
      .toLowerCase()
      .replace(/[^a-z0-9_-]/g, '')
      .slice(0, 32),
  );
  const [namespace] = useState(configuredRelayNamespace);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<IdentityClaimProgress>('preparing');

  useEffect(() => {
    setChecked(false);
    setError(null);
    setUnverified(false);
    if (restoredVerifiedClaim) {
      setClaim(identity.relayNameClaim ?? null);
      publishingPolicy.setMode('verified');
      setChecked(true);
      return;
    }
    setClaim(null);
    publishingPolicy.setMode('required');
    let active = true;
    identityService
      .getIdentityEntryState()
      .then(async ({ claim: local, mode }) => {
        if (!active) return;
        const isUnverified = mode === 'unverified';
        setUnverified(isUnverified);
        publishingPolicy.setMode(mode);
        if (local?.request.peerId === identity.peerId) {
          await identityService.setPublishingMode('verified');
          if (!active) return;
          setClaim(local);
          attachVerifiedRelayName(local);
          publishingPolicy.setMode('verified');
        } else if (mode === 'verified') {
          setError(
            'Your previously verified Harbor name is not available locally. Reconnect and retry the claim so Harbor can restore it.',
          );
        }
      })
      .catch((err) => {
        if (active) setError(getErrorMessage(err));
      })
      .finally(() => {
        if (active) setChecked(true);
      });
    return () => {
      active = false;
    };
  }, [attachVerifiedRelayName, identity.peerId, identity.relayNameClaim, restoredVerifiedClaim]);

  useEffect(() => {
    const handleVerificationRequest = () => {
      if (identity.relayNameVerified) return;
      setError(null);
      setUnverified(false);
      publishingPolicy.setMode('required');
    };
    window.addEventListener(IDENTITY_VERIFICATION_REQUEST_EVENT, handleVerificationRequest);
    return () =>
      window.removeEventListener(IDENTITY_VERIFICATION_REQUEST_EVENT, handleVerificationRequest);
  }, [identity.relayNameVerified]);

  if (!checked)
    return <div className="min-h-screen grid place-items-center">Checking your Harbor name…</div>;
  if (claim || unverified) return <>{children}</>;

  async function register() {
    setBusy(true);
    setProgress('preparing');
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
        setProgress,
      );
      const next = completed.relayNameClaim;
      if (!next) throw new Error('Harbor completed registration without a verified name claim.');
      setClaim(next);
      attachVerifiedRelayName(next);
      publishingPolicy.setMode('verified');
    } catch (err) {
      setError(getNameClaimErrorMessage(err));
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
              “{identity.displayName}” is your local account label. Claim a relay-unique address so
              other people can verify which identity is yours.
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
            {error}
          </div>
        )}
        <Button className="w-full" disabled={busy || !name || !namespace} onClick={register}>
          {busy ? progressLabels[progress] : error ? 'Retry name claim' : 'Claim this name'}
        </Button>
        <button
          className="w-full text-sm underline"
          onClick={async () => {
            try {
              await identityService.setPublishingMode('unverified');
              publishingPolicy.setMode('unverified');
              setUnverified(true);
            } catch (err) {
              setError(getErrorMessage(err));
            }
          }}
        >
          Continue with an unverified identity
        </button>
        <p className="text-xs" style={{ color: 'hsl(var(--harbor-warning))' }}>
          Your posts and messages remain signed by your account keys, but your chosen label will
          always show @unverified until you claim a relay-unique name.
        </p>
      </section>
    </main>
  );
}
