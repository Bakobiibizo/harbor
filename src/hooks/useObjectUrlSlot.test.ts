import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useObjectUrlSlot } from './useObjectUrlSlot';

describe('useObjectUrlSlot', () => {
  const createObjectURL = vi.fn();
  const revokeObjectURL = vi.fn();

  beforeEach(() => {
    createObjectURL
      .mockReset()
      .mockReturnValueOnce('blob:attachment-1')
      .mockReturnValueOnce('blob:attachment-2')
      .mockReturnValueOnce('blob:attachment-3');
    revokeObjectURL.mockReset();
    vi.stubGlobal('URL', { ...URL, createObjectURL, revokeObjectURL });
  });

  afterEach(() => vi.unstubAllGlobals());

  it('revokes every replaced, cleared, and unmounted attachment URL exactly once', () => {
    const { result, unmount } = renderHook(() => useObjectUrlSlot());

    act(() => {
      expect(result.current.replace(new Blob(['one']))).toBe('blob:attachment-1');
      expect(result.current.replace(new Blob(['two']))).toBe('blob:attachment-2');
    });
    expect(revokeObjectURL.mock.calls).toEqual([['blob:attachment-1']]);

    act(() => {
      result.current.clear();
      result.current.clear();
      expect(result.current.replace(new Blob(['three']))).toBe('blob:attachment-3');
    });
    expect(revokeObjectURL.mock.calls).toEqual([['blob:attachment-1'], ['blob:attachment-2']]);

    unmount();
    expect(revokeObjectURL.mock.calls).toEqual([
      ['blob:attachment-1'],
      ['blob:attachment-2'],
      ['blob:attachment-3'],
    ]);
  });
});
