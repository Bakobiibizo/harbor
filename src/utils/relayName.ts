import type { IdentityInfo, RelayNameClaim, RelayNamePresentation } from '../types';
import { qualifiedRelayName } from '../types';

export const UNVERIFIED_HARBOR_USER = 'Unverified Harbor user';

export function isVerifiedQualifiedName(value: string | null | undefined): value is string {
  return !!value && /^@[a-z0-9](?:[a-z0-9-]*[a-z0-9])?@[a-z0-9.-]+$/.test(value);
}

export function presentRelayName(
  claim: RelayNameClaim | null | undefined,
  _legacyName: string,
  verified: boolean,
  now = Math.floor(Date.now() / 1000),
): RelayNamePresentation {
  if (!claim) return { label: UNVERIFIED_HARBOR_USER, qualifiedName: null, trust: 'unverified' };
  const label = qualifiedRelayName(claim);
  if (claim.notAfter <= now) return { label, qualifiedName: label, trust: 'expired' };
  return { label, qualifiedName: label, trust: verified ? 'verified' : 'untrusted' };
}

export function presentIdentityName(identity: IdentityInfo): RelayNamePresentation {
  return presentRelayName(
    identity.relayNameClaim,
    identity.displayName,
    identity.relayNameVerified === true,
  );
}
export function safeIdentityLabel(identity: IdentityInfo): string {
  const presentation = presentIdentityName(identity);
  return presentation.trust === 'verified' && isVerifiedQualifiedName(presentation.qualifiedName)
    ? presentation.qualifiedName
    : UNVERIFIED_HARBOR_USER;
}
export function safePeerLabel(_peerId: string, verifiedQualifiedName?: string | null): string {
  return isVerifiedQualifiedName(verifiedQualifiedName)
    ? verifiedQualifiedName
    : UNVERIFIED_HARBOR_USER;
}
