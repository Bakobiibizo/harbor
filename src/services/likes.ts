import { invoke } from '@tauri-apps/api/core';

/** Summary of like/reaction state for a post. */
export interface LikeSummary {
  postId: string;
  totalLikes: number;
  userHasLiked: boolean;
}

interface RawLikeSummary {
  postId?: string;
  post_id?: string;
  totalLikes?: number;
  total_likes?: number;
  userHasLiked?: boolean;
  user_has_liked?: boolean;
}

function normalizeLikeSummary(summary: RawLikeSummary): LikeSummary {
  return {
    postId: summary.postId ?? summary.post_id ?? '',
    totalLikes: summary.totalLikes ?? summary.total_likes ?? 0,
    userHasLiked: summary.userHasLiked ?? summary.user_has_liked ?? false,
  };
}

/** Likes service - wraps Tauri commands for signed post reactions. */
export const likesService = {
  async likePost(postId: string): Promise<LikeSummary> {
    return normalizeLikeSummary(await invoke<RawLikeSummary>('like_post', { postId }));
  },

  async unlikePost(postId: string): Promise<LikeSummary> {
    return normalizeLikeSummary(await invoke<RawLikeSummary>('unlike_post', { postId }));
  },

  async getPostLikes(postId: string): Promise<LikeSummary> {
    return normalizeLikeSummary(await invoke<RawLikeSummary>('get_post_likes', { postId }));
  },

  async getPostsLikesBatch(postIds: string[]): Promise<LikeSummary[]> {
    const summaries = await invoke<RawLikeSummary[]>('get_posts_likes_batch', { postIds });
    return summaries.map(normalizeLikeSummary);
  },

  async getMyLikedPosts(): Promise<string[]> {
    return invoke<string[]>('get_my_liked_posts');
  },
};
