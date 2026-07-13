import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { mentionsService } from './mentions';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

describe('mentionsService', () => {
  beforeEach(() => vi.clearAllMocks());
  it('keeps signed mention creation behind one typed request', async () => {
    vi.mocked(invoke).mockResolvedValue({ postId: 'p', createdAt: 1 });
    const request = {
      contentType: 'text',
      contentText: 'hi',
      visibility: 'contacts' as const,
      mentions: [
        {
          qualifiedName: '@a@relay',
          intent: 'notify' as const,
          authorizedPeerId: 'peer',
          claimDigest: 'digest',
        },
      ],
    };
    await mentionsService.publish(request);
    expect(invoke).toHaveBeenCalledWith('create_post_with_mentions', { request });
  });
  it('sends review consent separately from contact authorization', async () => {
    await mentionsService.review('m1', 'accept-repost');
    expect(invoke).toHaveBeenCalledWith('review_private_mention', {
      mentionId: 'm1',
      decision: 'accept-repost',
    });
  });
});
