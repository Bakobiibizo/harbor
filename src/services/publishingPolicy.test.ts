import { describe, expect, it } from 'vitest';
import { publishingPolicy } from './publishingPolicy';

describe('publishingPolicy', () => {
  it('tracks explicit verified and compatibility modes', () => {
    publishingPolicy.setMode('verified');
    expect(publishingPolicy.getMode()).toBe('verified');
    publishingPolicy.setMode('compatibility');
    expect(publishingPolicy.getMode()).toBe('compatibility');
  });
  it('retains required mode as a distinct release gate', () => {
    publishingPolicy.setMode('required');
    expect(publishingPolicy.getMode()).toBe('required');
  });
});
