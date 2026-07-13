export const OFFICIAL_RELAY_NAMESPACE = 'harbor.social';
export function validateRelayNamespace(value: string): string | null {
  if (
    !/^(?=.{4,253}$)(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$/.test(
      value,
    )
  )
    return 'Relay namespace must be a canonical lowercase hostname.';
  return null;
}
const rawNamespaceOverride = import.meta.env.VITE_HARBOR_RELAY_NAMESPACE;
const namespaceOverride = rawNamespaceOverride?.trim();
if (rawNamespaceOverride !== undefined && validateRelayNamespace(namespaceOverride || ''))
  throw new Error(
    `Invalid VITE_HARBOR_RELAY_NAMESPACE override: ${JSON.stringify(rawNamespaceOverride)}`,
  );
export const configuredRelayNamespace = namespaceOverride || OFFICIAL_RELAY_NAMESPACE;
if (validateRelayNamespace(configuredRelayNamespace))
  throw new Error(`Invalid VITE_HARBOR_RELAY_NAMESPACE: ${configuredRelayNamespace}`);
export function validateRelayLocalName(value: string): string | null {
  if (!/^[a-z0-9](?:[a-z0-9-]{1,30}[a-z0-9])$/.test(value) || value.includes('--'))
    return 'Use 3–32 lowercase letters or numbers, with single hyphens only.';
  return null;
}
export function relayAddressPreview(name: string, namespace: string): string | null {
  if (!namespace || validateRelayNamespace(namespace)) return null;
  return `@${name}@${namespace}`;
}
