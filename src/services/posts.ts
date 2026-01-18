import { invoke } from '@tauri-apps/api/core';
import type { Post, PostMedia, PostVisibility, CreatePostResult } from '../types';

/** Posts service - wraps Tauri commands for wall/blog functionality */
export const postsService = {
  /** Create a new post */
  async createPost(
    contentType: string,
    contentText?: string,
    visibility?: PostVisibility,
  ): Promise<CreatePostResult> {
    return invoke<CreatePostResult>('create_post', {
      contentType,
      contentText,
      visibility,
    });
  },

  /** Update a post's content */
  async updatePost(postId: string, contentText?: string): Promise<void> {
    return invoke<void>('update_post', { postId, contentText });
  },

  /** Delete a post (soft delete) */
  async deletePost(postId: string): Promise<void> {
    return invoke<void>('delete_post', { postId });
  },

  /** Get a single post by ID */
  async getPost(postId: string): Promise<Post | null> {
    return invoke<Post | null>('get_post', { postId });
  },

  /** Get the local user's posts (their wall) */
  async getMyPosts(limit?: number, beforeTimestamp?: number): Promise<Post[]> {
    return invoke<Post[]>('get_my_posts', { limit, beforeTimestamp });
  },

  /** Get posts by a specific author */
  async getPostsByAuthor(
    authorPeerId: string,
    limit?: number,
    beforeTimestamp?: number,
  ): Promise<Post[]> {
    return invoke<Post[]>('get_posts_by_author', {
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
    return invoke<void>('add_post_media', {
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
    });
  },

  /** Get media for a post */
  async getPostMedia(postId: string): Promise<PostMedia[]> {
    return invoke<PostMedia[]>('get_post_media', { postId });
  },
};
