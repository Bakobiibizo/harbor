import { create } from 'zustand';
import { postsService } from '../services/posts';
import { feedService } from '../services/feed';
import { commentsService, type Comment } from '../services/comments';
import { likesService } from '../services/likes';
import { createLogger } from '../utils/logger';
import { getErrorMessage } from '../utils/errors';
import { useSettingsStore } from './settings';
import type {
  CreatePostMediaInput,
  Post,
  PostMedia,
  PostRelayStatus,
  PostVisibility,
} from '../types';

const log = createLogger('WallStore');

/** Content types for wall posts */
export type WallContentType = 'post' | 'thought' | 'image' | 'video' | 'audio';
type DraftMedia = {
  type: 'image' | 'video' | 'audio';
  url: string;
  name?: string;
  mediaHash?: string;
  mimeType?: string;
  fileSize?: number;
};

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
  media?: {
    type: 'image' | 'video' | 'audio';
    url: string;
    name?: string;
    sourcePeerId?: string;
    mimeType?: string;
    totalBytes?: number;
  }[];
  // Repost data
  sharedFrom?: SharedFrom;
  // Backend data
  authorPeerId: string;
  visibility: PostVisibility;
  lamportClock: number;
  relayStatus: PostRelayStatus;
  deletionPending?: boolean;
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
    media?: DraftMedia[],
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
  setPostRelayStatus: (postId: string, status: PostRelayStatus) => void;
  reset: () => void;
}

let lifecycleGeneration = 0;

const initialState = {
  posts: [] as WallPost[],
  isLoading: false,
  isSyncingRelay: false,
  lastSyncAt: null as number | null,
  syncError: null as string | null,
  syncStatus: 'idle' as const,
  error: null as string | null,
  editingPostId: null as string | null,
  commentsByPost: {} as Record<string, Comment[]>,
  expandedComments: new Set<string>(),
  loadingComments: new Set<string>(),
};

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
    resolvedMedia = media.map((m) => ({
      type: (m.mediaType === 'video' ? 'video' : m.mediaType === 'audio' ? 'audio' : 'image') as
        'image' | 'video' | 'audio',
      url: m.mediaHash,
      name: m.fileName,
      sourcePeerId: post.authorPeerId,
      mimeType: m.mimeType,
      totalBytes: m.fileSize,
    }));
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
    relayStatus: post.relayStatus,
  };
}

export const useWallStore = create<WallState>((set, get) => ({
  ...initialState,

  loadPosts: async () => {
    const generation = lifecycleGeneration;
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
          feedService
            .fetchWallSocialEvents(authorPeerId, postIds)
            .catch((err) => log.warn('Failed to fetch wall social events from relay', err));
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

      if (generation === lifecycleGeneration) set({ posts: wallPosts, isLoading: false });
    } catch (err) {
      if (generation !== lifecycleGeneration) return;
      log.error('Failed to load posts', err);
      set({ error: getErrorMessage(err), isLoading: false });
    }
  },

  createPost: async (
    content: string,
    contentType: WallContentType = 'post',
    media?: DraftMedia[],
    visibility?: PostVisibility,
  ) => {
    const generation = lifecycleGeneration;
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
          if (!m.mediaHash || m.fileSize == null) {
            throw new Error('Attachment must be imported before publishing');
          }
          const fileSize = m.fileSize;
          const mediaHash = m.mediaHash;
          const mimeType = m.mimeType || fallbackMimeType;

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
      if (generation !== lifecycleGeneration) return;

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
        relayStatus: result.relayStatus,
      };

      set((state) => ({
        posts: [newPost, ...state.posts],
      }));
    } catch (err) {
      log.error('Failed to create post', err);
      throw err;
    }
  },

  shareToWall: async (comment: string, sharedFrom: SharedFrom) => {
    const generation = lifecycleGeneration;
    try {
      // Build the content text: user comment + marker for shared content
      // The shared metadata is stored in the sharedFrom field on WallPost
      const contentForBackend = comment.trim()
        ? `${comment.trim()}\n\n[Shared from ${sharedFrom.authorName}]`
        : `[Shared from ${sharedFrom.authorName}]`;

      const selectedVisibility = useSettingsStore.getState().defaultVisibility;
      const result = await postsService.createPost('shared', contentForBackend, selectedVisibility);
      if (generation !== lifecycleGeneration) return;

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
        relayStatus: result.relayStatus,
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
    const generation = lifecycleGeneration;
    try {
      const result = await postsService.updatePost(postId, content);
      if (generation !== lifecycleGeneration) return;

      // Update local state
      set((state) => ({
        posts: state.posts.map((post) =>
          post.postId === postId ? { ...post, content, relayStatus: result.relayStatus } : post,
        ),
        editingPostId: null,
      }));
    } catch (err) {
      log.error('Failed to update post', err);
      throw err;
    }
  },

  deletePost: async (postId: string) => {
    const generation = lifecycleGeneration;
    try {
      const result = await postsService.deletePost(postId);
      if (generation !== lifecycleGeneration) return;

      // Keep a truthful tombstone placeholder until the relay acknowledges it.
      set((state) => ({
        posts: state.posts.map((post) =>
          post.postId === postId
            ? { ...post, relayStatus: result.relayStatus, deletionPending: true }
            : post,
        ),
      }));
    } catch (err) {
      log.error('Failed to delete post', err);
      throw err;
    }
  },

  setEditingPost: (postId: string | null) => {
    set({ editingPostId: postId });
  },

  setPostRelayStatus: (postId: string, status: PostRelayStatus) => {
    set((state) => ({
      posts:
        status === 'relay_acknowledged'
          ? state.posts.filter((post) => !(post.postId === postId && post.deletionPending))
          : state.posts.map((post) =>
              post.postId === postId ? { ...post, relayStatus: status } : post,
            ),
    }));
  },

  likePost: async (postId: string) => {
    const generation = lifecycleGeneration;
    const currentPost = get().posts.find((post) => post.postId === postId);
    if (!currentPost) return;

    try {
      const summary = currentPost.liked
        ? await likesService.unlikePost(postId)
        : await likesService.likePost(postId);
      if (generation !== lifecycleGeneration) return;
      feedService
        .syncWallSocialEventsToRelay()
        .catch((err) => log.warn('Failed to sync wall reaction event to relay', err));

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
    const generation = lifecycleGeneration;
    if (get().loadingComments.has(postId)) return;

    set((state) => {
      const loadingComments = new Set(state.loadingComments);
      loadingComments.add(postId);
      return { loadingComments };
    });

    try {
      const comments = await commentsService.getComments(postId);
      if (generation !== lifecycleGeneration) return;
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
      if (generation !== lifecycleGeneration) return;
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
    const generation = lifecycleGeneration;
    const comment = await commentsService.addComment(postId, content);
    if (generation !== lifecycleGeneration) return;
    feedService
      .syncWallSocialEventsToRelay()
      .catch((err) => log.warn('Failed to sync wall comment event to relay', err));
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
    const generation = lifecycleGeneration;
    await commentsService.deleteComment(commentId);
    if (generation !== lifecycleGeneration) return;
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
  reset: () => {
    lifecycleGeneration += 1;
    set({
      ...initialState,
      posts: [],
      commentsByPost: {},
      expandedComments: new Set(),
      loadingComments: new Set(),
    });
  },
}));

export interface PostRelayStatusEvent {
  post_id: string;
  event_id: string;
  status: PostRelayStatus;
}

/** Apply a network publication event only to a post in the active profile store. */
export function applyPostRelayStatusEvent(event: PostRelayStatusEvent): void {
  useWallStore.getState().setPostRelayStatus(event.post_id, event.status);
}
