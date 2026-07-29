export interface ProfileToken {
  readonly profileId: string;
  readonly epoch: number;
}

export type ProfileSuspendCallback = (token: ProfileToken) => void;

let activeToken: ProfileToken | null = null;
let nextEpoch = 0;
const suspendCallbacks = new Set<ProfileSuspendCallback>();

function validateProfileId(profileId: string): string {
  if (!profileId || profileId.trim() !== profileId) {
    throw new Error('A non-empty trusted profile ID is required');
  }
  return profileId;
}

/** Activate a trusted backend profile and invalidate any previous session. */
export function activateProfile(profileId: string): ProfileToken {
  const validated = validateProfileId(profileId);
  if (activeToken?.profileId === validated) return activeToken;

  suspendProfile();
  activeToken = Object.freeze({ profileId: validated, epoch: ++nextEpoch });
  return activeToken;
}

/** Invalidate the current epoch. Repeated suspension is intentionally idempotent. */
export function suspendProfile(): void {
  const suspended = activeToken;
  if (!suspended) return;

  // Invalidate first so callbacks and delayed work already observe a closed epoch.
  activeToken = null;
  for (const callback of [...suspendCallbacks]) {
    try {
      callback(suspended);
    } catch (error) {
      console.error('[ProfileSession] Profile teardown callback failed', error);
    }
  }
}

/** Capture the immutable token for the currently active profile. */
export function captureProfile(): ProfileToken | null {
  return activeToken;
}

/** True only for the exact token issued for the active epoch. */
export function isCurrentProfile(token: ProfileToken | null | undefined): boolean {
  return token != null && token === activeToken;
}

/** Return the trusted active profile ID or fail before profile activation. */
export function requireProfileId(): string {
  if (!activeToken) throw new Error('No Harbor profile is active');
  return activeToken.profileId;
}

/** Subscribe to profile teardown. Each callback runs at most once per active epoch. */
export function onProfileSuspend(callback: ProfileSuspendCallback): () => void {
  suspendCallbacks.add(callback);
  return () => suspendCallbacks.delete(callback);
}
