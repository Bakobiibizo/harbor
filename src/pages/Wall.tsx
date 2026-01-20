import { useState, useRef, useEffect } from 'react';
import toast from 'react-hot-toast';
import { useIdentityStore, useWallStore } from '../stores';
import { WallIcon, EllipsisIcon } from '../components/icons';

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
  const [pendingMedia, setPendingMedia] = useState<
    { type: 'image' | 'video'; url: string; name: string }[]
  >([]);
  const [showPostMenu, setShowPostMenu] = useState<string | null>(null);
  const [editContent, setEditContent] = useState('');
  const fileInputRef = useRef<HTMLInputElement>(null);
  const mediaTypeRef = useRef<'image' | 'video'>('image');

  const identity = state.status === 'unlocked' ? state.identity : null;

  // Load posts from SQLite on mount
  useEffect(() => {
    if (identity) {
      loadPosts();
    }
  }, [identity, loadPosts]);

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

    try {
      await createPost(newPost.trim(), pendingMedia.length > 0 ? pendingMedia : undefined);
      setNewPost('');
      setPendingMedia([]);
      setIsComposing(false);
      toast.success('Post published!');
    } catch (err) {
      console.error('Failed to create post:', err);
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

    // Create object URL for preview
    const url = URL.createObjectURL(file);
    setPendingMedia([
      ...pendingMedia,
      {
        type: mediaTypeRef.current,
        url,
        name: file.name,
      },
    ]);
    toast.success(`${mediaTypeRef.current === 'image' ? 'Image' : 'Video'} added!`);

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
    } catch (err) {
      console.error('Failed to update post:', err);
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
        <div className="max-w-3xl mx-auto space-y-8">
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
                    Creating a new post
                  </p>
                </div>
              </div>
            </div>

            {/* Composer body */}
            <div className="p-5">
              <textarea
                placeholder="Share your thoughts, ideas, or creative work..."
                value={newPost}
                onChange={(e) => {
                  setNewPost(e.target.value);
                  setIsComposing(true);
                }}
                onFocus={() => setIsComposing(true)}
                rows={isComposing ? 6 : 3}
                className="w-full resize-none text-base leading-relaxed"
                style={{
                  background: 'transparent',
                  border: 'none',
                  outline: 'none',
                  color: 'hsl(var(--harbor-text-primary))',
                }}
              />

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

          {/* Posts - blog style with less rounded corners */}
          {isLoading ? (
            <div className="text-center py-16">
              <div
                className="w-10 h-10 border-2 border-t-transparent rounded-full animate-spin mx-auto mb-4"
                style={{ borderColor: 'hsl(var(--harbor-primary))', borderTopColor: 'transparent' }}
              />
              <p style={{ color: 'hsl(var(--harbor-text-secondary))' }}>Loading posts...</p>
            </div>
          ) : posts.length === 0 ? (
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
                No posts yet
              </h3>
              <p
                className="text-sm max-w-xs mx-auto"
                style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
              >
                Share your first post with your contacts. Your posts are stored locally and shared
                peer-to-peer.
              </p>
            </div>
          ) : (
            posts.map((post) => (
              <article
                key={post.postId}
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
                        className="font-semibold text-sm"
                        style={{ color: 'hsl(var(--harbor-text-primary))' }}
                      >
                        {identity?.displayName || 'You'}
                      </p>
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
                <div className="px-5 py-5">
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
                    <p
                      className="text-base leading-relaxed whitespace-pre-wrap"
                      style={{ color: 'hsl(var(--harbor-text-primary))' }}
                    >
                      {post.content}
                    </p>
                  )}

                  {/* Post media */}
                  {post.media && post.media.length > 0 && (
                    <div className="mt-4 flex flex-wrap gap-3">
                      {post.media.map((media, index) => (
                        <div
                          key={index}
                          className="rounded-lg overflow-hidden"
                          style={{ background: 'hsl(var(--harbor-surface-1))' }}
                        >
                          {media.type === 'image' ? (
                            <img
                              src={media.url}
                              alt={media.name || 'Image'}
                              className="max-w-full max-h-96 object-contain"
                            />
                          ) : (
                            <video src={media.url} controls className="max-w-full max-h-96" />
                          )}
                        </div>
                      ))}
                    </div>
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
