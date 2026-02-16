import { create } from 'zustand';
import { feedService } from '../services/feed';
import { commentsService } from '../services/comments';
import type { Comment } from '../services/comments';
import * as networkService from '../services/network';
import { createLogger } from '../utils/logger';
import type { FeedItem } from '../types';

const log = createLogger('FeedStore');

export type { Comment } from '../services/comments';

interface FeedState {
  // State
  feedItems: FeedItem[];
  isLoading: boolean;
  isSyncingRelay: boolean;
  error: string | null;
  hasMore: boolean;

  // Comments state
  comments: Record<string, Comment[]>; // keyed by post ID
  commentCounts: Record<string, number>; // keyed by post ID
  expandedComments: Set<string>; // post IDs with expanded comments
  loadingComments: Set<string>; // post IDs currently loading comments

  // Actions
  loadFeed: (limit?: number) => Promise<void>;
  loadMore: (limit?: number) => Promise<void>;
  refreshFeed: () => Promise<void>;
  syncFromRelay: () => Promise<void>;

  // Comment actions
  loadComments: (postId: string) => Promise<void>;
  addComment: (postId: string, content: string) => Promise<void>;
  deleteComment: (postId: string, commentId: string) => Promise<void>;
  toggleComments: (postId: string) => void;
  loadCommentCounts: (postIds: string[]) => Promise<void>;
}

export const useFeedStore = create<FeedState>((set, get) => ({
  // Initial state
  feedItems: [],
  isLoading: false,
  isSyncingRelay: false,
  error: null,
  hasMore: true,

  // Comments initial state
  comments: {},
  commentCounts: {},
  expandedComments: new Set<string>(),
  loadingComments: new Set<string>(),

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

      // Load comment counts for all feed items
      if (feedItems.length > 0) {
        const postIds = feedItems.map((item) => item.postId);
        get().loadCommentCounts(postIds);
      }
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

      // Load comment counts for new items
      if (newItems.length > 0) {
        const postIds = newItems.map((item) => item.postId);
        get().loadCommentCounts(postIds);
      }
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

      // Load comment counts for refreshed feed
      if (feedItems.length > 0) {
        const postIds = feedItems.map((item) => item.postId);
        get().loadCommentCounts(postIds);
      }
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

  // Load comments for a specific post
  loadComments: async (postId: string) => {
    const { loadingComments } = get();
    if (loadingComments.has(postId)) return;

    const newLoading = new Set(loadingComments);
    newLoading.add(postId);
    set({ loadingComments: newLoading });

    try {
      const comments = await commentsService.getComments(postId);
      set((state) => {
        const newLoading = new Set(state.loadingComments);
        newLoading.delete(postId);
        return {
          comments: { ...state.comments, [postId]: comments },
          commentCounts: { ...state.commentCounts, [postId]: comments.length },
          loadingComments: newLoading,
        };
      });
    } catch (error) {
      log.error('Failed to load comments', error);
      set((state) => {
        const newLoading = new Set(state.loadingComments);
        newLoading.delete(postId);
        return { loadingComments: newLoading };
      });
    }
  },

  // Add a comment to a post
  addComment: async (postId: string, content: string) => {
    try {
      const comment = await commentsService.addComment(postId, content);

      set((state) => {
        const existingComments = state.comments[postId] || [];
        const currentCount = state.commentCounts[postId] || 0;
        return {
          comments: { ...state.comments, [postId]: [...existingComments, comment] },
          commentCounts: { ...state.commentCounts, [postId]: currentCount + 1 },
        };
      });
    } catch (error) {
      log.error('Failed to add comment', error);
      throw error;
    }
  },

  // Delete a comment
  deleteComment: async (postId: string, commentId: string) => {
    try {
      await commentsService.deleteComment(commentId);

      set((state) => {
        const existingComments = state.comments[postId] || [];
        const currentCount = state.commentCounts[postId] || 0;
        return {
          comments: {
            ...state.comments,
            [postId]: existingComments.filter((c) => c.commentId !== commentId),
          },
          commentCounts: { ...state.commentCounts, [postId]: Math.max(0, currentCount - 1) },
        };
      });
    } catch (error) {
      log.error('Failed to delete comment', error);
      throw error;
    }
  },

  // Toggle comments section visibility for a post
  toggleComments: (postId: string) => {
    set((state) => {
      const newExpanded = new Set(state.expandedComments);
      if (newExpanded.has(postId)) {
        newExpanded.delete(postId);
      } else {
        newExpanded.add(postId);
        // Load comments if not already loaded
        if (!state.comments[postId]) {
          get().loadComments(postId);
        }
      }
      return { expandedComments: newExpanded };
    });
  },

  // Load comment counts for multiple posts
  loadCommentCounts: async (postIds: string[]) => {
    try {
      const counts = await commentsService.getCommentCounts(postIds);
      set((state) => {
        const newCounts = { ...state.commentCounts };
        for (const c of counts) {
          newCounts[c.postId] = c.count;
        }
        return { commentCounts: newCounts };
      });
    } catch (error) {
      log.error('Failed to load comment counts', error);
    }
  },
}));
