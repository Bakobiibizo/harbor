import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useWallStore } from './wall';
import { useSettingsStore } from './settings';
import { postsService } from '../services/posts';
import { mediaService } from '../services/media';
import { commentsService } from '../services/comments';
import { feedService } from '../services/feed';
import { likesService } from '../services/likes';

vi.mock('../services/posts', () => ({
  postsService: {
    getMyPosts: vi.fn(),
    getPostMedia: vi.fn(),
    createPost: vi.fn(),
    updatePost: vi.fn(),
    deletePost: vi.fn(),
    addPostMedia: vi.fn(),
  },
}));

vi.mock('../services/media', () => ({
  mediaService: {
    storeMediaBytes: vi.fn(),
    getMediaUrl: vi.fn(),
  },
}));

vi.mock('../services/comments', () => ({
  commentsService: {
    getCommentCounts: vi.fn(),
    getComments: vi.fn(),
    addComment: vi.fn(),
    deleteComment: vi.fn(),
  },
}));

vi.mock('../services/likes', () => ({
  likesService: {
    getPostsLikesBatch: vi.fn(),
    likePost: vi.fn(),
    unlikePost: vi.fn(),
  },
}));

vi.mock('../services/feed', () => ({
  feedService: {
    syncWallToRelay: vi.fn(() => Promise.resolve()),
    syncWallSocialEventsToRelay: vi.fn(() => Promise.resolve(0)),
    fetchWallSocialEvents: vi.fn(() => Promise.resolve()),
  },
}));

const mockBackendPost = {
  postId: 'post-1',
  authorPeerId: 'peer-abc',
  contentType: 'text',
  contentText: 'Hello world',
  visibility: 'contacts' as const,
  lamportClock: 1,
  createdAt: 1700000000,
  updatedAt: 1700000000,
  deletedAt: null,
  isLocal: true,
};

describe('useWallStore', () => {
  beforeEach(() => {
    useWallStore.setState({
      posts: [],
      isLoading: false,
      error: null,
      editingPostId: null,
      commentsByPost: {},
      expandedComments: new Set(),
      loadingComments: new Set(),
    });
    useSettingsStore.setState({ defaultVisibility: 'contacts' });
    vi.clearAllMocks();
    vi.mocked(commentsService.getCommentCounts).mockResolvedValue([]);
    vi.mocked(commentsService.getComments).mockResolvedValue([]);
    vi.mocked(likesService.getPostsLikesBatch).mockResolvedValue([]);
    vi.mocked(feedService.syncWallToRelay).mockResolvedValue(undefined);
    vi.mocked(feedService.syncWallSocialEventsToRelay).mockResolvedValue(0);
    vi.mocked(feedService.fetchWallSocialEvents).mockResolvedValue(undefined);
  });

  describe('loadPosts', () => {
    it('should load posts from backend and convert to WallPost format', async () => {
      vi.mocked(postsService.getMyPosts).mockResolvedValue([mockBackendPost]);
      vi.mocked(postsService.getPostMedia).mockResolvedValue([]);

      await useWallStore.getState().loadPosts();

      const state = useWallStore.getState();
      expect(state.isLoading).toBe(false);
      expect(state.posts).toHaveLength(1);
      expect(state.posts[0].postId).toBe('post-1');
      expect(state.posts[0].content).toBe('Hello world');
      expect(state.posts[0].contentType).toBe('post'); // 'text' maps to 'post'
      expect(state.posts[0].likes).toBe(0);
      expect(state.posts[0].comments).toBe(0);
      expect(state.posts[0].liked).toBe(false);
    });

    it('should set isLoading during load', async () => {
      let resolvePromise: (value: never[]) => void;
      vi.mocked(postsService.getMyPosts).mockReturnValue(
        new Promise((resolve) => {
          resolvePromise = resolve;
        }),
      );

      const loadPromise = useWallStore.getState().loadPosts();
      expect(useWallStore.getState().isLoading).toBe(true);

      resolvePromise!([]);
      await loadPromise;
      expect(useWallStore.getState().isLoading).toBe(false);
    });

    it('should handle load errors', async () => {
      vi.mocked(postsService.getMyPosts).mockRejectedValue(new Error('Network error'));

      await useWallStore.getState().loadPosts();

      const state = useWallStore.getState();
      expect(state.isLoading).toBe(false);
      expect(state.error).toContain('Network error');
    });

    it('should render image and video media after reload or sync', async () => {
      vi.mocked(postsService.getMyPosts).mockResolvedValue([mockBackendPost]);
      vi.mocked(postsService.getPostMedia).mockResolvedValue([
        {
          id: 1,
          postId: 'post-1',
          mediaHash: 'a'.repeat(64),
          mediaType: 'image',
          mimeType: 'image/png',
          fileName: 'photo.png',
          fileSize: 100,
          width: null,
          height: null,
          durationSeconds: null,
          sortOrder: 0,
          signature: [1, 2, 3],
        },
        {
          id: 2,
          postId: 'post-1',
          mediaHash: 'b'.repeat(64),
          mediaType: 'video',
          mimeType: 'video/mp4',
          fileName: 'clip.mp4',
          fileSize: 200,
          width: null,
          height: null,
          durationSeconds: 2,
          sortOrder: 1,
          signature: [4, 5, 6],
        },
      ]);
      vi.mocked(mediaService.getMediaUrl)
        .mockResolvedValueOnce('data:image/png;base64,image')
        .mockResolvedValueOnce('data:video/mp4;base64,video');

      await useWallStore.getState().loadPosts();

      expect(useWallStore.getState().posts[0].media).toEqual([
        { type: 'image', url: 'data:image/png;base64,image', name: 'photo.png' },
        { type: 'video', url: 'data:video/mp4;base64,video', name: 'clip.mp4' },
      ]);
    });

    it('should handle media fetch errors gracefully per post', async () => {
      vi.mocked(postsService.getMyPosts).mockResolvedValue([mockBackendPost]);
      vi.mocked(postsService.getPostMedia).mockRejectedValue(new Error('Media error'));

      await useWallStore.getState().loadPosts();

      // Should still load the post, just without media
      expect(useWallStore.getState().posts).toHaveLength(1);
      expect(useWallStore.getState().posts[0].media).toBeUndefined();
    });

    it('should load persisted backend comment and like counts', async () => {
      vi.mocked(postsService.getMyPosts).mockResolvedValue([mockBackendPost]);
      vi.mocked(postsService.getPostMedia).mockResolvedValue([]);
      vi.mocked(likesService.getPostsLikesBatch).mockResolvedValue([
        { postId: 'post-1', totalLikes: 3, userHasLiked: true },
      ]);
      vi.mocked(commentsService.getCommentCounts).mockResolvedValue([
        { postId: 'post-1', count: 2 },
      ]);

      await useWallStore.getState().loadPosts();

      expect(likesService.getPostsLikesBatch).toHaveBeenCalledWith(['post-1']);
      expect(commentsService.getCommentCounts).toHaveBeenCalledWith(['post-1']);
      expect(feedService.fetchWallSocialEvents).toHaveBeenCalledWith('peer-abc', ['post-1']);
      expect(useWallStore.getState().posts[0]).toMatchObject({
        likes: 3,
        liked: true,
        comments: 2,
      });
    });
  });

  describe('createPost', () => {
    it('should create a post and add it to local state', async () => {
      vi.mocked(postsService.createPost).mockResolvedValue({
        postId: 'new-post-1',
        createdAt: 1700000100,
      });

      await useWallStore.getState().createPost('New post content');

      const state = useWallStore.getState();
      expect(state.posts).toHaveLength(1);
      expect(state.posts[0].postId).toBe('new-post-1');
      expect(state.posts[0].content).toBe('New post content');
      expect(state.posts[0].contentType).toBe('post');
      expect(state.posts[0].visibility).toBe('contacts');
      expect(postsService.createPost).toHaveBeenCalledWith(
        'text',
        'New post content',
        'contacts',
        undefined,
      );
    });

    it('should use persisted default visibility from settings', async () => {
      useSettingsStore.setState({ defaultVisibility: 'public' });
      vi.mocked(postsService.createPost).mockResolvedValue({
        postId: 'public-post',
        createdAt: 1700000100,
      });

      await useWallStore.getState().createPost('Public by default');

      expect(postsService.createPost).toHaveBeenCalledWith(
        'text',
        'Public by default',
        'public',
        undefined,
      );
      expect(useWallStore.getState().posts[0].visibility).toBe('public');
    });

    it('should allow a per-post visibility override', async () => {
      useSettingsStore.setState({ defaultVisibility: 'public' });
      vi.mocked(postsService.createPost).mockResolvedValue({
        postId: 'contacts-post',
        createdAt: 1700000100,
      });

      await useWallStore.getState().createPost('Contacts override', 'post', undefined, 'contacts');

      expect(postsService.createPost).toHaveBeenCalledWith(
        'text',
        'Contacts override',
        'contacts',
        undefined,
      );
      expect(useWallStore.getState().posts[0].visibility).toBe('contacts');
    });

    it('should prepend new posts to the beginning', async () => {
      // Set up existing post
      useWallStore.setState({
        posts: [
          {
            postId: 'old-post',
            content: 'Old post',
            contentType: 'post',
            timestamp: new Date(1700000000000),
            likes: 0,
            comments: 0,
            liked: false,
            authorPeerId: 'peer-abc',
            visibility: 'contacts',
            lamportClock: 0,
          },
        ],
      });

      vi.mocked(postsService.createPost).mockResolvedValue({
        postId: 'new-post',
        createdAt: 1700000100,
      });

      await useWallStore.getState().createPost('New post');

      const posts = useWallStore.getState().posts;
      expect(posts).toHaveLength(2);
      expect(posts[0].postId).toBe('new-post');
      expect(posts[1].postId).toBe('old-post');
    });

    it('should handle create errors', async () => {
      vi.mocked(postsService.createPost).mockRejectedValue(new Error('Create failed'));

      await expect(useWallStore.getState().createPost('content')).rejects.toThrow('Create failed');
    });

    it('should sign image media at create time when provided', async () => {
      vi.mocked(postsService.createPost).mockResolvedValue({
        postId: 'media-post',
        createdAt: 1700000100,
      });
      vi.mocked(mediaService.storeMediaBytes).mockResolvedValue('a'.repeat(64));

      const file = {
        type: 'image/png',
        arrayBuffer: vi.fn(async () => new Uint8Array([1, 2, 3]).buffer),
      } as unknown as File;
      const media = [{ type: 'image' as const, url: 'blob:test', name: 'photo.png', file }];
      await useWallStore.getState().createPost('Post with image', 'post', media);

      expect(mediaService.storeMediaBytes).toHaveBeenCalledTimes(1);
      expect(postsService.createPost).toHaveBeenCalledWith('text', 'Post with image', 'contacts', [
        {
          mediaHash: 'a'.repeat(64),
          mediaType: 'image',
          mimeType: 'image/png',
          fileName: 'photo.png',
          fileSize: 3,
          sortOrder: 0,
        },
      ]);
      expect(postsService.addPostMedia).not.toHaveBeenCalled();
      const state = useWallStore.getState();
      expect(state.posts[0].media).toEqual(
        media.map(({ type, url, name }) => ({ type, url, name })),
      );
    });

    it('should sign video media at create time when provided', async () => {
      vi.mocked(postsService.createPost).mockResolvedValue({
        postId: 'video-post',
        createdAt: 1700000100,
      });
      vi.mocked(mediaService.storeMediaBytes).mockResolvedValue('b'.repeat(64));

      const file = {
        type: 'video/mp4',
        arrayBuffer: vi.fn(async () => new Uint8Array([4, 5, 6, 7]).buffer),
      } as unknown as File;
      await useWallStore
        .getState()
        .createPost('Post with video', 'video', [
          { type: 'video', url: 'blob:video', name: 'clip.mp4', file },
        ]);

      expect(postsService.createPost).toHaveBeenCalledWith('video', 'Post with video', 'contacts', [
        {
          mediaHash: 'b'.repeat(64),
          mediaType: 'video',
          mimeType: 'video/mp4',
          fileName: 'clip.mp4',
          fileSize: 4,
          sortOrder: 0,
        },
      ]);
    });
  });

  describe('updatePost', () => {
    it('should update post content in local state', async () => {
      useWallStore.setState({
        posts: [
          {
            postId: 'post-1',
            content: 'Original content',
            contentType: 'post',
            timestamp: new Date(),
            likes: 0,
            comments: 0,
            liked: false,
            authorPeerId: 'peer-abc',
            visibility: 'contacts',
            lamportClock: 0,
          },
        ],
      });

      vi.mocked(postsService.updatePost).mockResolvedValue(undefined);

      await useWallStore.getState().updatePost('post-1', 'Updated content');

      const state = useWallStore.getState();
      expect(state.posts[0].content).toBe('Updated content');
      expect(state.editingPostId).toBeNull();
    });

    it('should throw on update failure', async () => {
      useWallStore.setState({
        posts: [
          {
            postId: 'post-1',
            content: 'Original',
            contentType: 'post',
            timestamp: new Date(),
            likes: 0,
            comments: 0,
            liked: false,
            authorPeerId: 'peer-abc',
            visibility: 'contacts',
            lamportClock: 0,
          },
        ],
      });

      vi.mocked(postsService.updatePost).mockRejectedValue(new Error('Update failed'));

      await expect(useWallStore.getState().updatePost('post-1', 'new content')).rejects.toThrow(
        'Update failed',
      );
    });
  });

  describe('deletePost', () => {
    it('should remove post from local state', async () => {
      useWallStore.setState({
        posts: [
          {
            postId: 'post-1',
            content: 'To delete',
            contentType: 'post',
            timestamp: new Date(),
            likes: 0,
            comments: 0,
            liked: false,
            authorPeerId: 'peer-abc',
            visibility: 'contacts',
            lamportClock: 0,
          },
          {
            postId: 'post-2',
            content: 'To keep',
            contentType: 'post',
            timestamp: new Date(),
            likes: 0,
            comments: 0,
            liked: false,
            authorPeerId: 'peer-abc',
            visibility: 'contacts',
            lamportClock: 0,
          },
        ],
      });

      vi.mocked(postsService.deletePost).mockResolvedValue(undefined);

      await useWallStore.getState().deletePost('post-1');

      const posts = useWallStore.getState().posts;
      expect(posts).toHaveLength(1);
      expect(posts[0].postId).toBe('post-2');
    });
  });

  describe('likePost', () => {
    const makePost = (postId: string, liked = false, likes = 0) => ({
      postId,
      content: 'test',
      contentType: 'post' as const,
      timestamp: new Date(),
      likes,
      comments: 0,
      liked,
      authorPeerId: 'peer-abc',
      visibility: 'contacts',
      lamportClock: 0,
    });

    it('should toggle like status on using backend reaction state', async () => {
      useWallStore.setState({ posts: [makePost('post-1', false, 0)] });
      vi.mocked(likesService.likePost).mockResolvedValue({
        postId: 'post-1',
        totalLikes: 1,
        userHasLiked: true,
      });

      await useWallStore.getState().likePost('post-1');

      expect(likesService.likePost).toHaveBeenCalledWith('post-1');
      expect(feedService.syncWallSocialEventsToRelay).toHaveBeenCalledTimes(1);
      const post = useWallStore.getState().posts[0];
      expect(post.liked).toBe(true);
      expect(post.likes).toBe(1);
    });

    it('should toggle like status off using backend reaction state', async () => {
      useWallStore.setState({ posts: [makePost('post-1', true, 1)] });
      vi.mocked(likesService.unlikePost).mockResolvedValue({
        postId: 'post-1',
        totalLikes: 0,
        userHasLiked: false,
      });

      await useWallStore.getState().likePost('post-1');

      expect(likesService.unlikePost).toHaveBeenCalledWith('post-1');
      expect(feedService.syncWallSocialEventsToRelay).toHaveBeenCalledTimes(1);
      const post = useWallStore.getState().posts[0];
      expect(post.liked).toBe(false);
      expect(post.likes).toBe(0);
    });

    it('should only affect the target post', async () => {
      useWallStore.setState({
        posts: [makePost('post-1', false, 0), makePost('post-2', false, 0)],
      });
      vi.mocked(likesService.likePost).mockResolvedValue({
        postId: 'post-1',
        totalLikes: 1,
        userHasLiked: true,
      });

      await useWallStore.getState().likePost('post-1');

      const posts = useWallStore.getState().posts;
      expect(posts[0].liked).toBe(true);
      expect(posts[1].liked).toBe(false);
    });
  });

  describe('comments', () => {
    const makePost = (postId: string, comments = 0) => ({
      postId,
      content: 'test',
      contentType: 'post' as const,
      timestamp: new Date(),
      likes: 0,
      comments,
      liked: false,
      authorPeerId: 'peer-abc',
      visibility: 'contacts' as const,
      lamportClock: 0,
    });

    it('should expand comments and load thread from backend', () => {
      vi.mocked(commentsService.getComments).mockResolvedValue([]);

      useWallStore.getState().toggleComments('post-1');

      expect(useWallStore.getState().expandedComments.has('post-1')).toBe(true);
      expect(commentsService.getComments).toHaveBeenCalledWith('post-1');
    });

    it('should add comments and update counts', async () => {
      useWallStore.setState({ posts: [makePost('post-1')] });
      const comment = {
        id: 1,
        commentId: 'comment-1',
        postId: 'post-1',
        authorPeerId: 'peer-abc',
        authorName: 'Alice',
        content: 'Great post',
        createdAt: 1700000000,
        deletedAt: null,
      };
      vi.mocked(commentsService.addComment).mockResolvedValue(comment);

      await useWallStore.getState().addComment('post-1', 'Great post');

      expect(commentsService.addComment).toHaveBeenCalledWith('post-1', 'Great post');
      expect(feedService.syncWallSocialEventsToRelay).toHaveBeenCalledTimes(1);
      expect(useWallStore.getState().commentsByPost['post-1']).toEqual([comment]);
      expect(useWallStore.getState().posts[0].comments).toBe(1);
    });

    it('should delete comments and update counts', async () => {
      useWallStore.setState({
        posts: [makePost('post-1', 2)],
        commentsByPost: {
          'post-1': [
            {
              id: 1,
              commentId: 'comment-1',
              postId: 'post-1',
              authorPeerId: 'peer-abc',
              authorName: 'Alice',
              content: 'First',
              createdAt: 1700000000,
              deletedAt: null,
            },
            {
              id: 2,
              commentId: 'comment-2',
              postId: 'post-1',
              authorPeerId: 'peer-def',
              authorName: 'Bob',
              content: 'Second',
              createdAt: 1700000100,
              deletedAt: null,
            },
          ],
        },
      });
      vi.mocked(commentsService.deleteComment).mockResolvedValue(true);

      await useWallStore.getState().deleteComment('post-1', 'comment-1');

      expect(commentsService.deleteComment).toHaveBeenCalledWith('comment-1');
      expect(useWallStore.getState().commentsByPost['post-1']).toHaveLength(1);
      expect(useWallStore.getState().posts[0].comments).toBe(1);
    });
  });

  describe('setEditingPost', () => {
    it('should set the editing post ID', () => {
      useWallStore.getState().setEditingPost('post-1');
      expect(useWallStore.getState().editingPostId).toBe('post-1');
    });

    it('should clear the editing post ID', () => {
      useWallStore.getState().setEditingPost('post-1');
      useWallStore.getState().setEditingPost(null);
      expect(useWallStore.getState().editingPostId).toBeNull();
    });
  });

  describe('shareToWall', () => {
    it('should create a shared post with sharedFrom data', async () => {
      vi.mocked(postsService.createPost).mockResolvedValue({
        postId: 'shared-post-1',
        createdAt: 1700000200,
      });

      const sharedFrom = {
        authorName: 'Alice',
        authorPeerId: 'peer-alice',
        avatarGradient: 'linear-gradient(#f00, #00f)',
        originalContent: 'Original post content',
        originalPostId: 'original-post-1',
      };

      await useWallStore.getState().shareToWall('My comment', sharedFrom);

      const state = useWallStore.getState();
      expect(state.posts).toHaveLength(1);
      expect(state.posts[0].sharedFrom).toEqual(sharedFrom);
      expect(state.posts[0].content).toBe('My comment');

      // Verify the backend was called with the correct format
      expect(postsService.createPost).toHaveBeenCalledWith(
        'shared',
        expect.stringContaining('[Shared from Alice]'),
        'contacts',
      );
    });

    it('should handle empty comment for shared post', async () => {
      vi.mocked(postsService.createPost).mockResolvedValue({
        postId: 'shared-post-2',
        createdAt: 1700000200,
      });

      const sharedFrom = {
        authorName: 'Bob',
        authorPeerId: 'peer-bob',
        avatarGradient: 'linear-gradient(#0f0, #00f)',
        originalContent: 'Some content',
        originalPostId: 'original-post-2',
      };

      await useWallStore.getState().shareToWall('', sharedFrom);

      expect(postsService.createPost).toHaveBeenCalledWith(
        'shared',
        '[Shared from Bob]',
        'contacts',
      );
    });
  });
});
