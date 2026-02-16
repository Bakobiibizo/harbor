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

  /** Sync all contact walls from the relay server into the local feed */
  async syncFromRelay(): Promise<void> {
    return invoke<void>('sync_feed_from_relay');
  },

  /** Push the local wall to the relay server */
  async syncWallToRelay(): Promise<void> {
    return invoke<void>('sync_wall_to_relay');
  },

  /** Fetch a specific contact's wall from the relay server */
  async fetchContactWall(authorPeerId: string): Promise<void> {
    return invoke<void>('fetch_contact_wall_from_relay', { authorPeerId });
  },
};
