import { create } from 'zustand';
import { postsService } from '../services/posts';
import type { Post, PostMedia } from '../types';

/** Extended post with UI-specific data */
export interface WallPost {
  postId: string;
  content: string;
  timestamp: Date;
  likes: number;
  comments: number;
  liked: boolean;
  media?: { type: 'image' | 'video'; url: string; name?: string }[];
  // Backend data
  authorPeerId: string;
  visibility: string;
  lamportClock: number;
}

interface WallState {
  posts: WallPost[];
  isLoading: boolean;
  error: string | null;

  // Actions
  loadPosts: () => Promise<void>;
  createPost: (
    content: string,
    media?: { type: 'image' | 'video'; url: string; name?: string }[],
  ) => Promise<void>;
  deletePost: (postId: string) => Promise<void>;
  likePost: (postId: string) => void; // Local-only for now (likes not in backend schema)
}

/** Convert backend Post to WallPost */
function toWallPost(post: Post, media?: PostMedia[]): WallPost {
  return {
    postId: post.postId,
    content: post.contentText || '',
    timestamp: new Date(post.createdAt),
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
    media?: { type: 'image' | 'video'; url: string; name?: string }[],
  ) => {
    try {
      const result = await postsService.createPost('text', content, 'contacts');

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
        timestamp: new Date(result.createdAt),
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
