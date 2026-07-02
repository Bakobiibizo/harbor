import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useFeedStore } from './feed';
import { feedService } from '../services/feed';
import { commentsService } from '../services/comments';
import { likesService } from '../services/likes';
import * as networkService from '../services/network';

vi.mock('../services/feed', () => ({
  feedService: {
    getFeed: vi.fn(),
    getWall: vi.fn(),
    syncFromRelay: vi.fn(),
  },
}));

vi.mock('../services/comments', () => ({
  commentsService: {
    getComments: vi.fn(),
    addComment: vi.fn(),
    deleteComment: vi.fn(),
    getCommentCounts: vi.fn(),
  },
}));

vi.mock('../services/likes', () => ({
  likesService: {
    getPostsLikesBatch: vi.fn(),
    likePost: vi.fn(),
    unlikePost: vi.fn(),
  },
}));

vi.mock('../services/network', () => ({
  syncFeed: vi.fn(),
}));

const mockFeedItems = [
  {
    postId: 'feed-1',
    authorPeerId: 'peer-alice',
    authorDisplayName: 'Alice',
    contentType: 'text',
    contentText: 'Hello from Alice',
    visibility: 'contacts',
    lamportClock: 1,
    createdAt: 1700000100,
    updatedAt: 1700000100,
    isLocal: false,
  },
  {
    postId: 'feed-2',
    authorPeerId: 'peer-bob',
    authorDisplayName: 'Bob',
    contentType: 'text',
    contentText: 'Hello from Bob',
    visibility: 'contacts',
    lamportClock: 2,
    createdAt: 1700000000,
    updatedAt: 1700000000,
    isLocal: false,
  },
];

describe('useFeedStore', () => {
  beforeEach(() => {
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
      expandedComments: new Set(),
      loadingComments: new Set(),
    });
    localStorage.clear();
    vi.mocked(likesService.getPostsLikesBatch).mockResolvedValue([]);
    vi.clearAllMocks();
    vi.mocked(likesService.getPostsLikesBatch).mockResolvedValue([]);
  });

  describe('loadFeed', () => {
    it('should load feed items', async () => {
      vi.mocked(feedService.getFeed).mockResolvedValue(mockFeedItems);
      vi.mocked(commentsService.getCommentCounts).mockResolvedValue([]);

      await useFeedStore.getState().loadFeed();

      const state = useFeedStore.getState();
      expect(state.feedItems).toHaveLength(2);
      expect(state.isLoading).toBe(false);
      expect(state.error).toBeNull();
    });

    it('should set hasMore based on returned count vs limit', async () => {
      // Returns less than limit, meaning no more items
      vi.mocked(feedService.getFeed).mockResolvedValue(mockFeedItems);
      vi.mocked(commentsService.getCommentCounts).mockResolvedValue([]);

      await useFeedStore.getState().loadFeed(50);

      expect(useFeedStore.getState().hasMore).toBe(false); // 2 < 50
    });

    it('should set hasMore to true when result count equals limit', async () => {
      const items = Array(10)
        .fill(null)
        .map((_, i) => ({
          ...mockFeedItems[0],
          postId: `feed-${i}`,
        }));
      vi.mocked(feedService.getFeed).mockResolvedValue(items);
      vi.mocked(commentsService.getCommentCounts).mockResolvedValue([]);

      await useFeedStore.getState().loadFeed(10);

      expect(useFeedStore.getState().hasMore).toBe(true);
    });

    it('should handle load errors', async () => {
      vi.mocked(feedService.getFeed).mockRejectedValue(new Error('Feed error'));

      await useFeedStore.getState().loadFeed();

      const state = useFeedStore.getState();
      expect(state.isLoading).toBe(false);
      expect(state.error).toContain('Feed error');
    });

    it('should trigger comment count loading for feed items', async () => {
      vi.mocked(feedService.getFeed).mockResolvedValue(mockFeedItems);
      vi.mocked(commentsService.getCommentCounts).mockResolvedValue([
        { postId: 'feed-1', count: 3 },
        { postId: 'feed-2', count: 0 },
      ]);

      await useFeedStore.getState().loadFeed();

      // Verify that getCommentCounts was called with the post IDs
      expect(commentsService.getCommentCounts).toHaveBeenCalledWith(['feed-1', 'feed-2']);
    });
  });

  describe('loadMore', () => {
    it('should append new items to existing feed', async () => {
      useFeedStore.setState({ feedItems: mockFeedItems, hasMore: true });

      const newItems = [
        {
          ...mockFeedItems[0],
          postId: 'feed-3',
          createdAt: 1699999900,
        },
      ];
      vi.mocked(feedService.getFeed).mockResolvedValue(newItems);
      vi.mocked(commentsService.getCommentCounts).mockResolvedValue([]);

      await useFeedStore.getState().loadMore();

      expect(useFeedStore.getState().feedItems).toHaveLength(3);
    });

    it('should not load more if already loading', async () => {
      useFeedStore.setState({ isLoading: true, hasMore: true });

      await useFeedStore.getState().loadMore();

      expect(feedService.getFeed).not.toHaveBeenCalled();
    });

    it('should not load more if hasMore is false', async () => {
      useFeedStore.setState({ hasMore: false });

      await useFeedStore.getState().loadMore();

      expect(feedService.getFeed).not.toHaveBeenCalled();
    });
  });

  describe('refreshFeed', () => {
    it('should sync and reload feed', async () => {
      vi.mocked(networkService.syncFeed).mockResolvedValue(undefined);
      vi.mocked(feedService.getFeed).mockResolvedValue(mockFeedItems);
      vi.mocked(commentsService.getCommentCounts).mockResolvedValue([]);

      await useFeedStore.getState().refreshFeed();

      expect(networkService.syncFeed).toHaveBeenCalledWith(50);
      expect(feedService.getFeed).toHaveBeenCalledWith(50);
      expect(useFeedStore.getState().feedItems).toHaveLength(2);
    });

    it('should handle refresh errors as partial sync failures', async () => {
      vi.mocked(networkService.syncFeed).mockRejectedValue(new Error('Sync failed'));

      await useFeedStore.getState().refreshFeed();

      const state = useFeedStore.getState();
      expect(state.error).toContain('Sync failed');
      expect(state.syncStatus).toBe('partial_failure');
      expect(state.syncError).toContain('Sync failed');
      expect(state.lastSyncAt).toBeGreaterThan(0);
    });
  });

  describe('relay sync status', () => {
    it('records partial failure without replacing locally loaded feed', async () => {
      useFeedStore.setState({ rawFeedItems: mockFeedItems, feedItems: mockFeedItems });
      vi.mocked(feedService.syncFromRelay).mockRejectedValue(new Error('relay offline'));

      await useFeedStore.getState().syncFromRelay();

      const state = useFeedStore.getState();
      expect(state.feedItems).toHaveLength(2);
      expect(state.syncStatus).toBe('partial_failure');
      expect(state.syncError).toContain('relay offline');
      expect(state.lastSyncAt).toBeGreaterThan(0);
    });
  });

  describe('durable feed interactions', () => {
    it('loads signed reaction summaries and toggles like state with backend results', async () => {
      vi.mocked(feedService.getFeed).mockResolvedValue(mockFeedItems);
      vi.mocked(commentsService.getCommentCounts).mockResolvedValue([]);
      vi.mocked(likesService.getPostsLikesBatch).mockResolvedValue([
        { postId: 'feed-1', totalLikes: 2, userHasLiked: false },
        { postId: 'feed-2', totalLikes: 5, userHasLiked: true },
      ]);

      await useFeedStore.getState().loadFeed();

      expect(likesService.getPostsLikesBatch).toHaveBeenCalledWith(['feed-1', 'feed-2']);
      expect(useFeedStore.getState().feedItems[0]).toMatchObject({
        postId: 'feed-1',
        likes: 2,
        likedByUser: false,
      });

      vi.mocked(likesService.likePost).mockResolvedValue({
        postId: 'feed-1',
        totalLikes: 3,
        userHasLiked: true,
      });

      await useFeedStore.getState().toggleLike('feed-1');

      expect(likesService.likePost).toHaveBeenCalledWith('feed-1');
      expect(useFeedStore.getState().feedItems[0]).toMatchObject({
        likes: 3,
        likedByUser: true,
      });

      vi.mocked(likesService.unlikePost).mockResolvedValue({
        postId: 'feed-1',
        totalLikes: 2,
        userHasLiked: false,
      });

      await useFeedStore.getState().toggleLike('feed-1');

      expect(likesService.unlikePost).toHaveBeenCalledWith('feed-1');
      expect(useFeedStore.getState().feedItems[0]).toMatchObject({
        likes: 2,
        likedByUser: false,
      });
    });

    it('persists saved posts locally and restores saved tab inputs without duplicates', async () => {
      vi.mocked(feedService.getFeed).mockResolvedValue(mockFeedItems);
      vi.mocked(commentsService.getCommentCounts).mockResolvedValue([]);

      await useFeedStore.getState().loadFeed();
      useFeedStore.getState().toggleSave('feed-1');
      useFeedStore.getState().toggleSave('feed-1');
      useFeedStore.getState().toggleSave('feed-1');

      expect(useFeedStore.getState().savedPostIds).toEqual(['feed-1']);
      expect(useFeedStore.getState().getSavedFeedItems().map((p) => p.postId)).toEqual(['feed-1']);

      useFeedStore.setState({ savedPostIds: [] });
      useFeedStore.getState().hydratePreferences();

      expect(useFeedStore.getState().savedPostIds).toEqual(['feed-1']);
      expect(useFeedStore.getState().getSavedFeedItems().map((p) => p.postId)).toEqual(['feed-1']);
    });

    it('persists hidden posts and restores them through management controls without deleting raw feed', async () => {
      useFeedStore.setState({ rawFeedItems: mockFeedItems, feedItems: mockFeedItems });

      useFeedStore.getState().hidePost('feed-1');

      expect(useFeedStore.getState().feedItems.map((p) => p.postId)).toEqual(['feed-2']);
      expect(useFeedStore.getState().rawFeedItems.map((p) => p.postId)).toEqual(['feed-1', 'feed-2']);

      useFeedStore.setState({ hiddenPostIds: [], feedItems: mockFeedItems });
      useFeedStore.getState().hydratePreferences();

      expect(useFeedStore.getState().feedItems.map((p) => p.postId)).toEqual(['feed-2']);

      useFeedStore.getState().unhidePost('feed-1');

      expect(useFeedStore.getState().feedItems.map((p) => p.postId)).toEqual(['feed-1', 'feed-2']);
    });

    it('persists snoozed authors, filters deterministically, and expires old snoozes on reload', () => {
      useFeedStore.setState({ rawFeedItems: mockFeedItems, feedItems: mockFeedItems });

      useFeedStore.getState().snoozeAuthor('peer-alice', 24);

      expect(useFeedStore.getState().feedItems.map((p) => p.postId)).toEqual(['feed-2']);

      const expiredPrefs = {
        savedPostIds: [],
        hiddenPostIds: [],
        snoozedAuthors: [{ peerId: 'peer-alice', snoozedUntil: 1 }],
      };
      localStorage.setItem('harbor-feed-local-preferences-v1', JSON.stringify(expiredPrefs));
      useFeedStore.getState().hydratePreferences();

      expect(useFeedStore.getState().snoozedAuthors).toEqual([]);
      expect(useFeedStore.getState().feedItems.map((p) => p.postId)).toEqual(['feed-1', 'feed-2']);
    });
  });

  describe('comments', () => {
    it('should load comments for a post', async () => {
      const mockComments = [
        {
          id: 1,
          commentId: 'comment-1',
          postId: 'feed-1',
          authorPeerId: 'peer-alice',
          authorName: 'Alice',
          content: 'Nice post!',
          createdAt: 1700000200,
          deletedAt: null,
        },
      ];
      vi.mocked(commentsService.getComments).mockResolvedValue(mockComments);

      await useFeedStore.getState().loadComments('feed-1');

      const state = useFeedStore.getState();
      expect(state.comments['feed-1']).toEqual(mockComments);
      expect(state.commentCounts['feed-1']).toBe(1);
      expect(state.loadingComments.has('feed-1')).toBe(false);
    });

    it('should not duplicate load if already loading', async () => {
      useFeedStore.setState({
        loadingComments: new Set(['feed-1']),
      });

      await useFeedStore.getState().loadComments('feed-1');

      expect(commentsService.getComments).not.toHaveBeenCalled();
    });

    it('should add a comment to a post', async () => {
      const newComment = {
        id: 2,
        commentId: 'comment-2',
        postId: 'feed-1',
        authorPeerId: 'peer-me',
        authorName: 'Me',
        content: 'Great!',
        createdAt: 1700000300,
        deletedAt: null,
      };
      vi.mocked(commentsService.addComment).mockResolvedValue(newComment);

      await useFeedStore.getState().addComment('feed-1', 'Great!');

      const state = useFeedStore.getState();
      expect(state.comments['feed-1']).toEqual([newComment]);
      expect(state.commentCounts['feed-1']).toBe(1);
    });

    it('should delete a comment from a post', async () => {
      useFeedStore.setState({
        comments: {
          'feed-1': [
            {
              id: 1,
              commentId: 'comment-1',
              postId: 'feed-1',
              authorPeerId: 'peer-me',
              authorName: 'Me',
              content: 'To delete',
              createdAt: 1700000200,
              deletedAt: null,
            },
            {
              id: 2,
              commentId: 'comment-2',
              postId: 'feed-1',
              authorPeerId: 'peer-alice',
              authorName: 'Alice',
              content: 'Keep this',
              createdAt: 1700000250,
              deletedAt: null,
            },
          ],
        },
        commentCounts: { 'feed-1': 2 },
      });

      vi.mocked(commentsService.deleteComment).mockResolvedValue(true);

      await useFeedStore.getState().deleteComment('feed-1', 'comment-1');

      const state = useFeedStore.getState();
      expect(state.comments['feed-1']).toHaveLength(1);
      expect(state.comments['feed-1'][0].commentId).toBe('comment-2');
      expect(state.commentCounts['feed-1']).toBe(1);
    });

    it('should not go below 0 for comment count', async () => {
      useFeedStore.setState({
        comments: { 'feed-1': [] },
        commentCounts: { 'feed-1': 0 },
      });

      vi.mocked(commentsService.deleteComment).mockResolvedValue(true);

      await useFeedStore.getState().deleteComment('feed-1', 'nonexistent');

      expect(useFeedStore.getState().commentCounts['feed-1']).toBe(0);
    });
  });

  describe('toggleComments', () => {
    it('should expand comments and trigger load if not loaded', () => {
      vi.mocked(commentsService.getComments).mockResolvedValue([]);

      useFeedStore.getState().toggleComments('feed-1');

      expect(useFeedStore.getState().expandedComments.has('feed-1')).toBe(true);
      expect(commentsService.getComments).toHaveBeenCalledWith('feed-1');
    });

    it('should collapse comments if already expanded', () => {
      useFeedStore.setState({
        expandedComments: new Set(['feed-1']),
      });

      useFeedStore.getState().toggleComments('feed-1');

      expect(useFeedStore.getState().expandedComments.has('feed-1')).toBe(false);
    });

    it('should not reload comments if already loaded', () => {
      useFeedStore.setState({
        comments: { 'feed-1': [] },
      });

      useFeedStore.getState().toggleComments('feed-1');

      expect(commentsService.getComments).not.toHaveBeenCalled();
    });
  });
});
