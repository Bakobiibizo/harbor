import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useFeedStore } from './feed';
import { feedService } from '../services/feed';
import { commentsService } from '../services/comments';
import * as networkService from '../services/network';

vi.mock('../services/feed', () => ({
  feedService: {
    getFeed: vi.fn(),
    getWall: vi.fn(),
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
      feedItems: [],
      isLoading: false,
      error: null,
      hasMore: true,
      comments: {},
      commentCounts: {},
      expandedComments: new Set(),
      loadingComments: new Set(),
    });
    vi.clearAllMocks();
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

    it('should handle refresh errors', async () => {
      vi.mocked(networkService.syncFeed).mockRejectedValue(new Error('Sync failed'));

      await useFeedStore.getState().refreshFeed();

      expect(useFeedStore.getState().error).toContain('Sync failed');
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
