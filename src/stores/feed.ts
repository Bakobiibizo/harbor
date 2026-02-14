import { create } from 'zustand';
import { feedService } from '../services/feed';
import * as networkService from '../services/network';
import { createLogger } from '../utils/logger';
import type { FeedItem } from '../types';

const log = createLogger('FeedStore');

interface FeedState {
  // State
  feedItems: FeedItem[];
  isLoading: boolean;
  isSyncingRelay: boolean;
  error: string | null;
  hasMore: boolean;

  // Actions
  loadFeed: (limit?: number) => Promise<void>;
  loadMore: (limit?: number) => Promise<void>;
  refreshFeed: () => Promise<void>;
  syncFromRelay: () => Promise<void>;
}

export const useFeedStore = create<FeedState>((set, get) => ({
  // Initial state
  feedItems: [],
  isLoading: false,
  isSyncingRelay: false,
  error: null,
  hasMore: true,

  // Load initial feed
  loadFeed: async (limit: number = 50) => {
    set({ isLoading: true, error: null });
    try {
      const feedItems = await feedService.getFeed(limit);
      set({
        feedItems,
        isLoading: false,
        hasMore: feedItems.length === limit,
      });
    } catch (error) {
      log.error('Failed to load feed', error);
      set({ error: String(error), isLoading: false });
    }
  },

  // Load more items (pagination)
  loadMore: async (limit: number = 50) => {
    const { feedItems, isLoading, hasMore } = get();
    if (isLoading || !hasMore) return;

    set({ isLoading: true });
    try {
      const lastItem = feedItems[feedItems.length - 1];
      const beforeTimestamp = lastItem?.createdAt;
      const newItems = await feedService.getFeed(limit, beforeTimestamp);

      set({
        feedItems: [...feedItems, ...newItems],
        isLoading: false,
        hasMore: newItems.length === limit,
      });
    } catch (error) {
      log.error('Failed to load more feed items', error);
      set({ error: String(error), isLoading: false });
    }
  },

  // Refresh feed (reload from beginning)
  refreshFeed: async () => {
    set({ isLoading: true, error: null });
    try {
      await networkService.syncFeed(50);
      const feedItems = await feedService.getFeed(50);
      set({
        feedItems,
        isLoading: false,
        hasMore: feedItems.length === 50,
      });
    } catch (error) {
      log.error('Failed to refresh feed', error);
      set({ error: String(error), isLoading: false });
    }
  },

  // Sync feed from relay server (fetches contact walls via relay)
  syncFromRelay: async () => {
    const { isSyncingRelay } = get();
    if (isSyncingRelay) return; // Avoid concurrent syncs

    set({ isSyncingRelay: true });
    try {
      await feedService.syncFromRelay();
      // Reload local feed to pick up any new posts from the relay
      const feedItems = await feedService.getFeed(50);
      set({
        feedItems,
        isSyncingRelay: false,
        hasMore: feedItems.length === 50,
      });
    } catch (error) {
      log.warn('Failed to sync feed from relay', error);
      set({ isSyncingRelay: false });
      // Don't set error state — relay sync is best-effort
    }
  },
}));
