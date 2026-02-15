import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useWallStore } from './wall';
import { postsService } from '../services/posts';

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
    });
    vi.clearAllMocks();
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

    it('should handle media fetch errors gracefully per post', async () => {
      vi.mocked(postsService.getMyPosts).mockResolvedValue([mockBackendPost]);
      vi.mocked(postsService.getPostMedia).mockRejectedValue(new Error('Media error'));

      await useWallStore.getState().loadPosts();

      // Should still load the post, just without media
      expect(useWallStore.getState().posts).toHaveLength(1);
      expect(useWallStore.getState().posts[0].media).toBeUndefined();
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

      await expect(useWallStore.getState().createPost('content')).rejects.toThrow(
        'Create failed',
      );
    });

    it('should add media when provided', async () => {
      vi.mocked(postsService.createPost).mockResolvedValue({
        postId: 'media-post',
        createdAt: 1700000100,
      });
      vi.mocked(postsService.addPostMedia).mockResolvedValue(undefined);

      const media = [{ type: 'image' as const, url: 'blob:test', name: 'photo.jpg' }];
      await useWallStore.getState().createPost('Post with image', 'post', media);

      expect(postsService.addPostMedia).toHaveBeenCalledTimes(1);
      const state = useWallStore.getState();
      expect(state.posts[0].media).toEqual(media);
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

      await expect(
        useWallStore.getState().updatePost('post-1', 'new content'),
      ).rejects.toThrow('Update failed');
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

    it('should toggle like status on', () => {
      useWallStore.setState({ posts: [makePost('post-1', false, 0)] });

      useWallStore.getState().likePost('post-1');

      const post = useWallStore.getState().posts[0];
      expect(post.liked).toBe(true);
      expect(post.likes).toBe(1);
    });

    it('should toggle like status off', () => {
      useWallStore.setState({ posts: [makePost('post-1', true, 1)] });

      useWallStore.getState().likePost('post-1');

      const post = useWallStore.getState().posts[0];
      expect(post.liked).toBe(false);
      expect(post.likes).toBe(0);
    });

    it('should only affect the target post', () => {
      useWallStore.setState({
        posts: [makePost('post-1', false, 0), makePost('post-2', false, 0)],
      });

      useWallStore.getState().likePost('post-1');

      const posts = useWallStore.getState().posts;
      expect(posts[0].liked).toBe(true);
      expect(posts[1].liked).toBe(false);
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
