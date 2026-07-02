import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { likesService } from './likes';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

describe('likesService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should like a post', async () => {
    vi.mocked(invoke).mockResolvedValue({
      post_id: 'post-1',
      total_likes: 1,
      user_has_liked: true,
    });

    const result = await likesService.likePost('post-1');

    expect(invoke).toHaveBeenCalledWith('like_post', { postId: 'post-1' });
    expect(result).toEqual({ postId: 'post-1', totalLikes: 1, userHasLiked: true });
  });

  it('should unlike a post', async () => {
    vi.mocked(invoke).mockResolvedValue({
      postId: 'post-1',
      totalLikes: 0,
      userHasLiked: false,
    });

    const result = await likesService.unlikePost('post-1');

    expect(invoke).toHaveBeenCalledWith('unlike_post', { postId: 'post-1' });
    expect(result).toEqual({ postId: 'post-1', totalLikes: 0, userHasLiked: false });
  });

  it('should fetch batch like summaries', async () => {
    vi.mocked(invoke).mockResolvedValue([
      { post_id: 'post-1', total_likes: 2, user_has_liked: false },
      { post_id: 'post-2', total_likes: 5, user_has_liked: true },
    ]);

    const result = await likesService.getPostsLikesBatch(['post-1', 'post-2']);

    expect(invoke).toHaveBeenCalledWith('get_posts_likes_batch', {
      postIds: ['post-1', 'post-2'],
    });
    expect(result).toEqual([
      { postId: 'post-1', totalLikes: 2, userHasLiked: false },
      { postId: 'post-2', totalLikes: 5, userHasLiked: true },
    ]);
  });
});
