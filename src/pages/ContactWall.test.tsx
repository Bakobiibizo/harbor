import { describe, expect, it } from 'vitest';
import { publicOnlyText } from './ContactWall';

describe('contact wall capability status', () => {
  it('states active private-wall access precisely', () => {
    expect(publicOnlyText(true)).toContain('Contact access is active');
  });

  it('explains expired or revoked access without claiming cached data was erased', () => {
    const text = publicOnlyText(false);
    expect(text).toContain('expired, or revoked');
    expect(text).toContain('New private posts are not served');
    expect(text).toContain('previously downloaded posts may remain');
  });

  it('uses a non-authoritative state while the current grant is still loading', () => {
    expect(publicOnlyText(null)).toContain('while Harbor verifies permission');
  });
});
