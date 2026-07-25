import { requireProfileId } from './profileSession';

function validateNamespace(namespace: string): string {
  if (!/^[a-z0-9][a-z0-9._-]*$/i.test(namespace)) {
    throw new Error(
      'Profile storage namespace must contain only letters, numbers, dot, dash, or underscore',
    );
  }
  return namespace;
}

function validateVersion(version: number): number {
  if (!Number.isSafeInteger(version) || version < 1) {
    throw new Error('Profile storage version must be a positive integer');
  }
  return version;
}

/** Build a storage key only after a trusted profile has been activated. */
export function profileStorageKey(namespace: string, version: number): string {
  const profileId = requireProfileId();
  return `harbor:profile:${encodeURIComponent(profileId)}:${validateNamespace(namespace)}:v${validateVersion(version)}`;
}

/**
 * Move one legacy string value into the active profile without overwriting an
 * existing scoped value. The legacy key is removed only after the scoped value
 * exists, making the migration one-time and safe to retry after storage errors.
 */
export function migrateLegacyProfileValue(
  legacyKey: string,
  namespace: string,
  version: number,
): string | null {
  if (!legacyKey) throw new Error('Legacy storage key is required');
  const targetKey = profileStorageKey(namespace, version);
  const existing = localStorage.getItem(targetKey);
  if (existing !== null) {
    localStorage.removeItem(legacyKey);
    return existing;
  }

  const legacy = localStorage.getItem(legacyKey);
  if (legacy === null) return null;
  localStorage.setItem(targetKey, legacy);
  localStorage.removeItem(legacyKey);
  return legacy;
}
