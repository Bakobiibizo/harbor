import { isCurrentProfile, type ProfileToken } from './profileSession';

export interface ProfileEventReadinessLease {
  readonly token: ProfileToken;
  readonly id: symbol;
}

let activeLease: ProfileEventReadinessLease | null = null;
let activeLeaseReady = false;

/** Begin listener registration for one exact profile epoch. */
export function beginProfileEventRegistration(token: ProfileToken): ProfileEventReadinessLease {
  const lease = Object.freeze({ token, id: Symbol('profile-event-readiness') });
  activeLease = lease;
  activeLeaseReady = false;
  return lease;
}

/** Publish readiness only when every listener committed for the active epoch. */
export function markProfileEventsReady(lease: ProfileEventReadinessLease): boolean {
  if (activeLease !== lease || !isCurrentProfile(lease.token)) return false;
  activeLeaseReady = true;
  return true;
}

/** Clear readiness without allowing delayed cleanup to affect a newer epoch. */
export function clearProfileEventRegistration(lease: ProfileEventReadinessLease): void {
  if (activeLease !== lease) return;
  activeLease = null;
  activeLeaseReady = false;
}

export function getProfileEventsReady(): boolean {
  return Boolean(activeLease && activeLeaseReady && isCurrentProfile(activeLease.token));
}
