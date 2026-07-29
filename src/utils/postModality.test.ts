import { describe, expect, it } from 'vitest';
import { contentTypeForPost, derivePostModality, matchesModalityFilter } from './postModality';

describe('post modality', () => {
  it('derives text and explicit media modalities', () => {
    expect(derivePostModality('text')).toBe('text');
    expect(derivePostModality('thought')).toBe('text');
    expect(derivePostModality('image')).toBe('image');
  });

  it('uses the first attachment as the canonical modality', () => {
    const media = [{ type: 'video' as const }, { type: 'image' as const }];
    expect(derivePostModality('post', media)).toBe('video');
    expect(contentTypeForPost('post', media)).toBe('video');
    expect(matchesModalityFilter('videos', 'post', media)).toBe(true);
    expect(matchesModalityFilter('images', 'post', media)).toBe(false);
  });

  it('always includes posts in All and filters each media modality strictly', () => {
    expect(matchesModalityFilter('all', 'text')).toBe(true);
    expect(matchesModalityFilter('audio', 'audio')).toBe(true);
    expect(matchesModalityFilter('videos', 'audio')).toBe(false);
  });
});
