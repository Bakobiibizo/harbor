import { create } from "zustand";
import { feedService } from "../services/feed";
import type { FeedItem } from "../types";

interface FeedState {
  // State
  feedItems: FeedItem[];
  isLoading: boolean;
  error: string | null;
  hasMore: boolean;

  // Actions
  loadFeed: (limit?: number) => Promise<void>;
  loadMore: (limit?: number) => Promise<void>;
  refreshFeed: () => Promise<void>;
}

export const useFeedStore = create<FeedState>((set, get) => ({
  // Initial state
  feedItems: [],
  isLoading: false,
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
      console.error("Failed to load feed:", error);
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
      console.error("Failed to load more feed items:", error);
      set({ error: String(error), isLoading: false });
    }
  },

  // Refresh feed (reload from beginning)
  refreshFeed: async () => {
    set({ isLoading: true, error: null });
    try {
      const feedItems = await feedService.getFeed(50);
      set({
        feedItems,
        isLoading: false,
        hasMore: feedItems.length === 50,
      });
    } catch (error) {
      console.error("Failed to refresh feed:", error);
      set({ error: String(error), isLoading: false });
    }
  },
}));
