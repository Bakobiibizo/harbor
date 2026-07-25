import { create } from 'zustand';
import { feedService } from '../services/feed';
import { commentsService } from '../services/comments';
import { likesService } from '../services/likes';
import type { Comment } from '../services/comments';
import type { LikeSummary } from '../services/likes';
import * as networkService from '../services/network';
import { mediaService } from '../services/media';
import { createLogger } from '../utils/logger';
import { getErrorMessage } from '../utils/errors';
import type { FeedItem } from '../types';
import { migrateLegacyProfileValue, profileStorageKey } from '../services/profileStorage';

const log = createLogger('FeedStore');

export type { Comment } from '../services/comments';

const FEED_PREFS_STORAGE_KEY = 'harbor-feed-local-preferences-v1';
const FEED_PROFILE_NAMESPACE = 'feed-preferences';
const FEED_PROFILE_VERSION = 1;
let lifecycleGeneration = 0;

export interface SnoozedAuthor {
  peerId: string;
  snoozedUntil: number;
}

interface PersistedFeedPreferences {
  savedPostIds?: string[];
  hiddenPostIds?: string[];
  snoozedAuthors?: SnoozedAuthor[];
}

function nowSeconds(): number {
  return Math.floor(Date.now() / 1000);
}

function unique(values: string[]): string[] {
  return [...new Set(values)];
}

function sanitizePreferences(prefs: PersistedFeedPreferences): Required<PersistedFeedPreferences> {
  const now = nowSeconds();
  return {
    savedPostIds: unique(prefs.savedPostIds ?? []),
    hiddenPostIds: unique(prefs.hiddenPostIds ?? []),
    snoozedAuthors: (prefs.snoozedAuthors ?? [])
      .filter((s) => s.peerId && s.snoozedUntil > now)
      .reduce<SnoozedAuthor[]>((acc, snooze) => {
        const existing = acc.findIndex((s) => s.peerId === snooze.peerId);
        if (existing >= 0) {
          acc[existing] = snooze.snoozedUntil > acc[existing].snoozedUntil ? snooze : acc[existing];
        } else {
          acc.push(snooze);
        }
        return acc;
      }, []),
  };
}

function readPreferences(): Required<PersistedFeedPreferences> {
  if (typeof localStorage === 'undefined') {
    return { savedPostIds: [], hiddenPostIds: [], snoozedAuthors: [] };
  }

  const raw = migrateLegacyProfileValue(
    FEED_PREFS_STORAGE_KEY,
    FEED_PROFILE_NAMESPACE,
    FEED_PROFILE_VERSION,
  );
  try {
    if (!raw) return { savedPostIds: [], hiddenPostIds: [], snoozedAuthors: [] };
    return sanitizePreferences(JSON.parse(raw) as PersistedFeedPreferences);
  } catch (error) {
    log.warn('Failed to read feed preferences', error);
    return { savedPostIds: [], hiddenPostIds: [], snoozedAuthors: [] };
  }
}

function writePreferences(prefs: PersistedFeedPreferences): Required<PersistedFeedPreferences> {
  const sanitized = sanitizePreferences(prefs);
  if (typeof localStorage !== 'undefined') {
    localStorage.setItem(
      profileStorageKey(FEED_PROFILE_NAMESPACE, FEED_PROFILE_VERSION),
      JSON.stringify(sanitized),
    );
  }
  return sanitized;
}

function filterVisibleFeedItems(
  items: FeedItem[],
  hiddenPostIds: string[],
  snoozedAuthors: SnoozedAuthor[],
): FeedItem[] {
  const hidden = new Set(hiddenPostIds);
  const now = nowSeconds();
  const snoozed = new Set(snoozedAuthors.filter((s) => s.snoozedUntil > now).map((s) => s.peerId));
  return items.filter((item) => !hidden.has(item.postId) && !snoozed.has(item.authorPeerId));
}

function mergeLikeSummaries(items: FeedItem[], summaries: LikeSummary[]): FeedItem[] {
  const byPost = new Map(summaries.map((summary) => [summary.postId, summary]));
  return items.map((item) => {
    const summary = byPost.get(item.postId);
    return summary
      ? { ...item, likes: summary.totalLikes, likedByUser: summary.userHasLiked }
      : { ...item, likes: 0, likedByUser: false };
  });
}

interface FeedState {
  // State
  rawFeedItems: FeedItem[];
  feedItems: FeedItem[];
  savedPostIds: string[];
  hiddenPostIds: string[];
  snoozedAuthors: SnoozedAuthor[];
  isLoading: boolean;
  isSyncingRelay: boolean;
  lastSyncAt: number | null;
  syncError: string | null;
  syncStatus: 'idle' | 'in_progress' | 'success' | 'partial_failure';
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
  hydratePreferences: () => void;
  getSavedFeedItems: () => FeedItem[];
  toggleLike: (postId: string) => Promise<void>;
  toggleSave: (postId: string) => void;
  isPostSaved: (postId: string) => boolean;
  hidePost: (postId: string) => void;
  unhidePost: (postId: string) => void;
  snoozeAuthor: (peerId: string, hours: number) => void;
  unsnoozeAuthor: (peerId: string) => void;
  clearHiddenPosts: () => void;
  clearSnoozedAuthors: () => void;

  // Comment actions
  loadComments: (postId: string) => Promise<void>;
  addComment: (postId: string, content: string) => Promise<void>;
  deleteComment: (postId: string, commentId: string) => Promise<void>;
  toggleComments: (postId: string) => void;
  loadCommentCounts: (postIds: string[]) => Promise<void>;
}

function applyFeedState(
  rawFeedItems: FeedItem[],
  hiddenPostIds: string[],
  snoozedAuthors: SnoozedAuthor[],
) {
  return {
    rawFeedItems,
    feedItems: filterVisibleFeedItems(rawFeedItems, hiddenPostIds, snoozedAuthors),
  };
}

async function decorateFeedItems(feedItems: FeedItem[]): Promise<FeedItem[]> {
  if (feedItems.length === 0) return feedItems;
  const postIds = feedItems.map((item) => item.postId);
  try {
    const likeSummaries = await likesService.getPostsLikesBatch(postIds);
    return mergeLikeSummaries(feedItems, likeSummaries);
  } catch (error) {
    log.warn('Failed to load feed reaction state', error);
    return mergeLikeSummaries(feedItems, []);
  }
}

export const useFeedStore = create<FeedState>((set, get) => ({
  // Initial state
  rawFeedItems: [],
  feedItems: [],
  savedPostIds: [],
  hiddenPostIds: [],
  snoozedAuthors: [],
  isLoading: false,
  isSyncingRelay: false,
  lastSyncAt: null,
  syncError: null,
  syncStatus: 'idle',
  error: null,
  hasMore: true,

  // Comments initial state
  comments: {},
  commentCounts: {},
  expandedComments: new Set<string>(),
  loadingComments: new Set<string>(),

  hydratePreferences: () => {
    const prefs = readPreferences();
    set((state) => ({
      ...prefs,
      ...applyFeedState(state.rawFeedItems, prefs.hiddenPostIds, prefs.snoozedAuthors),
    }));
  },

  getSavedFeedItems: () => {
    const { rawFeedItems, savedPostIds } = get();
    const saved = new Set(savedPostIds);
    return rawFeedItems.filter((item) => saved.has(item.postId));
  },

  toggleLike: async (postId: string) => {
    const generation = lifecycleGeneration;
    const existing = [...get().rawFeedItems, ...get().feedItems].find(
      (item) => item.postId === postId,
    );
    const summary = existing?.likedByUser
      ? await likesService.unlikePost(postId)
      : await likesService.likePost(postId);
    if (generation !== lifecycleGeneration) return;

    set((state) => {
      const update = (item: FeedItem) =>
        item.postId === postId
          ? { ...item, likes: summary.totalLikes, likedByUser: summary.userHasLiked }
          : item;
      return {
        rawFeedItems: state.rawFeedItems.map(update),
        feedItems: state.feedItems.map(update),
      };
    });
  },

  toggleSave: (postId: string) => {
    set((state) => {
      const savedPostIds = state.savedPostIds.includes(postId)
        ? state.savedPostIds.filter((id) => id !== postId)
        : [...state.savedPostIds, postId];
      const prefs = writePreferences({
        savedPostIds,
        hiddenPostIds: state.hiddenPostIds,
        snoozedAuthors: state.snoozedAuthors,
      });
      return { savedPostIds: prefs.savedPostIds };
    });
  },

  isPostSaved: (postId: string) => get().savedPostIds.includes(postId),

  hidePost: (postId: string) => {
    set((state) => {
      const prefs = writePreferences({
        savedPostIds: state.savedPostIds,
        hiddenPostIds: unique([...state.hiddenPostIds, postId]),
        snoozedAuthors: state.snoozedAuthors,
      });
      return {
        ...prefs,
        ...applyFeedState(state.rawFeedItems, prefs.hiddenPostIds, prefs.snoozedAuthors),
      };
    });
  },

  unhidePost: (postId: string) => {
    set((state) => {
      const prefs = writePreferences({
        savedPostIds: state.savedPostIds,
        hiddenPostIds: state.hiddenPostIds.filter((id) => id !== postId),
        snoozedAuthors: state.snoozedAuthors,
      });
      return {
        ...prefs,
        ...applyFeedState(state.rawFeedItems, prefs.hiddenPostIds, prefs.snoozedAuthors),
      };
    });
  },

  snoozeAuthor: (peerId: string, hours: number) => {
    const snoozedUntil = nowSeconds() + Math.max(1, hours) * 60 * 60;
    set((state) => {
      const prefs = writePreferences({
        savedPostIds: state.savedPostIds,
        hiddenPostIds: state.hiddenPostIds,
        snoozedAuthors: [
          ...state.snoozedAuthors.filter((s) => s.peerId !== peerId),
          { peerId, snoozedUntil },
        ],
      });
      return {
        ...prefs,
        ...applyFeedState(state.rawFeedItems, prefs.hiddenPostIds, prefs.snoozedAuthors),
      };
    });
  },

  unsnoozeAuthor: (peerId: string) => {
    set((state) => {
      const prefs = writePreferences({
        savedPostIds: state.savedPostIds,
        hiddenPostIds: state.hiddenPostIds,
        snoozedAuthors: state.snoozedAuthors.filter((s) => s.peerId !== peerId),
      });
      return {
        ...prefs,
        ...applyFeedState(state.rawFeedItems, prefs.hiddenPostIds, prefs.snoozedAuthors),
      };
    });
  },

  clearHiddenPosts: () => {
    set((state) => {
      const prefs = writePreferences({
        savedPostIds: state.savedPostIds,
        hiddenPostIds: [],
        snoozedAuthors: state.snoozedAuthors,
      });
      return {
        ...prefs,
        ...applyFeedState(state.rawFeedItems, prefs.hiddenPostIds, prefs.snoozedAuthors),
      };
    });
  },

  clearSnoozedAuthors: () => {
    set((state) => {
      const prefs = writePreferences({
        savedPostIds: state.savedPostIds,
        hiddenPostIds: state.hiddenPostIds,
        snoozedAuthors: [],
      });
      return {
        ...prefs,
        ...applyFeedState(state.rawFeedItems, prefs.hiddenPostIds, prefs.snoozedAuthors),
      };
    });
  },

  // Load initial feed
  loadFeed: async (limit: number = 50) => {
    const generation = lifecycleGeneration;
    set({ isLoading: true, error: null });
    try {
      const prefs = readPreferences();
      const rawFeedItems = await decorateFeedItems(await feedService.getFeed(limit));
      if (generation !== lifecycleGeneration) return;
      set({
        ...prefs,
        ...applyFeedState(rawFeedItems, prefs.hiddenPostIds, prefs.snoozedAuthors),
        isLoading: false,
        hasMore: rawFeedItems.length === limit,
      });

      // Load comment counts for all feed items
      if (rawFeedItems.length > 0) {
        const postIds = rawFeedItems.map((item) => item.postId);
        get().loadCommentCounts(postIds);
      }

      // Trigger background media preloader for any missing media
      mediaService
        .preloadMissingMedia()
        .catch((error) => log.warn('Background feed media preload is degraded', error));
    } catch (error) {
      if (generation !== lifecycleGeneration) return;
      log.error('Failed to load feed', error);
      set({ error: getErrorMessage(error), isLoading: false });
    }
  },

  // Load more items (pagination)
  loadMore: async (limit: number = 50) => {
    const generation = lifecycleGeneration;
    const { rawFeedItems, feedItems, isLoading, hasMore } = get();
    if (isLoading || !hasMore) return;

    set({ isLoading: true });
    try {
      const existingRawFeedItems = rawFeedItems.length > 0 ? rawFeedItems : feedItems;
      const lastItem = existingRawFeedItems[existingRawFeedItems.length - 1];
      const beforeTimestamp = lastItem?.createdAt;
      const newItems = await decorateFeedItems(await feedService.getFeed(limit, beforeTimestamp));
      if (generation !== lifecycleGeneration) return;
      const mergedRawFeedItems = [...existingRawFeedItems, ...newItems];
      const prefs = readPreferences();

      set({
        ...prefs,
        ...applyFeedState(mergedRawFeedItems, prefs.hiddenPostIds, prefs.snoozedAuthors),
        isLoading: false,
        hasMore: newItems.length === limit,
      });

      // Load comment counts for new items
      if (newItems.length > 0) {
        const postIds = newItems.map((item) => item.postId);
        get().loadCommentCounts(postIds);
      }
    } catch (error) {
      if (generation !== lifecycleGeneration) return;
      log.error('Failed to load more feed items', error);
      set({ error: getErrorMessage(error), isLoading: false });
    }
  },

  // Refresh feed (reload from beginning)
  refreshFeed: async () => {
    const generation = lifecycleGeneration;
    set({ isLoading: true, error: null, syncStatus: 'in_progress', syncError: null });
    try {
      await networkService.syncFeed(50);
      if (generation !== lifecycleGeneration) return;
      const prefs = readPreferences();
      const rawFeedItems = await decorateFeedItems(await feedService.getFeed(50));
      if (generation !== lifecycleGeneration) return;
      set({
        ...prefs,
        ...applyFeedState(rawFeedItems, prefs.hiddenPostIds, prefs.snoozedAuthors),
        isLoading: false,
        lastSyncAt: nowSeconds(),
        syncStatus: 'success',
        syncError: null,
        hasMore: rawFeedItems.length === 50,
      });

      // Load comment counts for refreshed feed
      if (rawFeedItems.length > 0) {
        const postIds = rawFeedItems.map((item) => item.postId);
        get().loadCommentCounts(postIds);
      }
    } catch (error) {
      if (generation !== lifecycleGeneration) return;
      log.error('Failed to refresh feed', error);
      set({
        error: getErrorMessage(error),
        isLoading: false,
        lastSyncAt: nowSeconds(),
        syncStatus: 'partial_failure',
        syncError: getErrorMessage(error),
      });
    }
  },

  // Sync feed from relay server (fetches contact walls via relay)
  syncFromRelay: async () => {
    const generation = lifecycleGeneration;
    const { isSyncingRelay } = get();
    if (isSyncingRelay) return; // Avoid concurrent syncs

    set({ isSyncingRelay: true, syncStatus: 'in_progress', syncError: null });
    try {
      await feedService.syncFromRelay();
      if (generation !== lifecycleGeneration) return;
      // Reload local feed to pick up any new posts from the relay
      const prefs = readPreferences();
      const rawFeedItems = await decorateFeedItems(await feedService.getFeed(50));
      if (generation !== lifecycleGeneration) return;
      set({
        ...prefs,
        ...applyFeedState(rawFeedItems, prefs.hiddenPostIds, prefs.snoozedAuthors),
        isSyncingRelay: false,
        lastSyncAt: nowSeconds(),
        syncStatus: 'success',
        syncError: null,
        hasMore: rawFeedItems.length === 50,
      });
      // Trigger background media preloader (best-effort, no error handling)
      mediaService
        .preloadMissingMedia()
        .catch((error) => log.warn('Background relay media preload is degraded', error));
    } catch (error) {
      if (generation !== lifecycleGeneration) return;
      log.warn('Failed to sync feed from relay', error);
      set({
        isSyncingRelay: false,
        lastSyncAt: nowSeconds(),
        syncStatus: 'partial_failure',
        syncError: getErrorMessage(error),
      });
      // Don't set error state — relay sync is best-effort
    }
  },

  // Load comments for a specific post
  loadComments: async (postId: string) => {
    const generation = lifecycleGeneration;
    const { loadingComments } = get();
    if (loadingComments.has(postId)) return;

    const newLoading = new Set(loadingComments);
    newLoading.add(postId);
    set({ loadingComments: newLoading });

    try {
      const comments = await commentsService.getComments(postId);
      if (generation !== lifecycleGeneration) return;
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
      if (generation !== lifecycleGeneration) return;
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
    const generation = lifecycleGeneration;
    try {
      const comment = await commentsService.addComment(postId, content);
      if (generation !== lifecycleGeneration) return;

      set((state) => {
        const existingComments = state.comments[postId] || [];
        const currentCount = state.commentCounts[postId] || 0;
        return {
          comments: { ...state.comments, [postId]: [...existingComments, comment] },
          commentCounts: { ...state.commentCounts, [postId]: currentCount + 1 },
        };
      });
    } catch (error) {
      if (generation === lifecycleGeneration) {
        log.error('Failed to add comment', error);
        set({ error: getErrorMessage(error) });
      }
      throw error;
    }
  },

  // Delete a comment
  deleteComment: async (postId: string, commentId: string) => {
    const generation = lifecycleGeneration;
    try {
      await commentsService.deleteComment(commentId);
      if (generation !== lifecycleGeneration) return;

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
      if (generation === lifecycleGeneration) {
        log.error('Failed to delete comment', error);
        set({ error: getErrorMessage(error) });
      }
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
    const generation = lifecycleGeneration;
    try {
      const counts = await commentsService.getCommentCounts(postIds);
      if (generation !== lifecycleGeneration) return;
      set((state) => {
        const newCounts = { ...state.commentCounts };
        for (const c of counts) {
          newCounts[c.postId] = c.count;
        }
        return { commentCounts: newCounts };
      });
    } catch (error) {
      if (generation !== lifecycleGeneration) return;
      log.error('Failed to load comment counts', error);
    }
  },
}));

export function hydrateFeedProfile(): void {
  useFeedStore.getState().hydratePreferences();
}

export function resetFeedProfileMemory(): void {
  lifecycleGeneration += 1;
  useFeedStore.setState({
    rawFeedItems: [],
    feedItems: [],
    savedPostIds: [],
    hiddenPostIds: [],
    snoozedAuthors: [],
    isLoading: false,
    isSyncingRelay: false,
    lastSyncAt: null,
    syncError: null,
    syncStatus: 'idle',
    error: null,
    hasMore: true,
    comments: {},
    commentCounts: {},
    expandedComments: new Set<string>(),
    loadingComments: new Set<string>(),
  });
}
