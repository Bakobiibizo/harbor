import { describe, expect, it } from 'vitest';
import { publishingPolicy } from './publishingPolicy';

describe('publishingPolicy', () => {
  it('tracks explicit verified and unverified modes', () => {
    publishingPolicy.setMode('verified');
    expect(publishingPolicy.getMode()).toBe('verified');
    publishingPolicy.setMode('unverified');
    expect(publishingPolicy.getMode()).toBe('unverified');
  });
  it('retains required mode as a distinct release gate', () => {
    publishingPolicy.setMode('required');
    expect(publishingPolicy.getMode()).toBe('required');
  });
});
