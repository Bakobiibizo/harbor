import { invoke } from '@tauri-apps/api/core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { getMessagingPrivacyPolicy, setReadReceiptsEnabled } from './messagingPrivacy';

describe('messaging privacy service', () => {
  beforeEach(() => vi.clearAllMocks());

  it('reads the authoritative profile policy', async () => {
    vi.mocked(invoke).mockResolvedValue({ readReceiptsEnabled: false });

    await expect(getMessagingPrivacyPolicy()).resolves.toEqual({ readReceiptsEnabled: false });
    expect(invoke).toHaveBeenCalledWith('get_messaging_privacy_policy');
  });

  it('persists the requested read receipt policy', async () => {
    vi.mocked(invoke).mockResolvedValue({ readReceiptsEnabled: true });

    await expect(setReadReceiptsEnabled(true)).resolves.toEqual({ readReceiptsEnabled: true });
    expect(invoke).toHaveBeenCalledWith('set_read_receipts_enabled', { enabled: true });
  });
});
