export const configuredRelayNamespace = import.meta.env.VITE_HARBOR_RELAY_NAMESPACE || '';
export function validateRelayLocalName(value: string): string | null {
  if (!/^[a-z0-9](?:[a-z0-9-]{1,30}[a-z0-9])$/.test(value) || value.includes('--'))
    return 'Use 3–32 lowercase letters or numbers, with single hyphens only.';
  return null;
}
