import { create } from 'zustand';
import { postsService } from '../services/posts';
import { mediaService } from '../services/media';
import { feedService } from '../services/feed';
import { commentsService, type Comment } from '../services/comments';
import { likesService } from '../services/likes';
import { createLogger } from '../utils/logger';
import { useSettingsStore } from './settings';
import type { CreatePostMediaInput, Post, PostMedia, PostVisibility } from '../types';

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
  media?: { type: 'image' | 'video' | 'audio'; url: string; name?: string }[];
  // Repost data
  sharedFrom?: SharedFrom;
  // Backend data
  authorPeerId: string;
  visibility: PostVisibility;
  lamportClock: number;
}

interface WallState {
  posts: WallPost[];
  isLoading: boolean;
  isSyncingRelay: boolean;
  lastSyncAt: number | null;
  syncError: string | null;
  syncStatus: 'idle' | 'in_progress' | 'success' | 'partial_failure';
  error: string | null;
  editingPostId: string | null;
  commentsByPost: Record<string, Comment[]>;
  expandedComments: Set<string>;
  loadingComments: Set<string>;

  // Actions
  loadPosts: () => Promise<void>;
  createPost: (
    content: string,
    contentType?: WallContentType,
    media?: { type: 'image' | 'video' | 'audio'; url: string; file?: File; name?: string }[],
    visibility?: PostVisibility,
  ) => Promise<void>;
  shareToWall: (comment: string, sharedFrom: SharedFrom) => Promise<void>;
  updatePost: (postId: string, content: string) => Promise<void>;
  deletePost: (postId: string) => Promise<void>;
  likePost: (postId: string) => Promise<void>;
  loadComments: (postId: string) => Promise<void>;
  toggleComments: (postId: string) => void;
  addComment: (postId: string, content: string) => Promise<void>;
  deleteComment: (postId: string, commentId: string) => Promise<void>;
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
        type: (m.mediaType === 'video' ? 'video' : m.mediaType === 'audio' ? 'audio' : 'image') as
          | 'image'
          | 'video'
          | 'audio',
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

export const useWallStore = create<WallState>((set, get) => ({
  posts: [],
  isLoading: false,
  isSyncingRelay: false,
  lastSyncAt: null,
  syncError: null,
  syncStatus: 'idle',
  error: null,
  editingPostId: null,
  commentsByPost: {},
  expandedComments: new Set<string>(),
  loadingComments: new Set<string>(),

  loadPosts: async () => {
    set({ isLoading: true, error: null });
    try {
      const posts = await postsService.getMyPosts(50);

      // Load media for each post and resolve hashes to URLs
      let wallPosts = await Promise.all(
        posts.map(async (post) => {
          try {
            const media = await postsService.getPostMedia(post.postId);
            return await toWallPost(post, media);
          } catch {
            return await toWallPost(post);
          }
        }),
      );

      const postIds = wallPosts.map((post) => post.postId);
      if (postIds.length > 0) {
        const authorPeerId = wallPosts[0]?.authorPeerId;
        if (authorPeerId) {
          feedService.fetchWallSocialEvents(authorPeerId, postIds).catch((err) =>
            log.warn('Failed to fetch wall social events from relay', err),
          );
        }
        try {
          const [likeSummaries, commentCounts] = await Promise.all([
            likesService.getPostsLikesBatch(postIds),
            commentsService.getCommentCounts(postIds),
          ]);
          const likesByPost = new Map(likeSummaries.map((summary) => [summary.postId, summary]));
          const commentsByPost = new Map(commentCounts.map((count) => [count.postId, count.count]));
          wallPosts = wallPosts.map((post) => {
            const likes = likesByPost.get(post.postId);
            return {
              ...post,
              likes: likes?.totalLikes ?? 0,
              liked: likes?.userHasLiked ?? false,
              comments: commentsByPost.get(post.postId) ?? 0,
            };
          });
        } catch (err) {
          log.warn('Failed to load wall social counts', err);
        }
      }

      set({ posts: wallPosts, isLoading: false });
    } catch (err) {
      log.error('Failed to load posts', err);
      set({ error: String(err), isLoading: false });
    }
  },

  createPost: async (
    content: string,
    contentType: WallContentType = 'post',
    media?: { type: 'image' | 'video' | 'audio'; url: string; file?: File; name?: string }[],
    visibility?: PostVisibility,
  ) => {
    try {
      // Map WallContentType to backend content_type string
      const backendContentType = contentType === 'post' ? 'text' : contentType;
      const selectedVisibility = visibility ?? useSettingsStore.getState().defaultVisibility;

      const signedMediaInputs: CreatePostMediaInput[] = [];
      if (media && media.length > 0) {
        for (let i = 0; i < media.length; i++) {
          const m = media[i];
          const fallbackMimeType =
            m.type === 'image' ? 'image/jpeg' : m.type === 'video' ? 'video/mp4' : 'audio/mpeg';
          let fileSize = 0;
          let mediaHash: string;
          let mimeType = m.file?.type || fallbackMimeType;

          if (m.file) {
            const bytes = await readFileAsBytes(m.file);
            fileSize = bytes.length;
            mediaHash = await mediaService.storeMediaBytes(bytes, mimeType);
          } else {
            const response = await fetch(m.url);
            const blob = await response.blob();
            const bytes = new Uint8Array(await blob.arrayBuffer());
            fileSize = bytes.length;
            mimeType = blob.type || mimeType;
            mediaHash = await mediaService.storeMediaBytes(bytes, mimeType);
          }

          signedMediaInputs.push({
            mediaHash,
            mediaType: m.type,
            mimeType,
            fileName: m.name || `media-${i}`,
            fileSize,
            sortOrder: i,
          });
        }
      }

      const result = await postsService.createPost(
        backendContentType,
        content,
        selectedVisibility,
        signedMediaInputs.length > 0 ? signedMediaInputs : undefined,
      );

      // Add to local state immediately for instant UI feedback.
      const previewMedia = media?.map((m) => ({
        type: m.type,
        url: m.url,
        name: m.name,
      }));

      const newPost: WallPost = {
        postId: result.postId,
        content,
        contentType,
        timestamp: new Date(result.createdAt * 1000),
        likes: 0,
        comments: 0,
        liked: false,
        media: previewMedia && previewMedia.length > 0 ? previewMedia : undefined,
        authorPeerId: '', // Will be set properly on reload
        visibility: selectedVisibility,
        lamportClock: 0,
      };

      set((state) => ({
        posts: [newPost, ...state.posts],
      }));

      // Best-effort sync to relay -- post is already saved locally
      set({ isSyncingRelay: true, syncStatus: 'in_progress', syncError: null });
      feedService
        .syncWallToRelay()
        .then(() => {
          set({
            isSyncingRelay: false,
            lastSyncAt: Math.floor(Date.now() / 1000),
            syncStatus: 'success',
            syncError: null,
          });
        })
        .catch((err) => {
          log.warn('Failed to sync post to relay (saved locally)', err);
          set({
            isSyncingRelay: false,
            lastSyncAt: Math.floor(Date.now() / 1000),
            syncStatus: 'partial_failure',
            syncError: String(err),
          });
        });
    } catch (err) {
      log.error('Failed to create post', err);
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

      const selectedVisibility = useSettingsStore.getState().defaultVisibility;
      const result = await postsService.createPost('shared', contentForBackend, selectedVisibility);

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
        visibility: selectedVisibility,
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

  likePost: async (postId: string) => {
    const currentPost = get().posts.find((post) => post.postId === postId);
    if (!currentPost) return;

    try {
      const summary = currentPost.liked
        ? await likesService.unlikePost(postId)
        : await likesService.likePost(postId);
      feedService.syncWallSocialEventsToRelay().catch((err) =>
        log.warn('Failed to sync wall reaction event to relay', err),
      );

      set((state) => ({
        posts: state.posts.map((post) =>
          post.postId === postId
            ? {
                ...post,
                liked: summary.userHasLiked,
                likes: summary.totalLikes,
              }
            : post,
        ),
      }));
    } catch (err) {
      log.error('Failed to toggle post reaction', err);
      throw err;
    }
  },

  loadComments: async (postId: string) => {
    if (get().loadingComments.has(postId)) return;

    set((state) => {
      const loadingComments = new Set(state.loadingComments);
      loadingComments.add(postId);
      return { loadingComments };
    });

    try {
      const comments = await commentsService.getComments(postId);
      set((state) => {
        const loadingComments = new Set(state.loadingComments);
        loadingComments.delete(postId);
        return {
          commentsByPost: { ...state.commentsByPost, [postId]: comments },
          posts: state.posts.map((post) =>
            post.postId === postId ? { ...post, comments: comments.length } : post,
          ),
          loadingComments,
        };
      });
    } catch (err) {
      log.error('Failed to load post comments', err);
      set((state) => {
        const loadingComments = new Set(state.loadingComments);
        loadingComments.delete(postId);
        return { loadingComments };
      });
      throw err;
    }
  },

  toggleComments: (postId: string) => {
    set((state) => {
      const expandedComments = new Set(state.expandedComments);
      if (expandedComments.has(postId)) {
        expandedComments.delete(postId);
      } else {
        expandedComments.add(postId);
        if (!state.commentsByPost[postId]) {
          get()
            .loadComments(postId)
            .catch((err) => log.error('Failed to open comments', err));
        }
      }
      return { expandedComments };
    });
  },

  addComment: async (postId: string, content: string) => {
    const comment = await commentsService.addComment(postId, content);
    feedService.syncWallSocialEventsToRelay().catch((err) =>
      log.warn('Failed to sync wall comment event to relay', err),
    );
    set((state) => {
      const existing = state.commentsByPost[postId] || [];
      return {
        commentsByPost: { ...state.commentsByPost, [postId]: [...existing, comment] },
        posts: state.posts.map((post) =>
          post.postId === postId ? { ...post, comments: post.comments + 1 } : post,
        ),
      };
    });
  },

  deleteComment: async (postId: string, commentId: string) => {
    await commentsService.deleteComment(commentId);
    set((state) => {
      const existing = state.commentsByPost[postId] || [];
      const nextComments = existing.filter((comment) => comment.commentId !== commentId);
      return {
        commentsByPost: { ...state.commentsByPost, [postId]: nextComments },
        posts: state.posts.map((post) =>
          post.postId === postId ? { ...post, comments: nextComments.length } : post,
        ),
      };
    });
  },
}));
