import { describe, it, expect, vi, beforeEach } from 'vitest';
import { postsService } from './posts';
import { invoke } from '@tauri-apps/api/core';

describe('postsService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('createPost', () => {
    it('should invoke create_post with correct arguments', async () => {
      const mockResult = { postId: 'post-1', createdAt: 1700000000 };
      vi.mocked(invoke).mockResolvedValue(mockResult);

      const result = await postsService.createPost('text', 'Hello world', 'contacts');

      expect(invoke).toHaveBeenCalledWith('create_post', {
        contentType: 'text',
        contentText: 'Hello world',
        visibility: 'contacts',
        media: undefined,
      });
      expect(result).toEqual(mockResult);
    });

    it('should invoke create_post with image and video media metadata', async () => {
      const media = [
        {
          mediaHash: 'a'.repeat(64),
          mediaType: 'image' as const,
          mimeType: 'image/png',
          fileName: 'photo.png',
          fileSize: 1024,
          sortOrder: 0,
        },
        {
          mediaHash: 'b'.repeat(64),
          mediaType: 'video' as const,
          mimeType: 'video/mp4',
          fileName: 'clip.mp4',
          fileSize: 2048,
          durationSeconds: 2,
          sortOrder: 1,
        },
      ];
      const mockResult = { postId: 'post-media', createdAt: 1700000000 };
      vi.mocked(invoke).mockResolvedValue(mockResult);

      const result = await postsService.createPost('video', 'caption', 'contacts', media);

      expect(invoke).toHaveBeenCalledWith('create_post', {
        contentType: 'video',
        contentText: 'caption',
        visibility: 'contacts',
        media,
      });
      expect(result).toEqual(mockResult);
    });
  });

  describe('updatePost', () => {
    it('should invoke update_post', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);

      await postsService.updatePost('post-1', 'Updated content');

      expect(invoke).toHaveBeenCalledWith('update_post', {
        postId: 'post-1',
        contentText: 'Updated content',
      });
    });
  });

  describe('deletePost', () => {
    it('should invoke delete_post', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);

      await postsService.deletePost('post-1');

      expect(invoke).toHaveBeenCalledWith('delete_post', { postId: 'post-1' });
    });
  });

  describe('getPost', () => {
    it('should invoke get_post and return result', async () => {
      const mockPost = {
        postId: 'post-1',
        authorPeerId: 'peer-me',
        contentType: 'text',
        contentText: 'Hello',
        visibility: 'contacts',
        lamportClock: 1,
        createdAt: 1700000000,
        updatedAt: 1700000000,
        deletedAt: null,
        isLocal: true,
      };
      vi.mocked(invoke).mockResolvedValue(mockPost);

      const result = await postsService.getPost('post-1');

      expect(invoke).toHaveBeenCalledWith('get_post', { postId: 'post-1' });
      expect(result).toEqual(mockPost);
    });

    it('should return null for non-existent post', async () => {
      vi.mocked(invoke).mockResolvedValue(null);

      const result = await postsService.getPost('nonexistent');

      expect(result).toBeNull();
    });
  });

  describe('getMyPosts', () => {
    it('should invoke get_my_posts with pagination', async () => {
      vi.mocked(invoke).mockResolvedValue([]);

      await postsService.getMyPosts(50, 1700000000);

      expect(invoke).toHaveBeenCalledWith('get_my_posts', {
        limit: 50,
        beforeTimestamp: 1700000000,
      });
    });
  });

  describe('getPostsByAuthor', () => {
    it('should invoke get_posts_by_author', async () => {
      vi.mocked(invoke).mockResolvedValue([]);

      await postsService.getPostsByAuthor('peer-alice', 25);

      expect(invoke).toHaveBeenCalledWith('get_posts_by_author', {
        authorPeerId: 'peer-alice',
        limit: 25,
        beforeTimestamp: undefined,
      });
    });
  });

  describe('addPostMedia', () => {
    it('should invoke add_post_media with all parameters', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);

      await postsService.addPostMedia(
        'post-1',
        'hash-abc',
        'image',
        'image/jpeg',
        'photo.jpg',
        1024000,
        1920,
        1080,
        undefined,
        0,
      );

      expect(invoke).toHaveBeenCalledWith('add_post_media', {
        params: {
          postId: 'post-1',
          mediaHash: 'hash-abc',
          mediaType: 'image',
          mimeType: 'image/jpeg',
          fileName: 'photo.jpg',
          fileSize: 1024000,
          width: 1920,
          height: 1080,
          durationSeconds: undefined,
          sortOrder: 0,
        },
      });
    });
  });

  describe('getPostMedia', () => {
    it('should invoke get_post_media', async () => {
      vi.mocked(invoke).mockResolvedValue([]);

      await postsService.getPostMedia('post-1');

      expect(invoke).toHaveBeenCalledWith('get_post_media', { postId: 'post-1' });
    });
  });
});
