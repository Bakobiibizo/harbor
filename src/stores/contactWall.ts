import { create } from 'zustand';
import { feedService } from '../services/feed';
import { commentsService } from '../services/comments';
import { likesService, type LikeSummary } from '../services/likes';
import { permissionsService } from '../services/permissions';
import { mediaService } from '../services/media';
import type { FeedItem } from '../types';
import type { Comment } from '../services/comments';
import { createLogger } from '../utils/logger';
import { getErrorMessage } from '../utils/errors';

const log = createLogger('ContactWallStore');

export interface ContactWallState {
  authorPeerId: string | null;
  wallItems: FeedItem[];
  isLoading: boolean;
  isSyncing: boolean;
  error: string | null;
  syncError: string | null;
  lastSyncAt: number | null;
  syncStatus: 'idle' | 'in_progress' | 'success' | 'partial_failure';
  hasMore: boolean;
  canReadContactsOnly: boolean | null;
  comments: Record<string, Comment[]>;
  commentCounts: Record<string, number>;
  expandedComments: Set<string>;
  loadingComments: Set<string>;

  loadWall: (authorPeerId: string, limit?: number) => Promise<void>;
  loadMore: (limit?: number) => Promise<void>;
  refreshWall: (limit?: number) => Promise<void>;
  reconcileWall: (limit?: number) => Promise<void>;
  toggleLike: (postId: string) => Promise<void>;
  loadComments: (postId: string) => Promise<void>;
  addComment: (postId: string, content: string) => Promise<void>;
  deleteComment: (postId: string, commentId: string) => Promise<void>;
  toggleComments: (postId: string) => void;
  loadCommentCounts: (postIds: string[]) => Promise<void>;
  reset: () => void;
}

const initialState: Pick<
  ContactWallState,
  | 'authorPeerId'
  | 'wallItems'
  | 'isLoading'
  | 'isSyncing'
  | 'error'
  | 'syncError'
  | 'lastSyncAt'
  | 'syncStatus'
  | 'hasMore'
  | 'canReadContactsOnly'
  | 'comments'
  | 'commentCounts'
  | 'expandedComments'
  | 'loadingComments'
> = {
  authorPeerId: null,
  wallItems: [],
  isLoading: false,
  isSyncing: false,
  error: null,
  syncError: null,
  lastSyncAt: null,
  syncStatus: 'idle',
  hasMore: true,
  canReadContactsOnly: null,
  comments: {},
  commentCounts: {},
  expandedComments: new Set<string>(),
  loadingComments: new Set<string>(),
};

let lifecycleGeneration = 0;
let wallSelectionGeneration = 0;
let wallRequestGeneration = 0;
let commentCountsRequestGeneration = 0;

interface WallSelectionSnapshot {
  lifecycle: number;
  selection: number;
  authorPeerId: string | null;
}

function captureWallSelection(get: () => ContactWallState): WallSelectionSnapshot {
  return {
    lifecycle: lifecycleGeneration,
    selection: wallSelectionGeneration,
    authorPeerId: get().authorPeerId,
  };
}

function isCurrentWallSelection(
  get: () => ContactWallState,
  snapshot: WallSelectionSnapshot,
): boolean {
  return (
    snapshot.lifecycle === lifecycleGeneration &&
    snapshot.selection === wallSelectionGeneration &&
    snapshot.authorPeerId === get().authorPeerId
  );
}

async function loadPermission(authorPeerId: string): Promise<boolean> {
  try {
    return await permissionsService.weHaveCapability(authorPeerId, 'wall_read');
  } catch (error) {
    log.warn('Failed to check WallRead capability', error);
    return false;
  }
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

async function decorateWallItems(items: FeedItem[]): Promise<FeedItem[]> {
  if (items.length === 0) return items;
  const postIds = items.map((item) => item.postId);
  try {
    return mergeLikeSummaries(items, await likesService.getPostsLikesBatch(postIds));
  } catch (error) {
    log.warn('Failed to load contact wall reaction state', error);
    return mergeLikeSummaries(items, []);
  }
}

export const useContactWallStore = create<ContactWallState>((set, get) => ({
  ...initialState,

  loadWall: async (authorPeerId: string, limit: number = 20) => {
    const generation = lifecycleGeneration;
    const selection = ++wallSelectionGeneration;
    const request = ++wallRequestGeneration;
    const isCurrent = () =>
      generation === lifecycleGeneration &&
      selection === wallSelectionGeneration &&
      request === wallRequestGeneration &&
      get().authorPeerId === authorPeerId;
    set({
      authorPeerId,
      isLoading: true,
      error: null,
      syncError: null,
      wallItems: [],
      hasMore: true,
      comments: {},
      commentCounts: {},
      expandedComments: new Set<string>(),
      loadingComments: new Set<string>(),
    });

    let syncError: string | null = null;
    set({ isSyncing: true, syncStatus: 'in_progress' });
    try {
      await feedService.fetchContactWall(authorPeerId);
    } catch (error) {
      if (!isCurrent()) return;
      syncError = getErrorMessage(error);
      log.warn('Targeted contact wall relay sync failed', error);
    } finally {
      if (isCurrent()) set({ isSyncing: false });
    }

    if (!isCurrent()) return;

    try {
      const [canReadContactsOnly, wallItems] = await Promise.all([
        loadPermission(authorPeerId),
        feedService.getWall(authorPeerId, limit).then(decorateWallItems),
      ]);

      if (!isCurrent()) return;

      set({
        wallItems,
        canReadContactsOnly,
        isLoading: false,
        syncError,
        lastSyncAt: Math.floor(Date.now() / 1000),
        syncStatus: syncError ? 'partial_failure' : 'success',
        hasMore: wallItems.length === limit,
      });

      if (wallItems.length > 0) {
        feedService
          .fetchWallSocialEvents(
            authorPeerId,
            wallItems.map((item) => item.postId),
          )
          .catch((error) =>
            log.warn('Failed to fetch contact wall social events from relay', error),
          );
        get().loadCommentCounts(wallItems.map((item) => item.postId));
        mediaService
          .preloadMissingMedia()
          .catch((error) => log.warn('Background contact media preload is degraded', error));
      }
    } catch (error) {
      if (!isCurrent()) return;
      log.error('Failed to load contact wall', error);
      set({
        error: getErrorMessage(error),
        isLoading: false,
        syncError: syncError ?? getErrorMessage(error),
        lastSyncAt: Math.floor(Date.now() / 1000),
        syncStatus: 'partial_failure',
      });
    }
  },

  loadMore: async (limit: number = 20) => {
    const { authorPeerId, wallItems, isLoading, hasMore } = get();
    if (!authorPeerId || isLoading || !hasMore) return;
    const snapshot = captureWallSelection(get);
    const request = ++wallRequestGeneration;
    const isCurrent = () =>
      request === wallRequestGeneration && isCurrentWallSelection(get, snapshot);

    set({ isLoading: true, error: null });
    try {
      const lastItem = wallItems[wallItems.length - 1];
      const newItems = await decorateWallItems(
        await feedService.getWall(authorPeerId, limit, lastItem?.createdAt),
      );
      if (!isCurrent()) return;
      set({
        wallItems: [...wallItems, ...newItems],
        isLoading: false,
        hasMore: newItems.length === limit,
      });
      if (newItems.length > 0) {
        get().loadCommentCounts(newItems.map((item) => item.postId));
      }
    } catch (error) {
      if (!isCurrent()) return;
      log.error('Failed to load more contact wall posts', error);
      set({ error: getErrorMessage(error), isLoading: false });
    }
  },

  refreshWall: async (limit: number = 20) => {
    const { authorPeerId } = get();
    if (!authorPeerId) return;
    await get().loadWall(authorPeerId, limit);
  },

  // Local-only refresh used by the app-wide event reconciler. Deliberately does
  // not request another relay sync, which would turn sync events into a loop.
  reconcileWall: async (limit: number = 20) => {
    const { authorPeerId } = get();
    if (!authorPeerId) return;
    const snapshot = captureWallSelection(get);
    const request = ++wallRequestGeneration;
    const isCurrent = () =>
      request === wallRequestGeneration && isCurrentWallSelection(get, snapshot);

    try {
      const [canReadContactsOnly, wallItems] = await Promise.all([
        loadPermission(authorPeerId),
        feedService.getWall(authorPeerId, limit).then(decorateWallItems),
      ]);
      if (!isCurrent()) return;
      set({
        wallItems,
        canReadContactsOnly,
        error: null,
        hasMore: wallItems.length === limit,
      });
      if (wallItems.length > 0) {
        get().loadCommentCounts(wallItems.map((item) => item.postId));
      }
    } catch (error) {
      if (!isCurrent()) return;
      log.warn('Failed to reconcile contact wall from local state', error);
    }
  },

  toggleLike: async (postId: string) => {
    const snapshot = captureWallSelection(get);
    const existing = get().wallItems.find((item) => item.postId === postId);
    const summary = existing?.likedByUser
      ? await likesService.unlikePost(postId)
      : await likesService.likePost(postId);
    if (!isCurrentWallSelection(get, snapshot)) return;
    feedService
      .syncWallSocialEventsToRelay()
      .catch((error) => log.warn('Failed to sync contact wall reaction event to relay', error));

    set((state) => ({
      wallItems: state.wallItems.map((item) =>
        item.postId === postId
          ? { ...item, likes: summary.totalLikes, likedByUser: summary.userHasLiked }
          : item,
      ),
    }));
  },

  loadComments: async (postId: string) => {
    const snapshot = captureWallSelection(get);
    const { loadingComments } = get();
    if (loadingComments.has(postId)) return;

    const nextLoading = new Set(loadingComments);
    nextLoading.add(postId);
    set({ loadingComments: nextLoading });

    try {
      const comments = await commentsService.getComments(postId);
      if (!isCurrentWallSelection(get, snapshot)) return;
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
      if (!isCurrentWallSelection(get, snapshot)) return;
      log.error('Failed to load contact wall comments', error);
      set((state) => {
        const newLoading = new Set(state.loadingComments);
        newLoading.delete(postId);
        return { loadingComments: newLoading };
      });
    }
  },

  addComment: async (postId: string, content: string) => {
    const snapshot = captureWallSelection(get);
    const comment = await commentsService.addComment(postId, content);
    if (!isCurrentWallSelection(get, snapshot)) return;
    feedService
      .syncWallSocialEventsToRelay()
      .catch((error) => log.warn('Failed to sync contact wall comment event to relay', error));
    set((state) => ({
      comments: { ...state.comments, [postId]: [...(state.comments[postId] || []), comment] },
      commentCounts: { ...state.commentCounts, [postId]: (state.commentCounts[postId] || 0) + 1 },
    }));
  },

  deleteComment: async (postId: string, commentId: string) => {
    const snapshot = captureWallSelection(get);
    await commentsService.deleteComment(commentId);
    if (!isCurrentWallSelection(get, snapshot)) return;
    set((state) => ({
      comments: {
        ...state.comments,
        [postId]: (state.comments[postId] || []).filter(
          (comment) => comment.commentId !== commentId,
        ),
      },
      commentCounts: {
        ...state.commentCounts,
        [postId]: Math.max(0, (state.commentCounts[postId] || 0) - 1),
      },
    }));
  },

  toggleComments: (postId: string) => {
    set((state) => {
      const expandedComments = new Set(state.expandedComments);
      if (expandedComments.has(postId)) {
        expandedComments.delete(postId);
      } else {
        expandedComments.add(postId);
        if (!state.comments[postId]) {
          get().loadComments(postId);
        }
      }
      return { expandedComments };
    });
  },

  loadCommentCounts: async (postIds: string[]) => {
    const snapshot = captureWallSelection(get);
    const request = ++commentCountsRequestGeneration;
    try {
      const counts = await commentsService.getCommentCounts(postIds);
      if (request !== commentCountsRequestGeneration || !isCurrentWallSelection(get, snapshot))
        return;
      set((state) => {
        const commentCounts = { ...state.commentCounts };
        for (const count of counts) {
          commentCounts[count.postId] = count.count;
        }
        return { commentCounts };
      });
    } catch (error) {
      if (request !== commentCountsRequestGeneration || !isCurrentWallSelection(get, snapshot))
        return;
      log.error('Failed to load contact wall comment counts', error);
    }
  },

  reset: () => {
    lifecycleGeneration += 1;
    wallSelectionGeneration += 1;
    wallRequestGeneration += 1;
    commentCountsRequestGeneration += 1;
    set({
      ...initialState,
      wallItems: [],
      comments: {},
      commentCounts: {},
      expandedComments: new Set(),
      loadingComments: new Set(),
    });
  },
}));
