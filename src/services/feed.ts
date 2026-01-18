import { invoke } from '@tauri-apps/api/core';
import type { FeedItem } from '../types';

/** Feed service - wraps Tauri commands for feed functionality */
export const feedService = {
  /** Get the user's feed (posts from contacts) */
  async getFeed(limit?: number, beforeTimestamp?: number): Promise<FeedItem[]> {
    return invoke<FeedItem[]>('get_feed', { limit, beforeTimestamp });
  },

  /** Get a specific user's wall */
  async getWall(
    authorPeerId: string,
    limit?: number,
    beforeTimestamp?: number,
  ): Promise<FeedItem[]> {
    return invoke<FeedItem[]>('get_wall', {
      authorPeerId,
      limit,
      beforeTimestamp,
    });
  },
};
