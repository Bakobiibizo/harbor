import { create } from 'zustand';
import toast from 'react-hot-toast';
import { postsService } from '../services/posts';
import { mediaService } from '../services/media';
import { feedService } from '../services/feed';
import { createLogger } from '../utils/logger';
import type { Post, PostMedia } from '../types';

const log = createLogger('WallStore');

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
  editingPostId: string | null;

  // Actions
  loadPosts: () => Promise<void>;
  createPost: (
    content: string,
    media?: { type: 'image' | 'video'; url: string; file?: File; name?: string }[],
  ) => Promise<void>;
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
    timestamp: new Date(post.createdAt * 1000),
    likes: 0, // Backend doesn't track likes yet
    comments: 0, // Backend doesn't track comments yet
    liked: false,
    media: resolvedMedia,
    authorPeerId: post.authorPeerId,
    visibility: post.visibility,
    lamportClock: post.lamportClock,
  };
}

/** Read a File object into a Uint8Array */
async function readFileAsBytes(file: File): Promise<Uint8Array> {
  const buffer = await file.arrayBuffer();
  return new Uint8Array(buffer);
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

      // Load media for each post and resolve hashes to URLs
      const wallPosts = await Promise.all(
        posts.map(async (post) => {
          try {
            const media = await postsService.getPostMedia(post.postId);
            return await toWallPost(post, media);
          } catch {
            return await toWallPost(post);
          }
        }),
      );

      set({ posts: wallPosts, isLoading: false });
    } catch (err) {
      log.error('Failed to load posts', err);
      set({ error: String(err), isLoading: false });
    }
  },

  createPost: async (
    content: string,
    media?: { type: 'image' | 'video'; url: string; file?: File; name?: string }[],
  ) => {
    try {
      const result = await postsService.createPost('text', content, 'contacts');

      // Store media files and record metadata
      const storedMedia: { type: 'image' | 'video'; url: string; name?: string }[] = [];

      if (media && media.length > 0) {
        for (let i = 0; i < media.length; i++) {
          const m = media[i];
          let mediaHash: string;
          let fileSize = 0;
          const mimeType = m.type === 'image' ? 'image/jpeg' : 'video/mp4';

          if (m.file) {
            // Store the actual file data via content-addressed storage
            const bytes = await readFileAsBytes(m.file);
            fileSize = bytes.length;
            mediaHash = await mediaService.storeMediaBytes(bytes, m.file.type || mimeType);
          } else {
            // Fallback: fetch blob URL and store the bytes
            try {
              const response = await fetch(m.url);
              const blob = await response.blob();
              const bytes = new Uint8Array(await blob.arrayBuffer());
              fileSize = bytes.length;
              const detectedMime = blob.type || mimeType;
              mediaHash = await mediaService.storeMediaBytes(bytes, detectedMime);
            } catch (fetchErr) {
              log.warn('Could not fetch blob URL, using URL as hash fallback', fetchErr);
              mediaHash = m.url;
            }
          }

          // Record the media metadata in the database
          await postsService.addPostMedia(
            result.postId,
            mediaHash,
            m.type,
            m.file?.type || mimeType,
            m.name || `media-${i}`,
            fileSize,
            undefined,
            undefined,
            undefined,
            i,
          );

          // Resolve the hash to a displayable URL for the UI
          const displayUrl = await resolveMediaUrl(mediaHash);
          storedMedia.push({
            type: m.type,
            url: displayUrl || m.url, // Fall back to blob URL for immediate display
            name: m.name,
          });
        }
      }

      // Add to local state immediately for instant UI feedback
      const newPost: WallPost = {
        postId: result.postId,
        content,
        timestamp: new Date(result.createdAt * 1000),
        likes: 0,
        comments: 0,
        liked: false,
        media: storedMedia.length > 0 ? storedMedia : undefined,
        authorPeerId: '', // Will be set properly on reload
        visibility: 'contacts',
        lamportClock: 0,
      };

      set((state) => ({
        posts: [newPost, ...state.posts],
      }));

      // Best-effort sync to relay -- post is already saved locally
      feedService
        .syncWallToRelay()
        .then(() => {
          toast.success('Post synced to relay');
        })
        .catch((err) => {
          log.warn('Failed to sync post to relay (saved locally)', err);
        });
    } catch (err) {
      log.error('Failed to create post', err);
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
      log.error('Failed to update post', err);
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
      log.error('Failed to delete post', err);
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
