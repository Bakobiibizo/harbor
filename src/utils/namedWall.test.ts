import { describe, expect, it } from 'vitest';
import { namedWallPath, normalizeQualifiedRelayName } from './namedWall';

describe('named wall routes', () => {
  it('creates a URL-safe internal route without identity keys', () => {
    expect(namedWallPath('@bugs@harbor.social')).toBe('/name/%40bugs%40harbor.social/wall');
  });

  it('rejects malformed names instead of placing them in a route', () => {
    expect(normalizeQualifiedRelayName('@bugs@harbor.social/../../settings')).toBeNull();
    expect(() => namedWallPath('peer-id')).toThrow(/Invalid relay-qualified/);
  });
});
