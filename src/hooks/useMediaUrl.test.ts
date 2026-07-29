import { renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { mediaService } from '../services/media';
import { useMediaUrl } from './useMediaUrl';

vi.mock('../services/media', () => ({
  mediaService: { getMediaUrl: vi.fn() },
}));

describe('useMediaUrl', () => {
  beforeEach(() => vi.clearAllMocks());

  it('resolves packaged media and drops stale lifecycle completions', async () => {
    let resolveOld!: (url: string) => void;
    vi.mocked(mediaService.getMediaUrl)
      .mockReturnValueOnce(
        new Promise((resolve) => {
          resolveOld = resolve;
        }),
      )
      .mockResolvedValueOnce('asset:new');
    const { result, rerender } = renderHook(({ hash }) => useMediaUrl(hash), {
      initialProps: { hash: 'a'.repeat(64) },
    });
    rerender({ hash: 'b'.repeat(64) });
    await waitFor(() => expect(result.current).toBe('asset:new'));
    resolveOld('asset:stale');
    await Promise.resolve();
    expect(result.current).toBe('asset:new');
  });

  it('fails closed when content is unavailable', async () => {
    vi.mocked(mediaService.getMediaUrl).mockRejectedValue(new Error('missing'));
    const { result } = renderHook(() => useMediaUrl('c'.repeat(64)));
    await waitFor(() => expect(mediaService.getMediaUrl).toHaveBeenCalled());
    expect(result.current).toBeNull();
  });
});
