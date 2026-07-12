import type { IdentityInfo, RelayNameClaim, RelayNamePresentation } from '../types';
import { qualifiedRelayName } from '../types';

export function presentRelayName(
  claim: RelayNameClaim | null | undefined,
  legacyName: string,
  verified: boolean,
  now = Date.now(),
): RelayNamePresentation {
  if (!claim) return { label: legacyName, qualifiedName: null, trust: 'unverified' };
  const label = qualifiedRelayName(claim);
  if (claim.expiresAt <= now) return { label, qualifiedName: label, trust: 'expired' };
  return { label, qualifiedName: label, trust: verified ? 'verified' : 'untrusted' };
}

export function presentIdentityName(identity: IdentityInfo): RelayNamePresentation {
  return presentRelayName(
    identity.relayNameClaim,
    identity.displayName,
    identity.relayNameVerified === true,
  );
}
