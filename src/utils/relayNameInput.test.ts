import { describe, expect, it } from 'vitest';
import {
  OFFICIAL_RELAY_NAMESPACE,
  configuredRelayNamespace,
  relayAddressPreview,
  validateRelayNamespace,
} from './relayNameInput';
describe('relay namespace packaging contract', () => {
  it('uses the official fail-safe namespace', () => {
    expect(OFFICIAL_RELAY_NAMESPACE).toBe('harbor.social');
    expect(configuredRelayNamespace).toBeTruthy();
    expect(validateRelayNamespace(configuredRelayNamespace)).toBeNull();
  });
  it.each([
    '',
    'Harbor.social',
    'https://harbor.social',
    'harbor.social/path',
    'localhost',
    '-bad.example',
  ])('rejects invalid namespace %s', (value) =>
    expect(validateRelayNamespace(value)).not.toBeNull(),
  );
  it('never fabricates a fallback address when namespace is absent', () => {
    expect(relayAddressPreview('alice', '')).toBeNull();
    expect(relayAddressPreview('alice', 'harbor.social')).toBe('@alice@harbor.social');
  });
});
