import type {
  Post,
  PostMedia,
  PostVisibility,
  CreatePostResult,
  CreatePostMediaInput,
  PostMutationResult,
} from '../types';
import { publishingPolicy } from './publishingPolicy';
import { invokeCommand } from './command';

/** Posts service - wraps Tauri commands for wall/blog functionality */
export const postsService = {
  /** Create a new post */
  async createPost(
    contentType: string,
    contentText?: string,
    visibility?: PostVisibility,
    media?: CreatePostMediaInput[],
  ): Promise<CreatePostResult> {
    publishingPolicy.assertAllowed();
    return invokeCommand('create_post', {
      contentType,
      contentText,
      visibility,
      media,
    });
  },

  /** Update a post's content */
  async updatePost(postId: string, contentText?: string): Promise<PostMutationResult> {
    return invokeCommand('update_post', { postId, contentText });
  },

  /** Delete a post (soft delete) */
  async deletePost(postId: string): Promise<PostMutationResult> {
    return invokeCommand('delete_post', { postId });
  },

  /** Get a single post by ID */
  async getPost(postId: string): Promise<Post | null> {
    return invokeCommand('get_post', { postId });
  },

  /** Get the local user's posts (their wall) */
  async getMyPosts(limit?: number, beforeTimestamp?: number): Promise<Post[]> {
    return invokeCommand('get_my_posts', { limit, beforeTimestamp });
  },

  /** Get posts by a specific author */
  async getPostsByAuthor(
    authorPeerId: string,
    limit?: number,
    beforeTimestamp?: number,
  ): Promise<Post[]> {
    return invokeCommand('get_posts_by_author', {
      authorPeerId,
      limit,
      beforeTimestamp,
    });
  },

  /** Add media to a post */
  async addPostMedia(
    postId: string,
    mediaHash: string,
    mediaType: string,
    mimeType: string,
    fileName: string,
    fileSize: number,
    width?: number,
    height?: number,
    durationSeconds?: number,
    sortOrder?: number,
  ): Promise<void> {
    return invokeCommand('add_post_media', {
      params: {
        postId,
        mediaHash,
        mediaType,
        mimeType,
        fileName,
        fileSize,
        width,
        height,
        durationSeconds,
        sortOrder,
      },
    });
  },

  /** Get media for a post */
  async getPostMedia(postId: string): Promise<PostMedia[]> {
    return invokeCommand('get_post_media', { postId });
  },
};
