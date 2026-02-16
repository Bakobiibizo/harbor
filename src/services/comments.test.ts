import { describe, it, expect, vi, beforeEach } from 'vitest';
import { commentsService } from './comments';
import { invoke } from '@tauri-apps/api/core';

describe('commentsService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('addComment', () => {
    it('should invoke add_comment with correct args', async () => {
      const mockComment = {
        id: 1,
        commentId: 'comment-1',
        postId: 'post-1',
        authorPeerId: 'peer-me',
        authorName: 'Me',
        content: 'Great post!',
        createdAt: 1700000200,
        deletedAt: null,
      };
      vi.mocked(invoke).mockResolvedValue(mockComment);

      const result = await commentsService.addComment('post-1', 'Great post!');

      expect(invoke).toHaveBeenCalledWith('add_comment', {
        postId: 'post-1',
        content: 'Great post!',
      });
      expect(result).toEqual(mockComment);
    });
  });

  describe('getComments', () => {
    it('should invoke get_comments with postId', async () => {
      vi.mocked(invoke).mockResolvedValue([]);

      await commentsService.getComments('post-1');

      expect(invoke).toHaveBeenCalledWith('get_comments', { postId: 'post-1' });
    });
  });

  describe('deleteComment', () => {
    it('should invoke delete_comment with commentId', async () => {
      vi.mocked(invoke).mockResolvedValue(true);

      const result = await commentsService.deleteComment('comment-1');

      expect(invoke).toHaveBeenCalledWith('delete_comment', { commentId: 'comment-1' });
      expect(result).toBe(true);
    });
  });

  describe('getCommentCounts', () => {
    it('should invoke get_comment_counts with postIds', async () => {
      const mockCounts = [
        { postId: 'post-1', count: 5 },
        { postId: 'post-2', count: 0 },
      ];
      vi.mocked(invoke).mockResolvedValue(mockCounts);

      const result = await commentsService.getCommentCounts(['post-1', 'post-2']);

      expect(invoke).toHaveBeenCalledWith('get_comment_counts', {
        postIds: ['post-1', 'post-2'],
      });
      expect(result).toEqual(mockCounts);
    });
  });
});
