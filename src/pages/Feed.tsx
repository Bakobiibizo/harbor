import { useState, useMemo, useEffect, useRef, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import toast from 'react-hot-toast';
import { FeedIcon, EllipsisIcon, PlusIcon } from '../components/icons';
import { PostMedia, type PostMediaItem } from '../components/common/PostMedia';
import { ModalityFilter } from '../components/common/ModalityFilter';
import { useFeedStore, useContactsStore, useWallStore, useSettingsStore } from '../stores';
import { postsService } from '../services/posts';
import { createLogger } from '../utils/logger';
import { safePeerLabel } from '../utils/relayName';
import type { FeedItem } from '../types';
import type { SharedFrom, Comment } from '../stores';
import { useIdentityStore } from '../stores';
import { matchesModalityFilter } from '../utils/postModality';
import { HARBOR_SHORTCUT_EVENTS } from '../hooks';

const log = createLogger('Feed');
import { getInitials, getContactColor, formatDate } from '../utils/formatting';

// Dropdown menu component
function PostMenu({
  isOpen,
  onClose,
  onHide,
  onSnooze,
  authorName,
}: {
  isOpen: boolean;
  onClose: () => void;
  onHide: () => void;
  onSnooze: (hours: number) => void;
  authorName: string;
}) {
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    if (isOpen) {
      document.addEventListener('mousedown', handleClickOutside);
    }
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  return (
    <div
      ref={menuRef}
      className="absolute right-0 top-full mt-1 w-56 rounded-lg shadow-lg z-50 overflow-hidden"
      style={{
        background: 'hsl(var(--harbor-bg-elevated))',
        border: '1px solid hsl(var(--harbor-border-subtle))',
      }}
    >
      <button
        onClick={() => {
          onHide();
          onClose();
        }}
        className="w-full px-4 py-3 text-left text-sm flex items-center gap-3 transition-colors hover:bg-white/5"
        style={{ color: 'hsl(var(--harbor-text-primary))' }}
      >
        <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={1.5}
            d="M13.875 18.825A10.05 10.05 0 0112 19c-4.478 0-8.268-2.943-9.543-7a9.97 9.97 0 011.563-3.029m5.858.908a3 3 0 114.243 4.243M9.878 9.878l4.242 4.242M9.88 9.88l-3.29-3.29m7.532 7.532l3.29 3.29M3 3l3.59 3.59m0 0A9.953 9.953 0 0112 5c4.478 0 8.268 2.943 9.543 7a10.025 10.025 0 01-4.132 5.411m0 0L21 21"
          />
        </svg>
        Hide this post
      </button>
      <div style={{ borderTop: '1px solid hsl(var(--harbor-border-subtle))' }}>
        <div className="px-4 py-2 text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
          Snooze {authorName}
        </div>
        <button
          onClick={() => {
            onSnooze(24);
            onClose();
          }}
          className="w-full px-4 py-2.5 text-left text-sm flex items-center gap-3 transition-colors hover:bg-white/5"
          style={{ color: 'hsl(var(--harbor-text-primary))' }}
        >
          <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={1.5}
              d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"
            />
          </svg>
          For 24 hours
        </button>
        <button
          onClick={() => {
            onSnooze(24 * 7);
            onClose();
          }}
          className="w-full px-4 py-2.5 text-left text-sm flex items-center gap-3 transition-colors hover:bg-white/5"
          style={{ color: 'hsl(var(--harbor-text-primary))' }}
        >
          <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={1.5}
              d="M8 7V3m8 4V3m-9 8h10M5 21h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z"
            />
          </svg>
          For 7 days
        </button>
        <button
          onClick={() => {
            onSnooze(24 * 30);
            onClose();
          }}
          className="w-full px-4 py-2.5 text-left text-sm flex items-center gap-3 transition-colors hover:bg-white/5"
          style={{ color: 'hsl(var(--harbor-text-primary))' }}
        >
          <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={1.5}
              d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z"
            />
          </svg>
          For 30 days
        </button>
      </div>
    </div>
  );
}

// Share modal component
function ShareModal({
  post,
  onClose,
  onShare,
}: {
  post: UnifiedPost;
  onClose: () => void;
  onShare: (comment: string) => void;
}) {
  const [comment, setComment] = useState('');
  const [isSharing, setIsSharing] = useState(false);
  const overlayRef = useRef<HTMLDivElement>(null);

  const handleSubmit = async () => {
    setIsSharing(true);
    try {
      await onShare(comment);
    } finally {
      setIsSharing(false);
    }
  };

  return (
    <div
      ref={overlayRef}
      className="fixed inset-0 z-50 flex items-center justify-center p-4"
      style={{ background: 'rgba(0, 0, 0, 0.6)', backdropFilter: 'blur(4px)' }}
      onClick={(e) => {
        if (e.target === overlayRef.current) onClose();
      }}
    >
      <div
        className="w-full max-w-lg rounded-xl overflow-hidden"
        style={{
          background: 'hsl(var(--harbor-bg-elevated))',
          border: '1px solid hsl(var(--harbor-border-subtle))',
          boxShadow: '0 25px 50px rgba(0, 0, 0, 0.5)',
        }}
      >
        {/* Modal header */}
        <div
          className="px-5 py-4 flex items-center justify-between border-b"
          style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
        >
          <h3
            className="text-lg font-semibold"
            style={{ color: 'hsl(var(--harbor-text-primary))' }}
          >
            Repost to profile
          </h3>
          <button
            onClick={onClose}
            className="p-1.5 rounded-lg transition-colors hover:bg-white/5"
            style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
          >
            <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={1.5}
                d="M6 18L18 6M6 6l12 12"
              />
            </svg>
          </button>
        </div>

        {/* Comment input */}
        <div className="p-5">
          <textarea
            placeholder="Add a comment (optional)..."
            value={comment}
            onChange={(e) => setComment(e.target.value)}
            rows={3}
            className="w-full resize-none text-sm leading-relaxed p-3 rounded-lg mb-4"
            style={{
              background: 'hsl(var(--harbor-surface-1))',
              border: '1px solid hsl(var(--harbor-border-subtle))',
              outline: 'none',
              color: 'hsl(var(--harbor-text-primary))',
            }}
            autoFocus
          />

          {/* Original post preview */}
          <div
            className="rounded-lg overflow-hidden"
            style={{
              background: 'hsl(var(--harbor-surface-1))',
              border: '1px solid hsl(var(--harbor-border-subtle))',
            }}
          >
            <div className="px-4 py-3 flex items-center gap-3">
              <div
                className="w-8 h-8 rounded-full flex items-center justify-center text-xs font-semibold text-white flex-shrink-0"
                style={{ background: post.author.avatarGradient }}
              >
                {post.author.name
                  .split(' ')
                  .map((n) => n[0])
                  .join('')
                  .toUpperCase()
                  .slice(0, 2)}
              </div>
              <div>
                <p
                  className="font-medium text-sm"
                  style={{ color: 'hsl(var(--harbor-text-primary))' }}
                >
                  {post.author.name}
                </p>
              </div>
            </div>
            <div className="px-4 pb-3">
              <p
                className="text-sm leading-relaxed whitespace-pre-wrap line-clamp-4"
                style={{ color: 'hsl(var(--harbor-text-secondary))' }}
              >
                {post.content}
              </p>
            </div>
          </div>
        </div>

        {/* Modal footer */}
        <div
          className="px-5 py-4 flex items-center justify-end gap-3 border-t"
          style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
        >
          <button
            onClick={onClose}
            className="px-4 py-2 rounded-lg text-sm font-medium transition-colors duration-200"
            style={{ color: 'hsl(var(--harbor-text-secondary))' }}
          >
            Cancel
          </button>
          <button
            onClick={handleSubmit}
            disabled={isSharing}
            className="px-5 py-2 rounded-lg text-sm font-medium transition-all duration-200 disabled:opacity-60"
            style={{
              background:
                'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
              color: 'white',
              boxShadow: '0 4px 12px hsl(var(--harbor-primary) / 0.3)',
            }}
          >
            {isSharing ? 'Reposting...' : 'Repost to profile'}
          </button>
        </div>
      </div>
    </div>
  );
}

// Comments section component for a single post
function CommentsSection({
  postId,
  comments,
  isLoading,
  onAddComment,
  onDeleteComment,
  currentPeerId,
  formatDate,
  getInitials,
}: {
  postId: string;
  comments: Comment[];
  isLoading: boolean;
  onAddComment: (postId: string, content: string) => Promise<void>;
  onDeleteComment: (postId: string, commentId: string) => Promise<void>;
  currentPeerId: string;
  formatDate: (date: Date) => string;
  getInitials: (name: string) => string;
}) {
  const [commentText, setCommentText] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  const handleSubmit = async () => {
    const trimmed = commentText.trim();
    if (!trimmed || isSubmitting) return;

    setIsSubmitting(true);
    try {
      await onAddComment(postId, trimmed);
      setCommentText('');
    } catch {
      toast.error('Failed to add comment');
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  };

  return (
    <div className="border-t" style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}>
      {/* Comments list */}
      <div className="px-5 pt-3 pb-1">
        {isLoading ? (
          <div className="flex items-center justify-center py-4">
            <div
              className="w-5 h-5 border-2 rounded-full animate-spin"
              style={{
                borderColor: 'hsl(var(--harbor-border-subtle))',
                borderTopColor: 'hsl(var(--harbor-primary))',
              }}
            />
            <span className="ml-2 text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
              Loading comments...
            </span>
          </div>
        ) : comments.length === 0 ? (
          <p
            className="text-xs py-3 text-center"
            style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
          >
            No comments yet. Be the first to comment.
          </p>
        ) : (
          <div className="space-y-3">
            {comments.map((comment) => (
              <div key={comment.commentId} className="flex gap-2.5 group">
                {/* Avatar */}
                <div
                  className="w-7 h-7 rounded-full flex items-center justify-center text-[10px] font-semibold text-white flex-shrink-0 mt-0.5"
                  style={{
                    background: getContactColor(comment.authorPeerId),
                  }}
                >
                  {getInitials(safePeerLabel(comment.authorPeerId, undefined, comment.authorName))}
                </div>

                {/* Comment content */}
                <div className="flex-1 min-w-0">
                  <div
                    className="rounded-lg px-3 py-2"
                    style={{ background: 'hsl(var(--harbor-surface-1))' }}
                  >
                    <span
                      className="font-semibold text-xs"
                      style={{ color: 'hsl(var(--harbor-text-primary))' }}
                    >
                      {safePeerLabel(comment.authorPeerId, undefined, comment.authorName)}
                    </span>
                    <p
                      className="text-sm leading-relaxed whitespace-pre-wrap mt-0.5"
                      style={{ color: 'hsl(var(--harbor-text-secondary))' }}
                    >
                      {comment.content}
                    </p>
                  </div>
                  <div className="flex items-center gap-3 mt-1 px-1">
                    <span
                      className="text-[11px]"
                      style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                    >
                      {formatDate(new Date(comment.createdAt * 1000))}
                    </span>
                    {comment.authorPeerId === currentPeerId && (
                      <button
                        onClick={() => onDeleteComment(postId, comment.commentId)}
                        className="text-[11px] opacity-0 group-hover:opacity-100 transition-opacity duration-200 hover:underline"
                        style={{ color: 'hsl(var(--harbor-error))' }}
                      >
                        Delete
                      </button>
                    )}
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Comment input */}
      <div
        className="px-5 py-3 flex items-center gap-3 border-t"
        style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
      >
        <input
          ref={inputRef}
          type="text"
          placeholder="Write a comment..."
          value={commentText}
          onChange={(e) => setCommentText(e.target.value)}
          onKeyDown={handleKeyDown}
          disabled={isSubmitting}
          className="flex-1 text-sm py-2 px-3 rounded-lg transition-colors duration-200"
          style={{
            background: 'hsl(var(--harbor-surface-1))',
            border: '1px solid hsl(var(--harbor-border-subtle))',
            outline: 'none',
            color: 'hsl(var(--harbor-text-primary))',
          }}
        />
        <button
          onClick={handleSubmit}
          disabled={!commentText.trim() || isSubmitting}
          className="p-2 rounded-lg transition-all duration-200 disabled:opacity-40"
          style={{
            color: commentText.trim()
              ? 'hsl(var(--harbor-primary))'
              : 'hsl(var(--harbor-text-tertiary))',
          }}
          title="Send comment"
        >
          <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M12 19l9 2-9-18-9 18 9-2zm0 0v-8"
            />
          </svg>
        </button>
      </div>
    </div>
  );
}

// Unified post type for both real and mock posts
interface UnifiedPost {
  id: string;
  postId: string; // The actual post ID (used for comments backend)
  content: string;
  contentType: string;
  timestamp: Date;
  likes: number;
  comments: number;
  likedByUser: boolean;
  author: {
    peerId: string;
    name: string;
    avatarGradient: string;
  };
  isReal: boolean;
  media?: { type: 'image' | 'video' | 'audio'; url: string; name?: string }[];
}

type FeedTab = 'all' | 'saved';

export function FindContactsButton() {
  const navigate = useNavigate();
  return (
    <button
      type="button"
      onClick={() => navigate('/network')}
      className="px-4 py-2 rounded-lg text-sm font-medium transition-all duration-200"
      style={{
        background:
          'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
        color: 'white',
        boxShadow: '0 4px 12px hsl(var(--harbor-primary) / 0.3)',
      }}
    >
      Find Contacts
    </button>
  );
}

export function FeedPage() {
  const navigate = useNavigate();
  const {
    feedItems,
    savedPostIds,
    hiddenPostIds,
    snoozedAuthors,
    loadFeed,
    refreshFeed,
    comments,
    commentCounts,
    expandedComments,
    loadingComments,
    toggleComments,
    addComment,
    deleteComment,
    isSyncingRelay,
    lastSyncAt,
    syncError,
    syncStatus,
    getSavedFeedItems,
    toggleLike,
    toggleSave,
    isPostSaved,
    hidePost,
    unhidePost,
    snoozeAuthor,
    unsnoozeAuthor,
    clearHiddenPosts,
    clearSnoozedAuthors,
  } = useFeedStore();
  const { contacts, loadContacts } = useContactsStore();
  const { shareToWall } = useWallStore();
  const identityState = useIdentityStore((s) => s.state);
  const currentPeerId =
    identityState.status === 'unlocked' || identityState.status === 'locked'
      ? identityState.identity.peerId
      : '';
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [activeTab, setActiveTab] = useState<FeedTab>('all');
  const [openMenuId, setOpenMenuId] = useState<string | null>(null);
  const [sharingPost, setSharingPost] = useState<UnifiedPost | null>(null);
  const { socialView, setSocialView } = useSettingsStore();

  // Relay polling is owned by the app-level contact feed worker so it can be
  // cancelled on lock/offline transitions and does not depend on this page.
  useEffect(() => {
    loadFeed().catch((err) => log.error('Failed to load feed', err));
    loadContacts().catch((err) => log.error('Failed to load contacts', err));
  }, [loadFeed, loadContacts]);

  // Track media for feed posts (fetched asynchronously)
  const [postMediaMap, setPostMediaMap] = useState<Record<string, PostMediaItem[]>>({});

  // Fetch media for all feed items when they change
  useEffect(() => {
    let cancelled = false;
    const fetchMedia = async () => {
      const mediaMap: Record<string, PostMediaItem[]> = {};
      await Promise.allSettled(
        feedItems.map(async (item) => {
          try {
            const mediaList = await postsService.getPostMedia(item.postId);
            if (mediaList.length > 0 && !cancelled) {
              mediaMap[item.postId] = mediaList.map((m) => ({
                type: (m.mediaType === 'video'
                  ? 'video'
                  : m.mediaType === 'audio'
                    ? 'audio'
                    : 'image') as 'image' | 'video' | 'audio',
                url: m.mediaHash,
                name: m.fileName,
                sourcePeerId: item.authorPeerId,
                mimeType: m.mimeType,
                totalBytes: m.fileSize,
              }));
            }
          } catch {
            // Media fetch is best-effort
          }
        }),
      );
      if (!cancelled) {
        setPostMediaMap(mediaMap);
      }
    };
    if (feedItems.length > 0) {
      fetchMedia();
    }
    return () => {
      cancelled = true;
    };
  }, [feedItems]);

  const toUnifiedPost = useCallback(
    (item: FeedItem): UnifiedPost => {
      return {
        id: `real-${item.postId}`,
        postId: item.postId,
        content: item.contentText || '',
        contentType: item.contentType,
        timestamp: new Date(item.createdAt * 1000),
        likes: item.likes ?? 0,
        comments: commentCounts[item.postId] || 0,
        likedByUser: item.likedByUser ?? false,
        author: {
          peerId: item.authorPeerId,
          name: safePeerLabel(
            item.authorPeerId,
            item.authorVerifiedQualifiedName,
            item.authorDisplayName,
          ),
          avatarGradient: getContactColor(item.authorPeerId),
        },
        isReal: true,
        media: postMediaMap[item.postId],
      };
    },
    [contacts, commentCounts, postMediaMap],
  );

  // Convert real feed items to unified format
  const allPosts: UnifiedPost[] = useMemo(() => {
    return feedItems
      .map((item: FeedItem): UnifiedPost => {
        return {
          id: `real-${item.postId}`,
          postId: item.postId,
          content: item.contentText || '',
          contentType: item.contentType,
          timestamp: new Date(item.createdAt * 1000),
          likes: item.likes ?? 0,
          comments: commentCounts[item.postId] || 0,
          likedByUser: item.likedByUser ?? false,
          author: {
            peerId: item.authorPeerId,
            name: safePeerLabel(
              item.authorPeerId,
              item.authorVerifiedQualifiedName,
              item.authorDisplayName,
            ),
            avatarGradient: getContactColor(item.authorPeerId),
          },
          isReal: true,
          media: postMediaMap[item.postId],
        };
      })
      .sort((a, b) => b.timestamp.getTime() - a.timestamp.getTime());
  }, [feedItems, contacts, commentCounts, postMediaMap]);

  const savedPosts: UnifiedPost[] = useMemo(() => {
    return getSavedFeedItems()
      .map(toUnifiedPost)
      .sort((a, b) => b.timestamp.getTime() - a.timestamp.getTime());
  }, [getSavedFeedItems, toUnifiedPost, savedPostIds, feedItems]);

  // Select posts based on active tab
  const selectedPosts: UnifiedPost[] = activeTab === 'saved' ? savedPosts : allPosts;
  const posts = selectedPosts.filter((post) => {
    return matchesModalityFilter(socialView, post.contentType, post.media);
  });

  const handleRefresh = useCallback(async () => {
    setIsRefreshing(true);
    try {
      await refreshFeed();
      toast.success('Feed refreshed!');
    } catch {
      toast.error('Failed to refresh feed');
    } finally {
      setIsRefreshing(false);
    }
  }, [refreshFeed]);

  const handleLike = async (post: UnifiedPost) => {
    try {
      await toggleLike(post.postId);
    } catch (error) {
      log.error('Failed to update feed reaction', error);
      toast.error('Could not update reaction');
    }
  };

  const handleSave = (post: UnifiedPost) => {
    const wasSaved = isPostSaved(post.postId);
    toggleSave(post.postId);
    toast.success(wasSaved ? 'Removed from saved posts' : 'Post saved');
  };

  const isSaved = (post: UnifiedPost): boolean => {
    return isPostSaved(post.postId);
  };

  const handleHidePost = (post: UnifiedPost) => {
    hidePost(post.postId);
    toast.success(
      (t) => (
        <span>
          Post hidden{' '}
          <button
            onClick={() => {
              unhidePost(post.postId);
              toast.dismiss(t.id);
            }}
            className="font-semibold underline"
          >
            Undo
          </button>
        </span>
      ),
      { duration: 6000 },
    );
  };

  const handleSnoozeUser = (post: UnifiedPost, hours: number) => {
    snoozeAuthor(post.author.peerId, hours);
    toast.success(
      (t) => (
        <span>
          Snoozed {post.author.name}{' '}
          <button
            onClick={() => {
              unsnoozeAuthor(post.author.peerId);
              toast.dismiss(t.id);
            }}
            className="font-semibold underline"
          >
            Undo
          </button>
        </span>
      ),
      { duration: 6000 },
    );
  };

  const handleShareToWall = async (post: UnifiedPost, comment: string) => {
    const sharedFromData: SharedFrom = {
      authorName: post.author.name,
      authorPeerId: post.author.peerId,
      avatarGradient: post.author.avatarGradient,
      originalContent: post.content,
      originalPostId: post.id,
    };

    try {
      await shareToWall(comment, sharedFromData);
      setSharingPost(null);
      toast.success('Reposted to your profile');
    } catch {
      toast.error('Failed to share post');
    }
  };

  return (
    <div className="h-full flex flex-col" style={{ background: 'hsl(var(--harbor-bg-primary))' }}>
      {/* Header */}
      <header
        className="harbor-page-gutter-x flex-shrink-0 border-b py-4"
        style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
      >
        <div className="max-w-3xl mx-auto">
          <div className="mb-4 flex flex-wrap items-start justify-between gap-4">
            <div>
              <h1
                className="text-2xl font-bold"
                style={{ color: 'hsl(var(--harbor-text-primary))' }}
              >
                Feed
              </h1>
              <p className="text-sm mt-1" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                {activeTab === 'saved' ? 'Your saved posts' : 'Updates from your contacts'}
              </p>
              <p
                className="text-xs mt-1"
                style={{
                  color:
                    syncStatus === 'partial_failure'
                      ? 'hsl(var(--harbor-warning))'
                      : 'hsl(var(--harbor-text-tertiary))',
                }}
              >
                {syncStatus === 'in_progress'
                  ? 'Sync in progress… local feed remains available.'
                  : lastSyncAt
                    ? `Last synced ${formatDate(new Date(lastSyncAt * 1000))}${syncError ? ' · Partial sync failure, retry available.' : ''}`
                    : 'Not synced yet. Local feed remains available.'}
              </p>
            </div>

            <div className="flex items-center gap-3">
              {isSyncingRelay && (
                <div className="flex items-center gap-2">
                  <div
                    className="w-3 h-3 border-2 border-t-transparent rounded-full animate-spin"
                    style={{
                      borderColor: 'hsl(var(--harbor-primary))',
                      borderTopColor: 'transparent',
                    }}
                  />
                  <span className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                    Syncing...
                  </span>
                </div>
              )}
              <button
                type="button"
                onClick={() =>
                  window.dispatchEvent(new CustomEvent(HARBOR_SHORTCUT_EVENTS.newPost))
                }
                className="harbor-interactive flex items-center gap-2 rounded-lg px-4 py-2 text-sm font-semibold text-white"
                style={{ background: 'hsl(var(--harbor-primary))' }}
              >
                <PlusIcon className="h-4 w-4" />
                Add post
              </button>
              <button
                onClick={handleRefresh}
                disabled={isRefreshing || activeTab === 'saved'}
                className="px-4 py-2 rounded-lg text-sm font-medium transition-all duration-200"
                style={{
                  background: 'hsl(var(--harbor-surface-1))',
                  color: 'hsl(var(--harbor-text-secondary))',
                  border: '1px solid hsl(var(--harbor-border-subtle))',
                  opacity: isRefreshing || activeTab === 'saved' ? 0.6 : 1,
                }}
              >
                {isRefreshing ? 'Refreshing...' : 'Refresh'}
              </button>
            </div>
          </div>

          <div className="mb-3">
            <ModalityFilter value={socialView} onChange={setSocialView} label="Filter your feed" />
          </div>
          {/* Saved-state tabs */}
          <div
            className="flex gap-1 p-1 rounded-lg"
            style={{ background: 'hsl(var(--harbor-surface-1))' }}
          >
            <button
              onClick={() => setActiveTab('all')}
              className="flex-1 px-4 py-2 rounded-md text-sm font-medium transition-all duration-200"
              style={{
                background:
                  activeTab === 'all'
                    ? 'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))'
                    : 'transparent',
                color: activeTab === 'all' ? 'white' : 'hsl(var(--harbor-text-secondary))',
                boxShadow:
                  activeTab === 'all' ? '0 2px 8px hsl(var(--harbor-primary) / 0.3)' : 'none',
              }}
            >
              All Posts
            </button>
            <button
              onClick={() => setActiveTab('saved')}
              className="flex-1 px-4 py-2 rounded-md text-sm font-medium transition-all duration-200 flex items-center justify-center gap-2"
              style={{
                background:
                  activeTab === 'saved'
                    ? 'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))'
                    : 'transparent',
                color: activeTab === 'saved' ? 'white' : 'hsl(var(--harbor-text-secondary))',
                boxShadow:
                  activeTab === 'saved' ? '0 2px 8px hsl(var(--harbor-primary) / 0.3)' : 'none',
              }}
            >
              <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={1.5}
                  d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z"
                />
              </svg>
              Saved
              {savedPostIds.length > 0 && (
                <span
                  className="px-1.5 py-0.5 rounded-full text-xs"
                  style={{
                    background:
                      activeTab === 'saved'
                        ? 'rgba(255,255,255,0.2)'
                        : 'hsl(var(--harbor-primary) / 0.15)',
                    color: activeTab === 'saved' ? 'white' : 'hsl(var(--harbor-primary))',
                  }}
                >
                  {savedPostIds.length}
                </span>
              )}
            </button>
          </div>

          {(hiddenPostIds.length > 0 || snoozedAuthors.length > 0) && (
            <div
              className="mt-3 rounded-lg p-3 text-xs flex flex-wrap items-center gap-3"
              style={{
                background: 'hsl(var(--harbor-surface-1))',
                color: 'hsl(var(--harbor-text-secondary))',
                border: '1px solid hsl(var(--harbor-border-subtle))',
              }}
            >
              <span>
                Feed filters: {hiddenPostIds.length} hidden post
                {hiddenPostIds.length === 1 ? '' : 's'}, {snoozedAuthors.length} snoozed contact
                {snoozedAuthors.length === 1 ? '' : 's'}
              </span>
              {hiddenPostIds.length > 0 && (
                <button
                  onClick={() => {
                    clearHiddenPosts();
                    toast.success('Hidden posts restored');
                  }}
                  className="font-semibold underline"
                  style={{ color: 'hsl(var(--harbor-primary))' }}
                >
                  Restore hidden
                </button>
              )}
              {snoozedAuthors.length > 0 && (
                <button
                  onClick={() => {
                    clearSnoozedAuthors();
                    toast.success('Snoozed contacts restored');
                  }}
                  className="font-semibold underline"
                  style={{ color: 'hsl(var(--harbor-primary))' }}
                >
                  Unsnooze all
                </button>
              )}
            </div>
          )}
        </div>
      </header>

      <div className="harbor-page-gutter flex-1 overflow-y-auto">
        <div className="max-w-3xl mx-auto space-y-8">
          {posts.length === 0 ? (
            <div className="text-center py-16">
              <div
                className="w-20 h-20 rounded-lg flex items-center justify-center mx-auto mb-4"
                style={{ background: 'hsl(var(--harbor-surface-1))' }}
              >
                {activeTab === 'saved' ? (
                  <svg
                    className="w-10 h-10"
                    style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                    fill="none"
                    viewBox="0 0 24 24"
                    stroke="currentColor"
                  >
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={1.5}
                      d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z"
                    />
                  </svg>
                ) : (
                  <FeedIcon
                    className="w-10 h-10"
                    style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                  />
                )}
              </div>
              <h3
                className="text-lg font-semibold mb-2"
                style={{ color: 'hsl(var(--harbor-text-primary))' }}
              >
                {activeTab === 'saved' ? 'No saved posts' : 'Your feed is empty'}
              </h3>
              <p
                className="text-sm max-w-xs mx-auto mb-4"
                style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
              >
                {activeTab === 'saved'
                  ? 'Save posts from your feed to view them here later.'
                  : "When your contacts share posts and grant you permission to view them, they'll appear here."}
              </p>
              {activeTab === 'saved' ? (
                <button
                  onClick={() => setActiveTab('all')}
                  className="px-4 py-2 rounded-lg text-sm font-medium transition-all duration-200"
                  style={{
                    background:
                      'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
                    color: 'white',
                    boxShadow: '0 4px 12px hsl(var(--harbor-primary) / 0.3)',
                  }}
                >
                  Browse Feed
                </button>
              ) : (
                <FindContactsButton />
              )}
            </div>
          ) : (
            posts.map((post) => {
              const saved = isSaved(post);

              return (
                <article
                  key={post.id}
                  className="rounded-lg overflow-hidden"
                  style={{
                    background: 'hsl(var(--harbor-bg-elevated))',
                    border: '1px solid hsl(var(--harbor-border-subtle))',
                  }}
                >
                  {/* Post header */}
                  <div
                    className="px-5 py-4 flex items-center justify-between border-b"
                    style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
                  >
                    <div className="flex items-center gap-3">
                      <div
                        className="w-10 h-10 rounded-full flex items-center justify-center text-sm font-semibold text-white"
                        style={{
                          background: post.author.avatarGradient,
                        }}
                      >
                        {getInitials(post.author.name)}
                      </div>
                      <div>
                        <div className="flex items-center gap-2">
                          <button
                            onClick={() =>
                              navigate(`/contacts/${encodeURIComponent(post.author.peerId)}/wall`)
                            }
                            className="font-semibold text-sm text-left hover:underline"
                            style={{ color: 'hsl(var(--harbor-text-primary))' }}
                            title={`Open ${post.author.name}'s profile`}
                          >
                            {post.author.name}
                          </button>
                          <span
                            className="text-[10px] px-1.5 py-0.5 rounded"
                            style={{
                              background: 'hsl(var(--harbor-success) / 0.15)',
                              color: 'hsl(var(--harbor-success))',
                            }}
                          >
                            P2P
                          </span>
                        </div>
                        <p
                          className="text-xs"
                          style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                        >
                          {formatDate(post.timestamp)}
                        </p>
                      </div>
                    </div>

                    <div className="relative">
                      <button
                        onClick={() => setOpenMenuId(openMenuId === post.id ? null : post.id)}
                        className="p-2 rounded-lg transition-colors duration-200 hover:bg-white/5"
                        style={{
                          color: 'hsl(var(--harbor-text-tertiary))',
                        }}
                      >
                        <EllipsisIcon className="w-5 h-5" />
                      </button>
                      <PostMenu
                        isOpen={openMenuId === post.id}
                        onClose={() => setOpenMenuId(null)}
                        onHide={() => handleHidePost(post)}
                        onSnooze={(hours) => handleSnoozeUser(post, hours)}
                        authorName={post.author.name}
                      />
                    </div>
                  </div>

                  {/* Post content */}
                  <div className="px-5 py-5">
                    <p
                      className="text-base leading-relaxed whitespace-pre-wrap"
                      style={{ color: 'hsl(var(--harbor-text-primary))' }}
                    >
                      {post.content}
                    </p>
                  </div>

                  {post.media && post.media.length > 0 && (
                    <div className="px-5 pb-5">
                      <PostMedia media={post.media} />
                    </div>
                  )}

                  {/* Post actions */}
                  <div
                    className="px-5 py-3 flex items-center gap-6 border-t"
                    style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
                  >
                    <button
                      onClick={() => handleLike(post)}
                      className="flex items-center gap-2 transition-colors duration-200"
                      style={{
                        color: post.likedByUser
                          ? 'hsl(var(--harbor-error))'
                          : 'hsl(var(--harbor-text-secondary))',
                      }}
                    >
                      <svg
                        className="w-5 h-5"
                        fill={post.likedByUser ? 'currentColor' : 'none'}
                        viewBox="0 0 24 24"
                        stroke="currentColor"
                      >
                        <path
                          strokeLinecap="round"
                          strokeLinejoin="round"
                          strokeWidth={1.5}
                          d="M4.318 6.318a4.5 4.5 0 000 6.364L12 20.364l7.682-7.682a4.5 4.5 0 00-6.364-6.364L12 7.636l-1.318-1.318a4.5 4.5 0 00-6.364 0z"
                        />
                      </svg>
                      <span className="text-sm">{post.likes}</span>
                    </button>

                    <button
                      onClick={() => toggleComments(post.postId)}
                      className="flex items-center gap-2 transition-colors duration-200"
                      style={{
                        color: expandedComments.has(post.postId)
                          ? 'hsl(var(--harbor-primary))'
                          : 'hsl(var(--harbor-text-secondary))',
                      }}
                    >
                      <svg
                        className="w-5 h-5"
                        fill="none"
                        viewBox="0 0 24 24"
                        stroke="currentColor"
                      >
                        <path
                          strokeLinecap="round"
                          strokeLinejoin="round"
                          strokeWidth={1.5}
                          d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z"
                        />
                      </svg>
                      <span className="text-sm">
                        {post.comments > 0 ? `Comments (${post.comments})` : 'Comment'}
                      </span>
                    </button>

                    <button
                      onClick={() => setSharingPost(post)}
                      className="flex items-center gap-2 transition-colors duration-200"
                      style={{
                        color: 'hsl(var(--harbor-text-secondary))',
                      }}
                    >
                      <svg
                        className="w-5 h-5"
                        fill="none"
                        viewBox="0 0 24 24"
                        stroke="currentColor"
                      >
                        <path
                          strokeLinecap="round"
                          strokeLinejoin="round"
                          strokeWidth={1.5}
                          d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12"
                        />
                      </svg>
                      <span className="text-sm">Share</span>
                    </button>

                    <button
                      onClick={() => handleSave(post)}
                      className="flex items-center gap-2 transition-colors duration-200 ml-auto"
                      style={{
                        color: saved
                          ? 'hsl(var(--harbor-primary))'
                          : 'hsl(var(--harbor-text-secondary))',
                      }}
                    >
                      <svg
                        className="w-5 h-5"
                        fill={saved ? 'currentColor' : 'none'}
                        viewBox="0 0 24 24"
                        stroke="currentColor"
                      >
                        <path
                          strokeLinecap="round"
                          strokeLinejoin="round"
                          strokeWidth={1.5}
                          d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z"
                        />
                      </svg>
                      <span className="text-sm">{saved ? 'Saved' : 'Save'}</span>
                    </button>
                  </div>

                  {/* Comments section (expandable) */}
                  {expandedComments.has(post.postId) && (
                    <CommentsSection
                      postId={post.postId}
                      comments={comments[post.postId] || []}
                      isLoading={loadingComments.has(post.postId)}
                      onAddComment={addComment}
                      onDeleteComment={deleteComment}
                      currentPeerId={currentPeerId}
                      formatDate={formatDate}
                      getInitials={getInitials}
                    />
                  )}
                </article>
              );
            })
          )}
        </div>
      </div>

      {/* Share modal */}
      {sharingPost && (
        <ShareModal
          post={sharingPost}
          onClose={() => setSharingPost(null)}
          onShare={(comment) => handleShareToWall(sharingPost, comment)}
        />
      )}
    </div>
  );
}
