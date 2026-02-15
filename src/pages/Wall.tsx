import { useState, useRef, useEffect, useMemo } from 'react';
import toast from 'react-hot-toast';
import { useIdentityStore, useWallStore } from '../stores';
import type { WallContentType } from '../stores';
import { WallIcon, EllipsisIcon } from '../components/icons';
import { PostMedia } from '../components/common/PostMedia';
import { LinkPreviewCard } from '../components/common/LinkPreviewCard';
import { getInitials, formatDate } from '../utils/formatting';
import { extractFirstUrl } from '../utils/urlDetection';
import { createLogger } from '../utils/logger';

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
      label: 'Thought',
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

/** Filter options including "All" */
const FILTER_OPTIONS: { type: WallContentType | 'all'; label: string }[] = [
  { type: 'all', label: 'All' },
  { type: 'post', label: 'Posts' },
  { type: 'thought', label: 'Thoughts' },
  { type: 'image', label: 'Images' },
  { type: 'video', label: 'Videos' },
  { type: 'audio', label: 'Audio' },
];

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

export function WallPage() {
  const { state } = useIdentityStore();
  const {
    posts,
    isLoading,
    loadPosts,
    createPost,
    updatePost,
    deletePost,
    likePost,
    editingPostId,
    setEditingPost,
  } = useWallStore();
  const [newPost, setNewPost] = useState('');
  const [isComposing, setIsComposing] = useState(false);
  const [selectedContentType, setSelectedContentType] = useState<WallContentType>('post');
  const [filterType, setFilterType] = useState<WallContentType | 'all'>('all');
  const [pendingMedia, setPendingMedia] = useState<
    { type: 'image' | 'video'; url: string; name: string; file: File }[]
  >([]);
  const [showPostMenu, setShowPostMenu] = useState<string | null>(null);
  const [editContent, setEditContent] = useState('');
  const fileInputRef = useRef<HTMLInputElement>(null);
  const mediaTypeRef = useRef<'image' | 'video'>('image');

  const identity = state.status === 'unlocked' ? state.identity : null;

  // Get content type config
  const currentTypeConfig = CONTENT_TYPES.find((c) => c.type === selectedContentType)!;
  const charLimit = currentTypeConfig.charLimit;

  // Filter posts by selected content type
  const filteredPosts = useMemo(() => {
    if (filterType === 'all') return posts;
    return posts.filter((post) => post.contentType === filterType);
  }, [posts, filterType]);

  // Load posts from SQLite on mount
  useEffect(() => {
    if (identity) {
      loadPosts().catch((err) => log.error('Failed to load posts', err));
    }
  }, [identity, loadPosts]);

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
      );
      setNewPost('');
      setPendingMedia([]);
      setIsComposing(false);
      setSelectedContentType('post');
      toast.success(`${getContentTypeLabel(selectedContentType)} published!`);
    } catch (err) {
      log.error('Failed to create post', err);
      toast.error('Failed to publish post');
    }
  };

  const handleLike = (postId: string) => {
    likePost(postId);
  };

  const handleAddMedia = (type: 'image' | 'video') => {
    mediaTypeRef.current = type;
    if (fileInputRef.current) {
      fileInputRef.current.accept = type === 'image' ? 'image/*' : 'video/*';
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

    // Create object URL for preview, and keep the File reference for storage
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
    toast.success(`${mediaTypeRef.current === 'image' ? 'Image' : 'Video'} added!`);

    // Auto-select the matching content type if adding media
    if (mediaTypeRef.current === 'image' && selectedContentType !== 'image') {
      setSelectedContentType('image');
    } else if (mediaTypeRef.current === 'video' && selectedContentType !== 'video') {
      setSelectedContentType('video');
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

  const handleShare = (postId: string) => {
    const post = posts.find((p) => p.postId === postId);
    if (post) {
      navigator.clipboard.writeText(post.content.slice(0, 100) + '...');
      toast.success('Post link copied to clipboard');
    }
  };

  const handleDeletePost = async (postId: string) => {
    try {
      await deletePost(postId);
      setShowPostMenu(null);
      toast.success('Post deleted');
    } catch (err) {
      log.error('Failed to delete post', err);
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
          <h1 className="text-2xl font-bold" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
            Wall
          </h1>
          <p className="text-sm mt-1" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
            Your personal space for thoughts and creations
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
                      ) : (
                        <video src={media.url} className="w-32 h-32 object-cover" />
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
              </div>

              <div className="flex items-center gap-2">
                {isComposing && (
                  <button
                    onClick={() => {
                      setIsComposing(false);
                      setNewPost('');
                      setSelectedContentType('post');
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

          {/* Filter bar */}
          <div className="flex items-center gap-2 overflow-x-auto pb-1">
            <span
              className="text-xs font-medium flex-shrink-0"
              style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
            >
              Filter:
            </span>
            {FILTER_OPTIONS.map((opt) => {
              const isActive = filterType === opt.type;
              return (
                <button
                  key={opt.type}
                  onClick={() => setFilterType(opt.type)}
                  className="px-3 py-1.5 rounded-full text-xs font-medium transition-all duration-200 flex-shrink-0"
                  style={{
                    background: isActive ? 'hsl(var(--harbor-primary) / 0.15)' : 'transparent',
                    color: isActive
                      ? 'hsl(var(--harbor-primary))'
                      : 'hsl(var(--harbor-text-secondary))',
                    border: isActive
                      ? '1px solid hsl(var(--harbor-primary) / 0.3)'
                      : '1px solid hsl(var(--harbor-border-subtle))',
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
                {filterType === 'all'
                  ? 'No posts yet'
                  : `No ${FILTER_OPTIONS.find((f) => f.type === filterType)?.label.toLowerCase() || 'posts'} yet`}
              </h3>
              <p
                className="text-sm max-w-xs mx-auto"
                style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
              >
                {filterType === 'all'
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
                          className={`whitespace-pre-wrap leading-relaxed ${post.contentType === 'thought' ? 'text-lg italic' : 'text-base'
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
                  {post.media && post.media.length > 0 && (
                    <PostMedia media={post.media} />
                  )}
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
                    onClick={() => toast('Comments coming soon!')}
                    className="flex items-center gap-2 transition-colors duration-200"
                    style={{ color: 'hsl(var(--harbor-text-secondary))' }}
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
