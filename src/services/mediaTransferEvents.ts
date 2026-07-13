import type { IdentityState } from '../types';

/** Reject delayed lifecycle events emitted by a previously active profile. */
export function isMediaTransferEventForIdentity(
  eventProfileId: string,
  identity: IdentityState,
): boolean {
  return identity.status === 'unlocked' && identity.identity.peerId === eventProfileId;
}
