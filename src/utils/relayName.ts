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
  return identity.relayNameVerified && identity.relayNameClaim
    ? qualifiedRelayName(identity.relayNameClaim)
    : `Peer ${identity.peerId.slice(0, 8)}… (unverified)`;
}
export function safePeerLabel(peerId: string): string {
  return `Peer ${peerId.slice(0, 8)}… (unverified)`;
}
