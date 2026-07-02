import { invoke } from '@tauri-apps/api/core';

/** A comment on a post */
export interface Comment {
  id: number;
  commentId: string;
  postId: string;
  authorPeerId: string;
  authorName: string;
  content: string;
  createdAt: number;
  deletedAt: number | null;
}

/** Comment count for a post */
export interface CommentCount {
  postId: string;
  count: number;
}

/** Comments service - wraps Tauri commands for comment functionality */
export const commentsService = {
  /** Add a comment to a post */
  async addComment(postId: string, content: string): Promise<Comment> {
    return invoke<Comment>('add_comment', { postId, content });
  },

  /** Get comments for a post */
  async getComments(postId: string): Promise<Comment[]> {
    return invoke<Comment[]>('get_comments', { postId });
  },

  /** Delete a comment */
  async deleteComment(commentId: string): Promise<boolean> {
    return invoke<boolean>('delete_comment', { commentId });
  },

  /** Get comment counts for multiple posts */
  async getCommentCounts(postIds: string[]): Promise<CommentCount[]> {
    return invoke<CommentCount[]>('get_comment_counts', { postIds });
  },
};
