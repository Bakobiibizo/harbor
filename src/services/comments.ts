import { publishingPolicy } from './publishingPolicy';
import { invokeCommand } from './command';

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
    publishingPolicy.assertAllowed();
    return invokeCommand('add_comment', { postId, content });
  },

  /** Get comments for a post */
  async getComments(postId: string): Promise<Comment[]> {
    return invokeCommand('get_comments', { postId });
  },

  /** Delete a comment */
  async deleteComment(commentId: string): Promise<boolean> {
    return invokeCommand('delete_comment', { commentId });
  },

  /** Get comment counts for multiple posts */
  async getCommentCounts(postIds: string[]): Promise<CommentCount[]> {
    return invokeCommand('get_comment_counts', { postIds });
  },
};
