import { useState, useRef, useEffect, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import toast from 'react-hot-toast';
import { useIdentityStore, useSettingsStore, useWallStore } from '../stores';
import type { WallContentType } from '../stores';
import type { FeedItem, PostVisibility } from '../types';
import {
  feedService,
  type WallPreviewPerspective,
  type WallVisibilityStats,
} from '../services/feed';
import { getShareableContactString } from '../services/network';
import { WallIcon, EllipsisIcon } from '../components/icons';
import { LinkPreviewCard } from '../components/common/LinkPreviewCard';
import { PostMedia } from '../components/common/PostMedia';
import { extractFirstUrl } from '../utils/urlDetection';
import { createLogger } from '../utils/logger';
import type { Comment } from '../services/comments';

const log = createLogger('Wall');

/** Content type metadata for UI rendering */
const CONTENT_TYPES: {
  type: WallContentType;
  label: string;
  icon: React.ReactNode;
  placeholder: string;
  charLimit?: number;
}[] = [
  {
    type: 'post',
    label: 'Post',
    icon: (
      <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth={1.5}
          d="M19.5 14.25v-2.625a3.375 3.375 0 00-3.375-3.375h-1.5A1.125 1.125 0 0113.5 7.125v-1.5a3.375 3.375 0 00-3.375-3.375H8.25m0 12.75h7.5m-7.5 3H12M10.5 2.25H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 00-9-9z"
        />
      </svg>
    ),
    placeholder: 'Share your thoughts, ideas, or creative work...',
  },
  {
    type: 'thought',
    label: 'Tweet',
    icon: (
      <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth={1.5}
          d="M12 18v-5.25m0 0a6.01 6.01 0 001.5-.189m-1.5.189a6.01 6.01 0 01-1.5-.189m3.75 7.478a12.06 12.06 0 01-4.5 0m3.75 2.383a14.406 14.406 0 01-3 0M14.25 18v-.192c0-.983.658-1.823 1.508-2.316a7.5 7.5 0 10-7.517 0c.85.493 1.509 1.333 1.509 2.316V18"
        />
      </svg>
    ),
    placeholder: "What's on your mind? (280 characters max)",
    charLimit: 280,
  },
  {
    type: 'image',
    label: 'Image',
    icon: (
      <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth={1.5}
          d="M2.25 15.75l5.159-5.159a2.25 2.25 0 013.182 0l5.159 5.159m-1.5-1.5l1.409-1.409a2.25 2.25 0 013.182 0l2.909 2.909m-18 3.75h16.5a1.5 1.5 0 001.5-1.5V6a1.5 1.5 0 00-1.5-1.5H3.75A1.5 1.5 0 002.25 6v12a1.5 1.5 0 001.5 1.5zm10.5-11.25h.008v.008h-.008V8.25zm.375 0a.375.375 0 11-.75 0 .375.375 0 01.75 0z"
        />
      </svg>
    ),
    placeholder: 'Add a caption for your image...',
  },
  {
    type: 'video',
    label: 'Video',
    icon: (
      <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth={1.5}
          d="M15.75 10.5l4.72-4.72a.75.75 0 011.28.53v11.38a.75.75 0 01-1.28.53l-4.72-4.72M4.5 18.75h9a2.25 2.25 0 002.25-2.25v-9a2.25 2.25 0 00-2.25-2.25h-9A2.25 2.25 0 002.25 7.5v9a2.25 2.25 0 002.25 2.25z"
        />
      </svg>
    ),
    placeholder: 'Add a caption for your video...',
  },
  {
    type: 'audio',
    label: 'Audio',
    icon: (
      <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth={1.5}
          d="M19.114 5.636a9 9 0 010 12.728M16.463 8.288a5.25 5.25 0 010 7.424M6.75 8.25l4.72-4.72a.75.75 0 011.28.53v15.88a.75.75 0 01-1.28.53l-4.72-4.72H4.51c-.88 0-1.704-.507-1.938-1.354A9.01 9.01 0 012.25 12c0-.83.112-1.633.322-2.396C2.806 8.756 3.63 8.25 4.51 8.25H6.75z"
        />
      </svg>
    ),
    placeholder: 'Add a caption for your audio...',
  },
];

/** Per-post visibility options */
const VISIBILITY_OPTIONS: { visibility: PostVisibility; label: string; description: string }[] = [
  {
    visibility: 'contacts',
    label: 'Contacts only',
    description: 'Visible to contacts with wall access',
  },
  {
    visibility: 'public',
    label: 'Public',
    description: 'Visible in public previews and RSS',
  },
];

const FILTER_OPTIONS: { type: 'posts' | 'images' | 'videos' | 'audio'; label: string }[] = [
  { type: 'posts', label: 'Posts' },
  { type: 'images', label: 'Images' },
  { type: 'videos', label: 'Videos' },
  { type: 'audio', label: 'Audio' },
] as const;

const PREVIEW_OPTIONS: {
  perspective: WallPreviewPerspective;
  label: string;
  summary: string;
}[] = [
  {
    perspective: 'guest',
    label: 'Guest preview',
    summary: 'Guests see public posts only. Contacts-only posts are hidden.',
  },
  {
    perspective: 'contact',
    label: 'Contact preview',
    summary: 'Contacts with WallRead see public and contacts-only posts.',
  },
  {
    perspective: 'owner',
    label: 'Owner preview',
    summary: 'You see every non-deleted local wall post regardless of visibility.',
  },
];

type ShareAction = 'rss-copy' | 'rss-export' | 'feed-link' | 'contact-link';

/** Get the icon for a content type (for display in post cards) */
function getContentTypeIcon(contentType: WallContentType) {
  const ct = CONTENT_TYPES.find((c) => c.type === contentType);
  return ct?.icon ?? null;
}

/** Get the label for a content type */
function getContentTypeLabel(contentType: WallContentType) {
  const ct = CONTENT_TYPES.find((c) => c.type === contentType);
  return ct?.label ?? 'Post';
}

function parseWallContentType(contentType: string): WallContentType {
  switch (contentType) {
    case 'thought':
    case 'image':
    case 'video':
    case 'audio':
      return contentType;
    case 'post':
    case 'text':
    default:
      return 'post';
  }
}

function normalizeVisibility(visibility: string): PostVisibility {
  return visibility === 'public' ? 'public' : 'contacts';
}

function formatVisibilityLabel(visibility: string) {
  return normalizeVisibility(visibility) === 'public' ? 'Public' : 'Contacts only';
}

function VisibilityBadge({
  visibility,
  compact = false,
}: {
  visibility: string;
  compact?: boolean;
}) {
  const normalized = normalizeVisibility(visibility);
  return (
    <span
      className={`px-2 py-0.5 rounded-full ${compact ? 'text-[11px]' : 'text-xs'}`}
      title={`Visibility: ${formatVisibilityLabel(visibility)}`}
      style={{
        background:
          normalized === 'public'
            ? 'hsl(var(--harbor-success) / 0.1)'
            : 'hsl(var(--harbor-primary) / 0.1)',
        color:
          normalized === 'public' ? 'hsl(var(--harbor-success))' : 'hsl(var(--harbor-primary))',
      }}
    >
      {formatVisibilityLabel(visibility)}
    </span>
  );
}

function buildRssConfig(identity: { peerId: string; displayName: string }) {
  return {
    base_url: `harbor://peer/${identity.peerId}`,
    title: `${identity.displayName}'s Public Harbor Wall`,
    description:
      'Locally generated RSS XML containing only posts marked Public on this Harbor wall.',
    max_items: 50,
  };
}

function buildRssFilename(displayName: string) {
  const safeName = displayName
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
  return `harbor-${safeName || 'wall'}-public-rss.xml`;
}

function getPreviewExplanation(
  perspective: WallPreviewPerspective,
  stats: WallVisibilityStats | null,
) {
  const publicCount = stats?.publicPosts ?? 0;
  const contactsOnlyCount = stats?.contactsOnlyPosts ?? 0;
  const totalCount = stats?.totalPosts ?? 0;

  switch (perspective) {
    case 'guest':
      return `Guest preview is loaded from the backend as public-only: ${publicCount} public post${publicCount === 1 ? '' : 's'} are visible; ${contactsOnlyCount} contacts-only post${contactsOnlyCount === 1 ? '' : 's'} are hidden.`;
    case 'contact':
      return `Contact preview is loaded from the backend for contacts with WallRead: ${totalCount} post${totalCount === 1 ? '' : 's'} are visible (${publicCount} public + ${contactsOnlyCount} contacts-only).`;
    case 'owner':
      return `Owner preview is loaded from the backend and shows all ${totalCount} non-deleted local post${totalCount === 1 ? '' : 's'}, including contacts-only posts.`;
  }

  return '';
}

function downloadTextFile(filename: string, content: string) {
  const blob = new Blob([content], { type: 'application/rss+xml;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = filename;
  document.body.appendChild(anchor);
  anchor.click();
  anchor.remove();
  URL.revokeObjectURL(url);
}

function formatCommentDate(timestamp: number) {
  return new Date(timestamp * 1000).toLocaleString(undefined, {
    month: 'short',
    day: 'numeric',
    hour: 'numeric',
    minute: '2-digit',
  });
}

function WallCommentsSection({
  comments,
  isLoading,
  currentPeerId,
  draft,
  isSubmitting,
  onDraftChange,
  onSubmit,
  onDelete,
}: {
  comments: Comment[];
  isLoading: boolean;
  currentPeerId?: string;
  draft: string;
  isSubmitting: boolean;
  onDraftChange: (value: string) => void;
  onSubmit: () => void;
  onDelete: (commentId: string) => void;
}) {
  return (
    <div
      className="px-5 py-4 border-t space-y-4"
      style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
    >
      <div className="flex gap-3">
        <textarea
          value={draft}
          onChange={(event) => onDraftChange(event.target.value)}
          rows={2}
          placeholder="Add a comment..."
          className="flex-1 resize-none rounded-lg px-3 py-2 text-sm"
          style={{
            background: 'hsl(var(--harbor-surface-1))',
            border: '1px solid hsl(var(--harbor-border-subtle))',
            color: 'hsl(var(--harbor-text-primary))',
            outline: 'none',
          }}
        />
        <button
          type="button"
          disabled={isSubmitting || !draft.trim()}
          onClick={onSubmit}
          className="self-end px-4 py-2 rounded-lg text-sm font-medium disabled:opacity-50 disabled:cursor-not-allowed"
          style={{
            background:
              'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
            color: 'white',
          }}
        >
          {isSubmitting ? 'Posting...' : 'Comment'}
        </button>
      </div>

      {isLoading ? (
        <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
          Loading comments...
        </p>
      ) : comments.length === 0 ? (
        <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
          No comments yet.
        </p>
      ) : (
        <div className="space-y-3">
          {comments.map((comment) => (
            <div key={comment.commentId} className="flex gap-3">
              <div
                className="w-8 h-8 rounded-full flex items-center justify-center text-xs font-semibold text-white flex-shrink-0"
                style={{
                  background:
                    'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
                }}
              >
                {comment.authorName
                  .split(' ')
                  .map((part) => part[0])
                  .join('')
                  .toUpperCase()
                  .slice(0, 2) || '??'}
              </div>
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2 flex-wrap">
                  <span
                    className="text-sm font-medium"
                    style={{ color: 'hsl(var(--harbor-text-primary))' }}
                  >
                    {comment.authorName}
                  </span>
                  <span className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                    {formatCommentDate(comment.createdAt)}
                  </span>
                  {comment.authorPeerId === currentPeerId && (
                    <button
                      type="button"
                      onClick={() => onDelete(comment.commentId)}
                      className="text-xs hover:underline"
                      style={{ color: 'hsl(var(--harbor-error))' }}
                    >
                      Delete
                    </button>
                  )}
                </div>
                <p
                  className="text-sm whitespace-pre-wrap break-words mt-1"
                  style={{ color: 'hsl(var(--harbor-text-secondary))' }}
                >
                  {comment.content}
                </p>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export function WallPage() {
  const { state } = useIdentityStore();
  const {
    posts,
    isLoading,
    isSyncingRelay,
    lastSyncAt,
    syncError,
    syncStatus,
    loadPosts,
    createPost,
    updatePost,
    deletePost,
    likePost,
    commentsByPost,
    expandedComments,
    loadingComments,
    toggleComments,
    addComment,
    deleteComment,
    editingPostId,
    setEditingPost,
  } = useWallStore();
  const { defaultVisibility, socialView, setSocialView } = useSettingsStore();
  const [newPost, setNewPost] = useState('');
  const [isComposing, setIsComposing] = useState(false);
  const [selectedContentType, setSelectedContentType] = useState<WallContentType>('post');
  const [selectedVisibility, setSelectedVisibility] = useState<PostVisibility>(defaultVisibility);
  const [previewPerspective, setPreviewPerspective] = useState<WallPreviewPerspective>('guest');
  const [previewPosts, setPreviewPosts] = useState<FeedItem[]>([]);
  const [visibilityStats, setVisibilityStats] = useState<WallVisibilityStats | null>(null);
  const [isPreviewLoading, setIsPreviewLoading] = useState(false);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [shareAction, setShareAction] = useState<ShareAction | null>(null);
  const [showPreview, setShowPreview] = useState(false);
  const [pendingMedia, setPendingMedia] = useState<
    { type: 'image' | 'video' | 'audio'; url: string; name: string; file: File }[]
  >([]);
  const [showPostMenu, setShowPostMenu] = useState<string | null>(null);
  const [editContent, setEditContent] = useState('');
  const [commentDrafts, setCommentDrafts] = useState<Record<string, string>>({});
  const [submittingComments, setSubmittingComments] = useState<Set<string>>(new Set());
  const fileInputRef = useRef<HTMLInputElement>(null);
  const mediaTypeRef = useRef<'image' | 'video' | 'audio'>('image');

  const identity = state.status === 'unlocked' ? state.identity : null;

  // Get content type config
  const currentTypeConfig = CONTENT_TYPES.find((c) => c.type === selectedContentType)!;
  const charLimit = currentTypeConfig.charLimit;

  // Filter posts by selected content type
  const filteredPosts = useMemo(() => {
    if (socialView === 'posts') return posts;
    const type = socialView === 'images' ? 'image' : socialView === 'videos' ? 'video' : 'audio';
    return posts.filter((post) => post.contentType === type);
  }, [posts, socialView]);

  // Load posts from SQLite on mount
  useEffect(() => {
    if (identity) {
      loadPosts();
    }
  }, [identity, loadPosts]);

  // Load the production backend wall preview and visibility counts.
  useEffect(() => {
    if (!identity) {
      setPreviewPosts([]);
      setVisibilityStats(null);
      return;
    }

    let cancelled = false;
    setIsPreviewLoading(true);
    setPreviewError(null);

    Promise.all([
      feedService.getWallPreview(previewPerspective, 20),
      feedService.getWallVisibilityStats(),
    ])
      .then(([preview, stats]) => {
        if (cancelled) return;
        setPreviewPosts(preview);
        setVisibilityStats(stats);
      })
      .catch((err) => {
        if (cancelled) return;
        log.error('Failed to load wall preview', err);
        setPreviewError('Could not load wall preview from the local backend.');
      })
      .finally(() => {
        if (!cancelled) {
          setIsPreviewLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [identity, previewPerspective, posts]);

  // Keep the composer aligned to the persisted default until the author starts a draft.
  useEffect(() => {
    if (!isComposing && !newPost && pendingMedia.length === 0) {
      setSelectedVisibility(defaultVisibility);
    }
  }, [defaultVisibility, isComposing, newPost, pendingMedia.length]);

  const formatDate = (date: Date) => {
    const now = new Date();
    const diff = now.getTime() - date.getTime();
    const hours = Math.floor(diff / 3600000);
    const days = Math.floor(diff / 86400000);

    if (hours < 1) return 'Just now';
    if (hours < 24) return `${hours}h ago`;
    if (days < 7) return `${days}d ago`;
    return date.toLocaleDateString('en-US', {
      month: 'short',
      day: 'numeric',
      year: date.getFullYear() !== now.getFullYear() ? 'numeric' : undefined,
    });
  };

  const getInitials = (name: string) => {
    return name
      .split(' ')
      .map((n) => n[0])
      .join('')
      .toUpperCase()
      .slice(0, 2);
  };

  const handlePost = async () => {
    if (!newPost.trim() && pendingMedia.length === 0) return;

    // Enforce character limit for thoughts
    if (charLimit && newPost.length > charLimit) {
      toast.error(`Thoughts must be ${charLimit} characters or less`);
      return;
    }

    try {
      await createPost(
        newPost.trim(),
        selectedContentType,
        pendingMedia.length > 0 ? pendingMedia : undefined,
        selectedVisibility,
      );
      setNewPost('');
      setPendingMedia([]);
      setIsComposing(false);
      setSelectedContentType('post');
      setSelectedVisibility(defaultVisibility);
      toast.success(`${getContentTypeLabel(selectedContentType)} published!`);
    } catch (err) {
      log.error('Failed to create post', err);
      toast.error('Failed to publish post');
    }
  };

  const handleLike = async (postId: string) => {
    try {
      await likePost(postId);
    } catch (err) {
      log.error('Failed to update reaction', err);
      toast.error('Could not update reaction');
    }
  };

  const handleAddMedia = (type: 'image' | 'video' | 'audio') => {
    mediaTypeRef.current = type;
    if (fileInputRef.current) {
      fileInputRef.current.accept =
        type === 'image' ? 'image/*' : type === 'video' ? 'video/*' : 'audio/*';
      fileInputRef.current.click();
    }
  };

  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    // Check file size (max 10MB)
    if (file.size > 10 * 1024 * 1024) {
      toast.error('File size must be less than 10MB');
      return;
    }

    // Create object URL for preview
    const url = URL.createObjectURL(file);
    setPendingMedia([
      ...pendingMedia,
      {
        type: mediaTypeRef.current,
        url,
        name: file.name,
        file,
      },
    ]);
    toast.success(
      `${mediaTypeRef.current === 'image' ? 'Image' : mediaTypeRef.current === 'video' ? 'Video' : 'Audio'} added!`,
    );

    // Auto-select the matching content type if adding media
    if (mediaTypeRef.current === 'image' && selectedContentType !== 'image') {
      setSelectedContentType('image');
    } else if (mediaTypeRef.current === 'video' && selectedContentType !== 'video') {
      setSelectedContentType('video');
    } else if (mediaTypeRef.current === 'audio' && selectedContentType !== 'audio') {
      setSelectedContentType('audio');
    }

    // Reset file input
    if (fileInputRef.current) {
      fileInputRef.current.value = '';
    }
  };

  const handleRemoveMedia = (index: number) => {
    const media = pendingMedia[index];
    URL.revokeObjectURL(media.url);
    setPendingMedia(pendingMedia.filter((_, i) => i !== index));
  };

  const handleShare = async (postId: string) => {
    const post = posts.find((p) => p.postId === postId);
    if (!post || !identity) return;

    try {
      await navigator.clipboard.writeText(`harbor://post/${identity.peerId}/${post.postId}`);
      toast.success(
        post.visibility === 'public'
          ? 'Public post reference copied'
          : 'Contacts-only post reference copied. Only contacts with WallRead should receive it.',
      );
    } catch (err) {
      log.error('Failed to copy post reference', err);
      toast.error('Could not copy post reference');
    }
  };

  const generatePublicRssXml = async () => {
    if (!identity) {
      throw new Error('Identity must be unlocked to generate RSS');
    }
    return feedService.generateRssFeed(buildRssConfig(identity));
  };

  const handleCopyRssXml = async () => {
    setShareAction('rss-copy');
    try {
      const rssXml = await generatePublicRssXml();
      await navigator.clipboard.writeText(rssXml);
      toast.success('Public RSS XML copied. Contacts-only posts are excluded by the backend.');
    } catch (err) {
      log.error('Failed to copy RSS XML', err);
      toast.error('Could not copy RSS XML');
    } finally {
      setShareAction(null);
    }
  };

  const handleExportRssXml = async () => {
    if (!identity) return;

    setShareAction('rss-export');
    try {
      const rssXml = await generatePublicRssXml();
      const filename = buildRssFilename(identity.displayName);
      try {
        const savedPath = await invoke<string>('save_to_downloads', {
          filename,
          content: rssXml,
        });
        toast.success(`Public RSS XML saved to ${savedPath}`);
      } catch (saveErr) {
        log.warn('Tauri save_to_downloads failed, falling back to browser download', saveErr);
        downloadTextFile(filename, rssXml);
        toast.success('Public RSS XML downloaded locally');
      }
    } catch (err) {
      log.error('Failed to export RSS XML', err);
      toast.error('Could not export RSS XML');
    } finally {
      setShareAction(null);
    }
  };

  const handleCopyFeedUri = async () => {
    setShareAction('feed-link');
    try {
      const feedUri = await feedService.getRssFeedUrl();
      await navigator.clipboard.writeText(feedUri);
      toast.success('Harbor public feed URI copied. RSS XML is generated locally, not hosted.');
    } catch (err) {
      log.error('Failed to copy public feed URI', err);
      toast.error('Could not copy public feed URI');
    } finally {
      setShareAction(null);
    }
  };

  const handleCopyContactInvite = async () => {
    setShareAction('contact-link');
    try {
      const contactString = await getShareableContactString();
      await navigator.clipboard.writeText(contactString);
      toast.success('Contact invite copied. It contains public keys and reachable addresses only.');
    } catch (err) {
      log.error('Failed to copy contact invite', err);
      toast.error('Could not copy contact invite. Start networking or connect to a relay first.');
    } finally {
      setShareAction(null);
    }
  };

  const handleDeletePost = async (postId: string) => {
    try {
      await deletePost(postId);
      setShowPostMenu(null);
      toast.success('Post deleted');
    } catch (err) {
      console.error('Failed to delete post:', err);
      toast.error('Failed to delete post');
    }
  };

  const handleStartEdit = (postId: string, content: string) => {
    setEditContent(content);
    setEditingPost(postId);
    setShowPostMenu(null);
  };

  const handleCancelEdit = () => {
    setEditContent('');
    setEditingPost(null);
  };

  const handleSaveEdit = async (postId: string) => {
    if (!editContent.trim()) {
      toast.error('Post cannot be empty');
      return;
    }

    try {
      await updatePost(postId, editContent.trim());
      setEditContent('');
      toast.success('Post updated!');
    } catch {
      toast.error('Failed to update post');
    }
  };

  const handleSubmitComment = async (postId: string) => {
    const content = commentDrafts[postId]?.trim() ?? '';
    if (!content) return;

    setSubmittingComments((current) => new Set(current).add(postId));
    try {
      await addComment(postId, content);
      setCommentDrafts((current) => ({ ...current, [postId]: '' }));
      toast.success('Comment added');
    } catch (err) {
      log.error('Failed to add comment', err);
      toast.error('Could not add comment');
    } finally {
      setSubmittingComments((current) => {
        const next = new Set(current);
        next.delete(postId);
        return next;
      });
    }
  };

  const handleDeleteComment = async (postId: string, commentId: string) => {
    try {
      await deleteComment(postId, commentId);
      toast.success('Comment deleted');
    } catch (err) {
      log.error('Failed to delete comment', err);
      toast.error('Could not delete comment');
    }
  };

  return (
    <div className="h-full flex flex-col" style={{ background: 'hsl(var(--harbor-bg-primary))' }}>
      <input
        ref={fileInputRef}
        type="file"
        accept="image/*"
        onChange={handleFileChange}
        className="hidden"
      />

      {/* Header */}
      <header
        className="px-6 py-4 border-b flex-shrink-0"
        style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
      >
        <div className="max-w-3xl mx-auto">
          <div
            className="mb-4 h-36 rounded-xl overflow-hidden relative flex items-center px-6"
            style={{ background: 'hsl(var(--harbor-bg-elevated))' }}
          >
            <img src="/harbor.svg" alt="" className="absolute right-5 w-32 h-32 opacity-80" />
            <div className="relative z-10">
              <p
                className="text-xs uppercase tracking-[0.2em] font-semibold"
                style={{ color: 'hsl(var(--harbor-primary))' }}
              >
                Your space
              </p>
              <p className="text-xl font-bold" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
                Share what matters to you.
              </p>
            </div>
          </div>
          <h1 className="text-2xl font-bold" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
            Wall
          </h1>
          <p className="text-sm mt-1" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
            Your personal space for thoughts and creations
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
            {isSyncingRelay
              ? 'Syncing to relay… local posts are already saved.'
              : lastSyncAt
                ? `Last relay sync ${formatDate(new Date(lastSyncAt * 1000))}${syncError ? ' · Partial sync failure, retry by posting or manual sync.' : ''}`
                : 'Not synced to relay yet. Local posts remain available.'}
          </p>
        </div>
      </header>

      <div className="flex-1 overflow-y-auto p-6">
        <div className="max-w-3xl mx-auto space-y-6">
          {/* Composer - blog style */}
          <div
            className="rounded-lg overflow-hidden"
            style={{
              background: 'hsl(var(--harbor-bg-elevated))',
              border: '1px solid hsl(var(--harbor-border-subtle))',
            }}
          >
            {/* Composer header */}
            <div
              className="px-5 py-3 border-b"
              style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
            >
              <div className="flex items-center gap-3">
                {identity && (
                  <div
                    className="w-10 h-10 rounded-full flex items-center justify-center text-sm font-semibold text-white"
                    style={{
                      background:
                        'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
                    }}
                  >
                    {getInitials(identity.displayName)}
                  </div>
                )}
                <div>
                  <p
                    className="font-medium text-sm"
                    style={{ color: 'hsl(var(--harbor-text-primary))' }}
                  >
                    {identity?.displayName || 'You'}
                  </p>
                  <p className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                    Creating a new {currentTypeConfig.label.toLowerCase()}
                  </p>
                </div>
              </div>
            </div>

            {/* Content type selector pills */}
            <div
              className="px-5 py-3 border-b flex items-center gap-2 overflow-x-auto"
              style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
            >
              <span
                className="text-xs font-medium flex-shrink-0"
                style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
              >
                Type:
              </span>
              {CONTENT_TYPES.map((ct) => {
                const isSelected = selectedContentType === ct.type;
                return (
                  <button
                    key={ct.type}
                    onClick={() => setSelectedContentType(ct.type)}
                    className="flex items-center gap-1.5 px-3 py-1.5 rounded-full text-xs font-medium transition-all duration-200 flex-shrink-0"
                    style={{
                      background: isSelected
                        ? 'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))'
                        : 'hsl(var(--harbor-surface-1))',
                      color: isSelected ? 'white' : 'hsl(var(--harbor-text-secondary))',
                      boxShadow: isSelected ? '0 2px 8px hsl(var(--harbor-primary) / 0.3)' : 'none',
                    }}
                  >
                    {ct.icon}
                    {ct.label}
                  </button>
                );
              })}
            </div>

            {/* Visibility is a compact publish control in the composer footer. */}
            <div className="hidden" style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}>
              <span
                className="text-xs font-medium flex-shrink-0"
                style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
              >
                Visibility:
              </span>
              <div className="flex flex-wrap gap-2">
                {VISIBILITY_OPTIONS.map((option) => {
                  const isSelected = selectedVisibility === option.visibility;
                  return (
                    <button
                      key={option.visibility}
                      type="button"
                      aria-pressed={isSelected}
                      onClick={() => {
                        setSelectedVisibility(option.visibility);
                        setIsComposing(true);
                      }}
                      className="px-3 py-2 rounded-lg text-left transition-all duration-200"
                      style={{
                        background: isSelected
                          ? 'hsl(var(--harbor-primary) / 0.15)'
                          : 'hsl(var(--harbor-surface-1))',
                        border: isSelected
                          ? '1px solid hsl(var(--harbor-primary) / 0.45)'
                          : '1px solid hsl(var(--harbor-border-subtle))',
                        color: isSelected
                          ? 'hsl(var(--harbor-primary))'
                          : 'hsl(var(--harbor-text-secondary))',
                      }}
                    >
                      <span className="block text-xs font-semibold">{option.label}</span>
                      <span
                        className="block text-[11px] mt-0.5"
                        style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                      >
                        {option.description}
                      </span>
                    </button>
                  );
                })}
              </div>
            </div>

            {/* Composer body */}
            <div className="p-5">
              <textarea
                placeholder={currentTypeConfig.placeholder}
                value={newPost}
                onChange={(e) => {
                  const val = e.target.value;
                  // Enforce char limit for thought type
                  if (charLimit && val.length > charLimit) {
                    return;
                  }
                  setNewPost(val);
                  setIsComposing(true);
                }}
                onFocus={() => setIsComposing(true)}
                rows={selectedContentType === 'thought' ? 2 : isComposing ? 6 : 3}
                className="w-full resize-none leading-relaxed"
                style={{
                  background: 'transparent',
                  border: 'none',
                  outline: 'none',
                  color: 'hsl(var(--harbor-text-primary))',
                  fontSize: selectedContentType === 'thought' ? '1.125rem' : '1rem',
                }}
              />

              {/* Character counter for thoughts */}
              {selectedContentType === 'thought' && (
                <div
                  className="text-right mt-1 text-xs"
                  style={{
                    color:
                      newPost.length > (charLimit ?? 280) * 0.9
                        ? 'hsl(var(--harbor-warning))'
                        : 'hsl(var(--harbor-text-tertiary))',
                  }}
                >
                  {newPost.length}/{charLimit}
                </div>
              )}

              {/* Pending media preview */}
              {pendingMedia.length > 0 && (
                <div className="mt-4 flex flex-wrap gap-3">
                  {pendingMedia.map((media, index) => (
                    <div
                      key={index}
                      className="relative rounded-lg overflow-hidden"
                      style={{ background: 'hsl(var(--harbor-surface-1))' }}
                    >
                      {media.type === 'image' ? (
                        <img src={media.url} alt={media.name} className="w-32 h-32 object-cover" />
                      ) : media.type === 'video' ? (
                        <video src={media.url} className="w-32 h-32 object-cover" />
                      ) : (
                        <div className="w-48 h-32 flex items-center justify-center p-3">
                          <audio src={media.url} controls className="w-full" />
                        </div>
                      )}
                      <button
                        onClick={() => handleRemoveMedia(index)}
                        className="absolute top-1 right-1 w-6 h-6 rounded-full flex items-center justify-center"
                        style={{
                          background: 'hsl(var(--harbor-error))',
                          color: 'white',
                        }}
                      >
                        <svg
                          className="w-4 h-4"
                          fill="none"
                          viewBox="0 0 24 24"
                          stroke="currentColor"
                        >
                          <path
                            strokeLinecap="round"
                            strokeLinejoin="round"
                            strokeWidth={2}
                            d="M6 18L18 6M6 6l12 12"
                          />
                        </svg>
                      </button>
                      <div
                        className="absolute bottom-0 left-0 right-0 px-2 py-1 text-xs truncate"
                        style={{
                          background: 'rgba(0,0,0,0.6)',
                          color: 'white',
                        }}
                      >
                        {media.name}
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>

            {/* Composer footer */}
            <div
              className="px-5 py-3 border-t flex items-center justify-between"
              style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
            >
              <div className="flex items-center gap-1">
                <button
                  onClick={() => handleAddMedia('image')}
                  className="p-2 rounded-lg transition-colors duration-200 hover:bg-white/5"
                  style={{ color: 'hsl(var(--harbor-text-secondary))' }}
                  title="Add image"
                >
                  <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={1.5}
                      d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z"
                    />
                  </svg>
                </button>
                <button
                  onClick={() => handleAddMedia('video')}
                  className="p-2 rounded-lg transition-colors duration-200 hover:bg-white/5"
                  style={{ color: 'hsl(var(--harbor-text-secondary))' }}
                  title="Add video"
                >
                  <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={1.5}
                      d="M15 10l4.553-2.276A1 1 0 0121 8.618v6.764a1 1 0 01-1.447.894L15 14M5 18h8a2 2 0 002-2V8a2 2 0 00-2-2H5a2 2 0 00-2 2v8a2 2 0 002 2z"
                    />
                  </svg>
                </button>
                <button
                  onClick={() => handleAddMedia('audio')}
                  className="p-2 rounded-lg transition-colors duration-200 hover:bg-white/5"
                  style={{ color: 'hsl(var(--harbor-text-secondary))' }}
                  title="Add audio"
                >
                  <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={1.5}
                      d="M9 19V6l12-2v13M9 19c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zm12-2c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2z"
                    />
                  </svg>
                </button>
                <button
                  type="button"
                  aria-pressed={selectedVisibility === 'public'}
                  onClick={() =>
                    setSelectedVisibility(selectedVisibility === 'public' ? 'contacts' : 'public')
                  }
                  className="ml-2 px-3 py-2 rounded-lg text-xs font-semibold"
                  title="Public posts appear in public wall previews and exported RSS. Contacts posts are shared only with contacts who have wall access."
                  style={{
                    background: 'hsl(var(--harbor-surface-2))',
                    color: 'hsl(var(--harbor-text-primary))',
                    border: '1px solid hsl(var(--harbor-border-subtle))',
                  }}
                >
                  {selectedVisibility === 'public' ? 'Public' : 'Contacts'}
                </button>
              </div>

              <div className="flex items-center gap-2">
                {isComposing && (
                  <button
                    onClick={() => {
                      setIsComposing(false);
                      setNewPost('');
                      setSelectedContentType('post');
                      setSelectedVisibility(defaultVisibility);
                      pendingMedia.forEach((m) => URL.revokeObjectURL(m.url));
                      setPendingMedia([]);
                    }}
                    className="px-4 py-2 rounded-lg text-sm font-medium transition-colors duration-200"
                    style={{ color: 'hsl(var(--harbor-text-secondary))' }}
                  >
                    Cancel
                  </button>
                )}
                <button
                  onClick={handlePost}
                  disabled={!newPost.trim() && pendingMedia.length === 0}
                  className="px-5 py-2 rounded-lg text-sm font-medium transition-all duration-200 disabled:opacity-50 disabled:cursor-not-allowed"
                  style={{
                    background:
                      newPost.trim() || pendingMedia.length > 0
                        ? 'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))'
                        : 'hsl(var(--harbor-surface-2))',
                    color:
                      newPost.trim() || pendingMedia.length > 0
                        ? 'white'
                        : 'hsl(var(--harbor-text-tertiary))',
                    boxShadow:
                      newPost.trim() || pendingMedia.length > 0
                        ? '0 4px 12px hsl(var(--harbor-primary) / 0.3)'
                        : 'none',
                  }}
                >
                  Publish
                </button>
              </div>
            </div>
          </div>

          <button
            type="button"
            onClick={() => setShowPreview((value) => !value)}
            className="px-4 py-2 rounded-lg text-sm font-semibold"
            style={{
              background: 'hsl(var(--harbor-surface-2))',
              color: 'hsl(var(--harbor-text-primary))',
              border: '1px solid hsl(var(--harbor-border-subtle))',
            }}
          >
            {showPreview ? 'Close wall preview' : 'Preview and share wall'}
          </button>
          {/* Preview, RSS, and sharing surfaces */}
          <section
            className={`${showPreview ? '' : 'hidden'} rounded-lg overflow-hidden`}
            aria-labelledby="wall-preview-share-heading"
            style={{
              background: 'hsl(var(--harbor-bg-elevated))',
              border: '1px solid hsl(var(--harbor-border-subtle))',
            }}
          >
            <div
              className="px-5 py-4 border-b"
              style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
            >
              <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
                <div>
                  <h2
                    id="wall-preview-share-heading"
                    className="text-base font-semibold"
                    style={{ color: 'hsl(var(--harbor-text-primary))' }}
                  >
                    Preview and share your wall
                  </h2>
                  <p
                    className="text-sm mt-1 max-w-2xl"
                    style={{ color: 'hsl(var(--harbor-text-secondary))' }}
                  >
                    Preview modes and RSS are loaded through Harbor backend commands. Public RSS XML
                    includes only posts marked Public; contacts-only posts are never copied or
                    exported through the RSS actions.
                  </p>
                </div>
                {visibilityStats && (
                  <div className="grid grid-cols-3 gap-2 text-center min-w-[15rem]">
                    <div
                      className="rounded-lg px-3 py-2"
                      style={{ background: 'hsl(var(--harbor-surface-1))' }}
                    >
                      <p
                        className="text-lg font-semibold"
                        style={{ color: 'hsl(var(--harbor-text-primary))' }}
                      >
                        {visibilityStats.totalPosts}
                      </p>
                      <p
                        className="text-[11px] uppercase tracking-wide"
                        style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                      >
                        Total
                      </p>
                    </div>
                    <div
                      className="rounded-lg px-3 py-2"
                      style={{ background: 'hsl(var(--harbor-surface-1))' }}
                    >
                      <p
                        className="text-lg font-semibold"
                        style={{ color: 'hsl(var(--harbor-success))' }}
                      >
                        {visibilityStats.publicPosts}
                      </p>
                      <p
                        className="text-[11px] uppercase tracking-wide"
                        style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                      >
                        Public
                      </p>
                    </div>
                    <div
                      className="rounded-lg px-3 py-2"
                      style={{ background: 'hsl(var(--harbor-surface-1))' }}
                    >
                      <p
                        className="text-lg font-semibold"
                        style={{ color: 'hsl(var(--harbor-primary))' }}
                      >
                        {visibilityStats.contactsOnlyPosts}
                      </p>
                      <p
                        className="text-[11px] uppercase tracking-wide"
                        style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                      >
                        Contacts
                      </p>
                    </div>
                  </div>
                )}
              </div>
            </div>

            <div className="p-5 space-y-5">
              <div className="flex flex-col gap-3">
                <div className="flex flex-wrap gap-2" role="tablist" aria-label="Wall preview mode">
                  {PREVIEW_OPTIONS.map((option) => {
                    const isSelected = previewPerspective === option.perspective;
                    return (
                      <button
                        key={option.perspective}
                        type="button"
                        role="tab"
                        aria-selected={isSelected}
                        onClick={() => setPreviewPerspective(option.perspective)}
                        className="px-3 py-2 rounded-lg text-left transition-all duration-200"
                        style={{
                          background: isSelected
                            ? 'hsl(var(--harbor-primary) / 0.15)'
                            : 'hsl(var(--harbor-surface-1))',
                          border: isSelected
                            ? '1px solid hsl(var(--harbor-primary) / 0.45)'
                            : '1px solid hsl(var(--harbor-border-subtle))',
                          color: isSelected
                            ? 'hsl(var(--harbor-primary))'
                            : 'hsl(var(--harbor-text-secondary))',
                        }}
                      >
                        <span className="block text-xs font-semibold">{option.label}</span>
                        <span
                          className="block text-[11px] mt-0.5 max-w-[13rem]"
                          style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                        >
                          {option.summary}
                        </span>
                      </button>
                    );
                  })}
                </div>

                <div
                  className="rounded-lg px-4 py-3 text-sm"
                  style={{
                    background: 'hsl(var(--harbor-surface-1))',
                    color: 'hsl(var(--harbor-text-secondary))',
                    border: '1px solid hsl(var(--harbor-border-subtle))',
                  }}
                >
                  {getPreviewExplanation(previewPerspective, visibilityStats)}
                </div>
              </div>

              <div
                className="rounded-lg overflow-hidden"
                data-testid="wall-preview-panel"
                style={{ border: '1px solid hsl(var(--harbor-border-subtle))' }}
              >
                <div
                  className="px-4 py-2.5 border-b flex items-center justify-between"
                  style={{
                    borderColor: 'hsl(var(--harbor-border-subtle))',
                    background: 'hsl(var(--harbor-surface-1))',
                  }}
                >
                  <p
                    className="text-xs font-semibold uppercase tracking-wide"
                    style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                  >
                    {PREVIEW_OPTIONS.find((option) => option.perspective === previewPerspective)
                      ?.label || 'Preview'}
                  </p>
                  <span className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                    {previewPosts.length} visible
                  </span>
                </div>

                {isPreviewLoading ? (
                  <div className="px-4 py-8 text-center">
                    <div
                      className="w-6 h-6 border-2 border-t-transparent rounded-full animate-spin mx-auto mb-3"
                      style={{
                        borderColor: 'hsl(var(--harbor-primary))',
                        borderTopColor: 'transparent',
                      }}
                    />
                    <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                      Loading backend preview...
                    </p>
                  </div>
                ) : previewError ? (
                  <div className="px-4 py-6 text-sm" style={{ color: 'hsl(var(--harbor-error))' }}>
                    {previewError}
                  </div>
                ) : previewPosts.length === 0 ? (
                  <div
                    className="px-4 py-6 text-sm text-center"
                    style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                  >
                    No posts are visible from this perspective.
                  </div>
                ) : (
                  <div
                    className="divide-y"
                    style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
                  >
                    {previewPosts.map((post) => {
                      const contentType = parseWallContentType(post.contentType);
                      return (
                        <article
                          key={post.postId}
                          className="px-4 py-3"
                          data-testid="wall-preview-post"
                        >
                          <div className="flex items-start justify-between gap-3">
                            <div className="min-w-0 flex-1">
                              <div className="flex flex-wrap items-center gap-2 mb-1">
                                <span
                                  className="text-xs font-medium"
                                  style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                                >
                                  {getContentTypeLabel(contentType)}
                                </span>
                                <VisibilityBadge visibility={post.visibility} compact />
                              </div>
                              <p
                                className="text-sm leading-relaxed whitespace-pre-wrap"
                                style={{ color: 'hsl(var(--harbor-text-primary))' }}
                              >
                                {post.contentText || 'Media post'}
                              </p>
                            </div>
                            <time
                              className="text-xs flex-shrink-0"
                              style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                            >
                              {formatDate(new Date(post.createdAt * 1000))}
                            </time>
                          </div>
                        </article>
                      );
                    })}
                  </div>
                )}
              </div>

              <div className="grid gap-3 md:grid-cols-2">
                <div
                  className="rounded-lg p-4 space-y-3"
                  style={{
                    background: 'hsl(var(--harbor-surface-1))',
                    border: '1px solid hsl(var(--harbor-border-subtle))',
                  }}
                >
                  <div>
                    <h3
                      className="text-sm font-semibold"
                      style={{ color: 'hsl(var(--harbor-text-primary))' }}
                    >
                      Public RSS XML
                    </h3>
                    <p
                      className="text-xs mt-1"
                      style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                    >
                      Generated locally from public posts. Harbor does not host this as an HTTP URL.
                    </p>
                  </div>
                  <div className="flex flex-wrap gap-2">
                    <button
                      type="button"
                      onClick={handleCopyRssXml}
                      disabled={!identity || shareAction !== null}
                      className="px-3 py-2 rounded-lg text-xs font-medium transition-all disabled:opacity-50"
                      style={{
                        background: 'hsl(var(--harbor-primary) / 0.15)',
                        color: 'hsl(var(--harbor-primary))',
                        border: '1px solid hsl(var(--harbor-primary) / 0.35)',
                      }}
                    >
                      {shareAction === 'rss-copy' ? 'Copying...' : 'Copy RSS XML'}
                    </button>
                    <button
                      type="button"
                      onClick={handleExportRssXml}
                      disabled={!identity || shareAction !== null}
                      className="px-3 py-2 rounded-lg text-xs font-medium transition-all disabled:opacity-50"
                      style={{
                        background: 'hsl(var(--harbor-surface-2))',
                        color: 'hsl(var(--harbor-text-secondary))',
                        border: '1px solid hsl(var(--harbor-border-subtle))',
                      }}
                    >
                      {shareAction === 'rss-export' ? 'Exporting...' : 'Export .xml'}
                    </button>
                  </div>
                </div>

                <div
                  className="rounded-lg p-4 space-y-3"
                  style={{
                    background: 'hsl(var(--harbor-surface-1))',
                    border: '1px solid hsl(var(--harbor-border-subtle))',
                  }}
                >
                  <div>
                    <h3
                      className="text-sm font-semibold"
                      style={{ color: 'hsl(var(--harbor-text-primary))' }}
                    >
                      Shareable Harbor links
                    </h3>
                    <p
                      className="text-xs mt-1"
                      style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                    >
                      Feed URIs identify your public wall. Contact invites include only public keys
                      and reachable addresses, never private keys or backups.
                    </p>
                  </div>
                  <div className="flex flex-wrap gap-2">
                    <button
                      type="button"
                      onClick={handleCopyFeedUri}
                      disabled={!identity || shareAction !== null}
                      className="px-3 py-2 rounded-lg text-xs font-medium transition-all disabled:opacity-50"
                      style={{
                        background: 'hsl(var(--harbor-surface-2))',
                        color: 'hsl(var(--harbor-text-secondary))',
                        border: '1px solid hsl(var(--harbor-border-subtle))',
                      }}
                    >
                      {shareAction === 'feed-link' ? 'Copying...' : 'Copy public feed URI'}
                    </button>
                    <button
                      type="button"
                      onClick={handleCopyContactInvite}
                      disabled={!identity || shareAction !== null}
                      className="px-3 py-2 rounded-lg text-xs font-medium transition-all disabled:opacity-50"
                      style={{
                        background: 'hsl(var(--harbor-surface-2))',
                        color: 'hsl(var(--harbor-text-secondary))',
                        border: '1px solid hsl(var(--harbor-border-subtle))',
                      }}
                    >
                      {shareAction === 'contact-link' ? 'Copying...' : 'Copy contact invite'}
                    </button>
                  </div>
                </div>
              </div>
            </div>
          </section>

          {/* Distinct wall views */}
          <div
            className="grid grid-cols-4 border-b"
            style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
          >
            {FILTER_OPTIONS.map((opt) => {
              const isActive = socialView === `${opt.type}`;
              return (
                <button
                  key={opt.type}
                  onClick={() => setSocialView(opt.type)}
                  className="px-3 py-3 text-sm font-semibold transition-all duration-200"
                  style={{
                    background: isActive ? 'hsl(var(--harbor-primary) / 0.15)' : 'transparent',
                    color: isActive
                      ? 'hsl(var(--harbor-primary))'
                      : 'hsl(var(--harbor-text-secondary))',
                    borderBottom: isActive
                      ? '3px solid hsl(var(--harbor-primary))'
                      : '3px solid transparent',
                  }}
                >
                  {opt.label}
                </button>
              );
            })}
          </div>

          {/* Posts */}
          {isLoading ? (
            <div className="text-center py-16">
              <div
                className="w-10 h-10 border-2 border-t-transparent rounded-full animate-spin mx-auto mb-4"
                style={{ borderColor: 'hsl(var(--harbor-primary))', borderTopColor: 'transparent' }}
              />
              <p style={{ color: 'hsl(var(--harbor-text-secondary))' }}>Loading posts...</p>
            </div>
          ) : filteredPosts.length === 0 ? (
            <div className="text-center py-16">
              <div
                className="w-20 h-20 rounded-lg flex items-center justify-center mx-auto mb-4"
                style={{ background: 'hsl(var(--harbor-surface-1))' }}
              >
                <WallIcon
                  className="w-10 h-10"
                  style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                />
              </div>
              <h3
                className="text-lg font-semibold mb-2"
                style={{ color: 'hsl(var(--harbor-text-primary))' }}
              >
                {socialView === 'posts'
                  ? 'No posts yet'
                  : `No ${FILTER_OPTIONS.find((f) => f.type === socialView)?.label.toLowerCase() || 'posts'} yet`}
              </h3>
              <p
                className="text-sm max-w-xs mx-auto"
                style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
              >
                {socialView === 'posts'
                  ? 'Share your first post with your contacts. Your posts are stored locally and shared peer-to-peer.'
                  : 'Try creating one using the composer above, or switch to a different filter.'}
              </p>
            </div>
          ) : (
            filteredPosts.map((post) => (
              <article
                key={post.postId}
                className="rounded-lg overflow-hidden"
                style={{
                  background: 'hsl(var(--harbor-bg-elevated))',
                  border:
                    post.contentType === 'thought'
                      ? '1px solid hsl(var(--harbor-primary) / 0.2)'
                      : '1px solid hsl(var(--harbor-border-subtle))',
                }}
              >
                {/* Shared indicator */}
                {post.sharedFrom && (
                  <div
                    className="px-5 py-2.5 flex items-center gap-2 border-b"
                    style={{
                      borderColor: 'hsl(var(--harbor-border-subtle))',
                      background: 'hsl(var(--harbor-surface-1))',
                    }}
                  >
                    <svg
                      className="w-4 h-4"
                      fill="none"
                      viewBox="0 0 24 24"
                      stroke="currentColor"
                      style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                    >
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        strokeWidth={1.5}
                        d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12"
                      />
                    </svg>
                    <span
                      className="text-xs font-medium"
                      style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                    >
                      Shared from{' '}
                      <span style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                        {post.sharedFrom.authorName}
                      </span>
                    </span>
                  </div>
                )}

                {/* Post header */}
                <div
                  className="px-5 py-4 flex items-center justify-between border-b"
                  style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
                >
                  <div className="flex items-center gap-3">
                    {identity && (
                      <div
                        className="w-10 h-10 rounded-full flex items-center justify-center text-sm font-semibold text-white"
                        style={{
                          background:
                            'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
                        }}
                      >
                        {getInitials(identity.displayName)}
                      </div>
                    )}
                    <div>
                      <div className="flex items-center gap-2">
                        <p
                          className="font-semibold text-sm"
                          style={{ color: 'hsl(var(--harbor-text-primary))' }}
                        >
                          {identity?.displayName || 'You'}
                        </p>
                        {/* Content type badge */}
                        {post.contentType !== 'post' && (
                          <span
                            className="flex items-center gap-1 px-2 py-0.5 rounded-full text-xs"
                            style={{
                              background:
                                post.contentType === 'thought'
                                  ? 'hsl(var(--harbor-primary) / 0.1)'
                                  : post.contentType === 'image'
                                    ? 'hsl(var(--harbor-success) / 0.1)'
                                    : post.contentType === 'video'
                                      ? 'hsl(var(--harbor-accent) / 0.1)'
                                      : 'hsl(var(--harbor-warning) / 0.1)',
                              color:
                                post.contentType === 'thought'
                                  ? 'hsl(var(--harbor-primary))'
                                  : post.contentType === 'image'
                                    ? 'hsl(var(--harbor-success))'
                                    : post.contentType === 'video'
                                      ? 'hsl(var(--harbor-accent))'
                                      : 'hsl(var(--harbor-warning))',
                            }}
                          >
                            {getContentTypeIcon(post.contentType)}
                            {getContentTypeLabel(post.contentType)}
                          </span>
                        )}
                        <VisibilityBadge visibility={post.visibility} />
                      </div>
                      <p className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                        {formatDate(post.timestamp)}
                      </p>
                    </div>
                  </div>

                  <div className="relative">
                    <button
                      onClick={() =>
                        setShowPostMenu(showPostMenu === post.postId ? null : post.postId)
                      }
                      className="p-2 rounded-lg transition-colors duration-200"
                      style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                    >
                      <EllipsisIcon className="w-5 h-5" />
                    </button>

                    {/* Post menu dropdown */}
                    {showPostMenu === post.postId && (
                      <div
                        className="absolute right-0 top-full mt-1 w-40 rounded-lg overflow-hidden z-10"
                        style={{
                          background: 'hsl(var(--harbor-bg-elevated))',
                          border: '1px solid hsl(var(--harbor-border-subtle))',
                          boxShadow: '0 10px 40px rgba(0,0,0,0.3)',
                        }}
                      >
                        <button
                          onClick={() => handleStartEdit(post.postId, post.content)}
                          className="w-full px-4 py-2.5 text-left text-sm transition-colors hover:bg-white/5"
                          style={{ color: 'hsl(var(--harbor-text-primary))' }}
                        >
                          Edit post
                        </button>
                        <button
                          onClick={() => handleDeletePost(post.postId)}
                          className="w-full px-4 py-2.5 text-left text-sm transition-colors hover:bg-white/5"
                          style={{ color: 'hsl(var(--harbor-error))' }}
                        >
                          Delete post
                        </button>
                      </div>
                    )}
                  </div>
                </div>

                {/* Post content */}
                <div className={post.contentType === 'thought' ? 'px-5 py-4' : 'px-5 py-5'}>
                  {editingPostId === post.postId ? (
                    <div className="space-y-3">
                      <textarea
                        value={editContent}
                        onChange={(e) => setEditContent(e.target.value)}
                        rows={4}
                        className="w-full resize-none text-base leading-relaxed p-3 rounded-lg"
                        style={{
                          background: 'hsl(var(--harbor-surface-1))',
                          border: '1px solid hsl(var(--harbor-border-subtle))',
                          outline: 'none',
                          color: 'hsl(var(--harbor-text-primary))',
                        }}
                        autoFocus
                      />
                      <div className="flex items-center gap-2 justify-end">
                        <button
                          onClick={handleCancelEdit}
                          className="px-4 py-2 rounded-lg text-sm font-medium transition-colors duration-200"
                          style={{ color: 'hsl(var(--harbor-text-secondary))' }}
                        >
                          Cancel
                        </button>
                        <button
                          onClick={() => handleSaveEdit(post.postId)}
                          className="px-4 py-2 rounded-lg text-sm font-medium transition-all duration-200"
                          style={{
                            background:
                              'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
                            color: 'white',
                            boxShadow: '0 4px 12px hsl(var(--harbor-primary) / 0.3)',
                          }}
                        >
                          Save changes
                        </button>
                      </div>
                    </div>
                  ) : (
                    <>
                      {post.content && (
                        <p
                          className={`whitespace-pre-wrap leading-relaxed ${
                            post.contentType === 'thought' ? 'text-lg italic' : 'text-base'
                          }`}
                          style={{ color: 'hsl(var(--harbor-text-primary))' }}
                        >
                          {post.content}
                        </p>
                      )}

                      {/* Link preview card for first URL in post */}
                      {post.content &&
                        (() => {
                          const firstUrl = extractFirstUrl(post.content);
                          return firstUrl ? <LinkPreviewCard url={firstUrl} /> : null;
                        })()}

                      {/* Shared post embed */}
                      {post.sharedFrom && (
                        <div
                          className={`rounded-lg overflow-hidden${post.content ? ' mt-4' : ''}`}
                          style={{
                            background: 'hsl(var(--harbor-surface-1))',
                            border: '1px solid hsl(var(--harbor-border-subtle))',
                          }}
                        >
                          <div
                            className="px-4 py-3 flex items-center gap-3 border-b"
                            style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
                          >
                            <div
                              className="w-8 h-8 rounded-full flex items-center justify-center text-xs font-semibold text-white flex-shrink-0"
                              style={{ background: post.sharedFrom.avatarGradient }}
                            >
                              {post.sharedFrom.authorName
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
                                {post.sharedFrom.authorName}
                              </p>
                              <p
                                className="text-xs"
                                style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                              >
                                Original post
                              </p>
                            </div>
                          </div>
                          <div className="px-4 py-3">
                            <p
                              className="text-sm leading-relaxed whitespace-pre-wrap"
                              style={{ color: 'hsl(var(--harbor-text-secondary))' }}
                            >
                              {post.sharedFrom.originalContent}
                            </p>
                          </div>
                        </div>
                      )}
                    </>
                  )}

                  {/* Post media */}
                  {post.media && post.media.length > 0 && <PostMedia media={post.media} />}
                </div>

                {/* Post actions */}
                <div
                  className="px-5 py-3 flex items-center gap-6 border-t"
                  style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
                >
                  <button
                    onClick={() => handleLike(post.postId)}
                    className="flex items-center gap-2 transition-colors duration-200"
                    style={{
                      color: post.liked
                        ? 'hsl(var(--harbor-error))'
                        : 'hsl(var(--harbor-text-secondary))',
                    }}
                  >
                    <svg
                      className="w-5 h-5"
                      fill={post.liked ? 'currentColor' : 'none'}
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
                    <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        strokeWidth={1.5}
                        d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z"
                      />
                    </svg>
                    <span className="text-sm">{post.comments}</span>
                  </button>

                  <button
                    onClick={() => handleShare(post.postId)}
                    className="flex items-center gap-2 transition-colors duration-200 ml-auto"
                    style={{ color: 'hsl(var(--harbor-text-secondary))' }}
                  >
                    <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        strokeWidth={1.5}
                        d="M8.684 13.342C8.886 12.938 9 12.482 9 12c0-.482-.114-.938-.316-1.342m0 2.684a3 3 0 110-2.684m0 2.684l6.632 3.316m-6.632-6l6.632-3.316m0 0a3 3 0 105.367-2.684 3 3 0 00-5.367 2.684zm0 9.316a3 3 0 105.368 2.684 3 3 0 00-5.368-2.684z"
                      />
                    </svg>
                    <span className="text-sm">Share</span>
                  </button>
                </div>

                {expandedComments.has(post.postId) && (
                  <WallCommentsSection
                    comments={commentsByPost[post.postId] || []}
                    isLoading={loadingComments.has(post.postId)}
                    currentPeerId={identity?.peerId}
                    draft={commentDrafts[post.postId] || ''}
                    isSubmitting={submittingComments.has(post.postId)}
                    onDraftChange={(value) =>
                      setCommentDrafts((current) => ({ ...current, [post.postId]: value }))
                    }
                    onSubmit={() => handleSubmitComment(post.postId)}
                    onDelete={(commentId) => handleDeleteComment(post.postId, commentId)}
                  />
                )}
              </article>
            ))
          )}
        </div>
      </div>

      {/* Click outside to close menu */}
      {showPostMenu && <div className="fixed inset-0 z-0" onClick={() => setShowPostMenu(null)} />}
    </div>
  );
}
