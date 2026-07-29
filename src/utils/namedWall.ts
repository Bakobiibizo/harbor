const QUALIFIED_RELAY_NAME = /^@[a-z0-9](?:[a-z0-9-]{1,30}[a-z0-9])?@[a-z0-9.-]{4,253}$/;

export function normalizeQualifiedRelayName(value: string): string | null {
  const normalized = value.trim().toLowerCase();
  return QUALIFIED_RELAY_NAME.test(normalized) ? normalized : null;
}

export function namedWallPath(qualifiedName: string): string {
  const normalized = normalizeQualifiedRelayName(qualifiedName);
  if (!normalized) throw new Error('Invalid relay-qualified account name.');
  return `/name/${encodeURIComponent(normalized)}/wall`;
}
