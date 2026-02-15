import { create } from 'zustand';
import { postsService } from '../services/posts';
import type { Post, PostMedia } from '../types';

const log = createLogger('WallStore');

/** Content types for wall posts */
export type WallContentType = 'post' | 'thought' | 'image' | 'video' | 'audio';

/** Repost attribution data */
export interface SharedFrom {
  authorName: string;
  authorPeerId: string;
  avatarGradient: string;
  originalContent: string;
  originalPostId: string;
}

/** Extended post with UI-specific data */
export interface WallPost {
  postId: string;
  content: string;
  contentType: WallContentType;
  timestamp: Date;
  likes: number;
  comments: number;
  liked: boolean;
  media?: { type: 'image' | 'video'; url: string; name?: string }[];
  // Repost data
  sharedFrom?: SharedFrom;
  // Backend data
  authorPeerId: string;
  visibility: string;
  lamportClock: number;
}

interface WallState {
  posts: WallPost[];
  isLoading: boolean;
  error: string | null;
  editingPostId: string | null;

  // Actions
  loadPosts: () => Promise<void>;
  createPost: (
    content: string,
    contentType?: WallContentType,
    media?: { type: 'image' | 'video'; url: string; file?: File; name?: string }[],
  ) => Promise<void>;
  shareToWall: (comment: string, sharedFrom: SharedFrom) => Promise<void>;
  updatePost: (postId: string, content: string) => Promise<void>;
  deletePost: (postId: string) => Promise<void>;
  likePost: (postId: string) => void; // Local-only for now (likes not in backend schema)
  setEditingPost: (postId: string | null) => void;
}

/** Resolve a media hash to a displayable URL using the media storage service */
async function resolveMediaUrl(mediaHash: string): Promise<string> {
  try {
    // If it looks like a blob URL or data URL, return it as-is (legacy/preview)
    if (mediaHash.startsWith('blob:') || mediaHash.startsWith('data:')) {
      return mediaHash;
    }
    return await mediaService.getMediaUrl(mediaHash);
  } catch {
    // If the media file is not found locally, return a placeholder
    log.warn('Could not resolve media URL for hash:', mediaHash);
    return '';
  }
}

/** Map backend content_type string to WallContentType */
function parseContentType(backendType: string): WallContentType {
  switch (backendType) {
    case 'thought':
      return 'thought';
    case 'image':
      return 'image';
    case 'video':
      return 'video';
    case 'audio':
      return 'audio';
    case 'post':
    case 'text':
    default:
      return 'post';
  }
}

/** Convert backend Post to WallPost, resolving media hashes to URLs */
async function toWallPost(post: Post, media?: PostMedia[]): Promise<WallPost> {
  let resolvedMedia: WallPost['media'] = undefined;

  if (media && media.length > 0) {
    resolvedMedia = await Promise.all(
      media.map(async (m) => ({
        type: (m.mediaType === 'video' ? 'video' : 'image') as 'image' | 'video',
        url: await resolveMediaUrl(m.mediaHash),
        name: m.fileName,
      })),
    );
    // Filter out any media with empty URLs (not found)
    resolvedMedia = resolvedMedia.filter((m) => m.url !== '');
    if (resolvedMedia.length === 0) {
      resolvedMedia = undefined;
    }
  }

  return {
    postId: post.postId,
    content: post.contentText || '',
    contentType: parseContentType(post.contentType),
    timestamp: new Date(post.createdAt * 1000),
    likes: 0, // Backend doesn't track likes yet
    comments: 0, // Backend doesn't track comments yet
    liked: false,
    media: media?.map((m) => ({
      type: m.mediaType === 'video' ? 'video' : 'image',
      url: m.mediaHash, // In real impl, this would be resolved to a URL
      name: m.fileName,
    })),
    authorPeerId: post.authorPeerId,
    visibility: post.visibility,
    lamportClock: post.lamportClock,
  };
}

export const useWallStore = create<WallState>((set) => ({
  posts: [],
  isLoading: false,
  error: null,
  editingPostId: null,

  loadPosts: async () => {
    set({ isLoading: true, error: null });
    try {
      const posts = await postsService.getMyPosts(50);

      // Load media for each post
      const wallPosts = await Promise.all(
        posts.map(async (post) => {
          try {
            const media = await postsService.getPostMedia(post.postId);
            return toWallPost(post, media);
          } catch {
            return toWallPost(post);
          }
        }),
      );

      set({ posts: wallPosts, isLoading: false });
    } catch (err) {
      console.error('Failed to load posts:', err);
      set({ error: String(err), isLoading: false });
    }
  },

  createPost: async (
    content: string,
    contentType: WallContentType = 'post',
    media?: { type: 'image' | 'video'; url: string; file?: File; name?: string }[],
  ) => {
    try {
      // Map WallContentType to backend content_type string
      const backendContentType = contentType === 'post' ? 'text' : contentType;
      const result = await postsService.createPost(backendContentType, content, 'contacts');

      // Add media if provided
      // Note: For now, media handling is simplified - in production,
      // media would be stored in content-addressed storage and hashed
      if (media && media.length > 0) {
        for (let i = 0; i < media.length; i++) {
          const m = media[i];
          await postsService.addPostMedia(
            result.postId,
            m.url, // Using URL as hash for now (would be content hash in production)
            m.type,
            m.type === 'image' ? 'image/jpeg' : 'video/mp4',
            m.name || `media-${i}`,
            0, // File size unknown from blob URL
            undefined,
            undefined,
            undefined,
            i,
          );
        }
      }

      // Add to local state immediately for instant UI feedback
      const newPost: WallPost = {
        postId: result.postId,
        content,
        contentType,
        timestamp: new Date(result.createdAt * 1000),
        likes: 0,
        comments: 0,
        liked: false,
        media,
        authorPeerId: '', // Will be set properly on reload
        visibility: 'contacts',
        lamportClock: 0,
      };

      set((state) => ({
        posts: [newPost, ...state.posts],
      }));
    } catch (err) {
      console.error('Failed to create post:', err);
      throw err;
    }
  },

  shareToWall: async (comment: string, sharedFrom: SharedFrom) => {
    try {
      // Build the content text: user comment + marker for shared content
      // The shared metadata is stored in the sharedFrom field on WallPost
      const contentForBackend = comment.trim()
        ? `${comment.trim()}\n\n[Shared from ${sharedFrom.authorName}]`
        : `[Shared from ${sharedFrom.authorName}]`;

      const result = await postsService.createPost('shared', contentForBackend, 'contacts');

      const newPost: WallPost = {
        postId: result.postId,
        content: comment.trim(),
        contentType: 'post',
        timestamp: new Date(result.createdAt * 1000),
        likes: 0,
        comments: 0,
        liked: false,
        sharedFrom,
        authorPeerId: '',
        visibility: 'contacts',
        lamportClock: 0,
      };

      set((state) => ({
        posts: [newPost, ...state.posts],
      }));
    } catch (err) {
      console.error('Failed to share post:', err);
      throw err;
    }
  },

  updatePost: async (postId: string, content: string) => {
    try {
      await postsService.updatePost(postId, content);

      // Update local state
      set((state) => ({
        posts: state.posts.map((post) => (post.postId === postId ? { ...post, content } : post)),
        editingPostId: null,
      }));
    } catch (err) {
      console.error('Failed to update post:', err);
      throw err;
    }
  },

  deletePost: async (postId: string) => {
    try {
      await postsService.deletePost(postId);

      // Remove from local state
      set((state) => ({
        posts: state.posts.filter((p) => p.postId !== postId),
      }));
    } catch (err) {
      console.error('Failed to delete post:', err);
      throw err;
    }
  },

  setEditingPost: (postId: string | null) => {
    set({ editingPostId: postId });
  },

  likePost: (postId: string) => {
    // Likes are local-only for now (not in backend schema)
    set((state) => ({
      posts: state.posts.map((post) =>
        post.postId === postId
          ? {
            ...post,
            liked: !post.liked,
            likes: post.liked ? post.likes - 1 : post.likes + 1,
          }
          : post,
      ),
    }));
  },
}));
