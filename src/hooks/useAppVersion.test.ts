import { renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { getVersion } from '@tauri-apps/api/app';
import { useAppVersion } from './useAppVersion';

vi.mock('@tauri-apps/api/app', () => ({ getVersion: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ isTauri: vi.fn(() => true) }));

describe('useAppVersion', () => {
  beforeEach(() => vi.mocked(getVersion).mockReset());

  it('uses the installed application version as the canonical display value', async () => {
    vi.mocked(getVersion).mockResolvedValue('9.8.7');

    const { result } = renderHook(() => useAppVersion());

    expect(result.current).toBe('Development build');
    await waitFor(() => expect(result.current).toBe('v9.8.7'));
  });
});
