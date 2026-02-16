import { useState, useEffect, useRef } from 'react';
import toast from 'react-hot-toast';
import { useBoardsStore, useIdentityStore } from '../stores';
import type { CommunityInfo, BoardInfo, BoardPost } from '../types/boards';

function formatTimeAgo(unixSeconds: number): string {
  const now = Date.now();
  const date = new Date(unixSeconds * 1000);
  const diff = now - date.getTime();

  const minutes = Math.floor(diff / 60000);
  if (minutes < 1) return 'Just now';
  if (minutes < 60) return `${minutes}m ago`;

  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;

  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d ago`;

  return date.toLocaleDateString();
}

function shortPeerId(peerId: string): string {
  if (peerId.length <= 16) return peerId;
  return `${peerId.slice(0, 8)}...${peerId.slice(-6)}`;
}

// Post card component
function PostCard({
  post,
  isOwnPost,
  onDelete,
}: {
  post: BoardPost;
  isOwnPost: boolean;
  onDelete: (postId: string) => void;
}) {
  return (
    <div
      className="p-4 rounded-xl"
      style={{
        background: 'hsl(var(--harbor-surface-1))',
        border: '1px solid hsl(var(--harbor-border-subtle))',
      }}
    >
      {/* Post header */}
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          <div
            className="w-8 h-8 rounded-full flex items-center justify-center text-xs font-semibold text-white"
            style={{
              background:
                'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
            }}
          >
            {(post.authorDisplayName || post.authorPeerId).slice(0, 2).toUpperCase()}
          </div>
          <div>
            <p className="text-sm font-medium" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
              {post.authorDisplayName || shortPeerId(post.authorPeerId)}
            </p>
            <p className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
              {formatTimeAgo(post.createdAt)}
            </p>
          </div>
        </div>
        {isOwnPost && (
          <button
            onClick={() => onDelete(post.postId)}
            className="p-1.5 rounded-lg transition-colors hover:bg-white/5"
            title="Delete post"
          >
            <svg
              className="w-4 h-4"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth={1.5}
              style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M14.74 9l-.346 9m-4.788 0L9.26 9m9.968-3.21c.342.052.682.107 1.022.166m-1.022-.165L18.16 19.673a2.25 2.25 0 01-2.244 2.077H8.084a2.25 2.25 0 01-2.244-2.077L4.772 5.79m14.456 0a48.108 48.108 0 00-3.478-.397m-12 .562c.34-.059.68-.114 1.022-.165m0 0a48.11 48.11 0 013.478-.397m7.5 0v-.916c0-1.18-.91-2.164-2.09-2.201a51.964 51.964 0 00-3.32 0c-1.18.037-2.09 1.022-2.09 2.201v.916m7.5 0a48.667 48.667 0 00-7.5 0"
              />
            </svg>
          </button>
        )}
      </div>
      {/* Post content */}
      <p
        className="text-sm whitespace-pre-wrap"
        style={{ color: 'hsl(var(--harbor-text-primary))' }}
      >
        {post.contentText}
      </p>
    </div>
  );
}

// Post composer component
function PostComposer({ onSubmit }: { onSubmit: (text: string) => Promise<void> }) {
  const [text, setText] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const handleSubmit = async () => {
    if (!text.trim() || isSubmitting) return;
    setIsSubmitting(true);
    try {
      await onSubmit(text.trim());
      setText('');
      if (textareaRef.current) {
        textareaRef.current.style.height = 'auto';
      }
    } catch {
      // Error handled by store
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      handleSubmit();
    }
  };

  return (
    <div
      className="p-4 rounded-xl"
      style={{
        background: 'hsl(var(--harbor-surface-1))',
        border: '1px solid hsl(var(--harbor-border-subtle))',
      }}
    >
      <textarea
        ref={textareaRef}
        value={text}
        onChange={(e) => {
          setText(e.target.value);
          // Auto-resize
          e.target.style.height = 'auto';
          e.target.style.height = e.target.scrollHeight + 'px';
        }}
        onKeyDown={handleKeyDown}
        placeholder="Write something..."
        className="w-full bg-transparent resize-none text-sm outline-none placeholder-opacity-50"
        style={{
          color: 'hsl(var(--harbor-text-primary))',
          minHeight: '60px',
        }}
        rows={2}
      />
      <div className="flex items-center justify-between mt-3">
        <p className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
          Ctrl+Enter to post
        </p>
        <button
          onClick={handleSubmit}
          disabled={!text.trim() || isSubmitting}
          className="px-4 py-1.5 rounded-lg text-sm font-medium text-white transition-all"
          style={{
            background:
              text.trim() && !isSubmitting
                ? 'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))'
                : 'hsl(var(--harbor-surface-2))',
            opacity: text.trim() && !isSubmitting ? 1 : 0.5,
            cursor: text.trim() && !isSubmitting ? 'pointer' : 'not-allowed',
          }}
        >
          {isSubmitting ? 'Posting...' : 'Post'}
        </button>
      </div>
    </div>
  );
}

// Board tabs component
function BoardTabs({
  boards,
  activeBoard,
  onSelect,
}: {
  boards: BoardInfo[];
  activeBoard: BoardInfo | null;
  onSelect: (board: BoardInfo) => void;
}) {
  if (boards.length === 0) return null;

  return (
    <div className="flex gap-1 overflow-x-auto pb-1 hide-scrollbar">
      {boards.map((board) => {
        const isActive = activeBoard?.boardId === board.boardId;
        return (
          <button
            key={board.boardId}
            onClick={() => onSelect(board)}
            className="px-3 py-1.5 rounded-lg text-sm font-medium whitespace-nowrap transition-all"
            style={{
              background: isActive
                ? 'linear-gradient(135deg, hsl(var(--harbor-primary) / 0.15), hsl(var(--harbor-accent) / 0.1))'
                : 'transparent',
              color: isActive ? 'hsl(var(--harbor-primary))' : 'hsl(var(--harbor-text-secondary))',
              border: isActive
                ? '1px solid hsl(var(--harbor-primary) / 0.2)'
                : '1px solid transparent',
            }}
          >
            {board.name}
          </button>
        );
      })}
    </div>
  );
}

// Community sidebar item
function CommunityItem({
  community,
  isActive,
  onSelect,
  onLeave,
}: {
  community: CommunityInfo;
  isActive: boolean;
  onSelect: () => void;
  onLeave: () => void;
}) {
  return (
    <div
      className="flex items-center gap-2 p-2 rounded-lg cursor-pointer transition-all group"
      onClick={onSelect}
      style={{
        background: isActive ? 'hsl(var(--harbor-surface-1))' : 'transparent',
        border: isActive ? '1px solid hsl(var(--harbor-primary) / 0.2)' : '1px solid transparent',
      }}
    >
      <div
        className="w-8 h-8 rounded-lg flex items-center justify-center text-xs font-bold text-white flex-shrink-0"
        style={{
          background:
            'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))',
        }}
      >
        {(community.communityName || 'C').slice(0, 1).toUpperCase()}
      </div>
      <div className="flex-1 min-w-0">
        <p
          className="text-sm font-medium truncate"
          style={{ color: 'hsl(var(--harbor-text-primary))' }}
        >
          {community.communityName || shortPeerId(community.relayPeerId)}
        </p>
        <p className="text-xs truncate" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
          {shortPeerId(community.relayPeerId)}
        </p>
      </div>
      <button
        onClick={(e) => {
          e.stopPropagation();
          onLeave();
        }}
        className="opacity-0 group-hover:opacity-100 p-1 rounded transition-all hover:bg-white/5"
        title="Leave community"
      >
        <svg
          className="w-3.5 h-3.5"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth={1.5}
          style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
        >
          <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
        </svg>
      </button>
    </div>
  );
}

// Join community modal
function JoinCommunityForm({ onJoin }: { onJoin: (address: string) => Promise<void> }) {
  const [address, setAddress] = useState('');
  const [isJoining, setIsJoining] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!address.trim() || isJoining) return;
    setIsJoining(true);
    try {
      await onJoin(address.trim());
      setAddress('');
      toast.success('Joined community');
    } catch (error) {
      toast.error(`Failed to join: ${error}`);
    } finally {
      setIsJoining(false);
    }
  };

  return (
    <form onSubmit={handleSubmit} className="flex gap-2">
      <input
        type="text"
        value={address}
        onChange={(e) => setAddress(e.target.value)}
        placeholder="/ip4/.../p2p/12D3Koo..."
        className="flex-1 px-3 py-2 rounded-lg text-sm outline-none"
        style={{
          background: 'hsl(var(--harbor-surface-1))',
          color: 'hsl(var(--harbor-text-primary))',
          border: '1px solid hsl(var(--harbor-border-subtle))',
        }}
      />
      <button
        type="submit"
        disabled={!address.trim() || isJoining}
        className="px-4 py-2 rounded-lg text-sm font-medium text-white transition-all"
        style={{
          background:
            address.trim() && !isJoining
              ? 'linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))'
              : 'hsl(var(--harbor-surface-2))',
          opacity: address.trim() && !isJoining ? 1 : 0.5,
        }}
      >
        {isJoining ? 'Joining...' : 'Join'}
      </button>
    </form>
  );
}

export function BoardsPage() {
  const { state } = useIdentityStore();
  const identity = state.status === 'unlocked' ? state.identity : null;

  const {
    communities,
    boards,
    boardPosts,
    activeCommunity,
    activeBoard,
    isLoading,
    error,
    loadCommunities,
    joinCommunity,
    leaveCommunity,
    selectCommunity,
    selectBoard,
    submitPost,
    deletePost,
    refreshBoard,
  } = useBoardsStore();

  useEffect(() => {
    loadCommunities();
  }, [loadCommunities]);

  const handleDeletePost = async (postId: string) => {
    try {
      await deletePost(postId);
      toast.success('Post deleted');
    } catch {
      toast.error('Failed to delete post');
    }
  };

  const handleSubmitPost = async (text: string) => {
    await submitPost(text);
    toast.success('Post submitted');
  };

  const handleLeaveCommunity = async (relayPeerId: string) => {
    try {
      await leaveCommunity(relayPeerId);
      toast.success('Left community');
    } catch {
      toast.error('Failed to leave community');
    }
  };

  // Empty state - no communities joined
  if (communities.length === 0 && !isLoading) {
    return (
      <div className="h-full flex flex-col">
        {/* Header */}
        <div className="p-6 border-b" style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}>
          <h1 className="text-2xl font-bold" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
            Community Boards
          </h1>
          <p className="text-sm mt-1" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
            Join a relay community to see boards and posts
          </p>
        </div>

        {/* Empty state */}
        <div className="flex-1 flex items-center justify-center p-6">
          <div className="text-center max-w-md">
            <div
              className="w-16 h-16 rounded-2xl flex items-center justify-center mx-auto mb-4"
              style={{
                background:
                  'linear-gradient(135deg, hsl(var(--harbor-primary) / 0.15), hsl(var(--harbor-accent) / 0.1))',
              }}
            >
              <svg
                className="w-8 h-8"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth={1.5}
                style={{ color: 'hsl(var(--harbor-primary))' }}
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  d="M18 18.72a9.094 9.094 0 003.741-.479 3 3 0 00-4.682-2.72m.94 3.198l.001.031c0 .225-.012.447-.037.666A11.944 11.944 0 0112 21c-2.17 0-4.207-.576-5.963-1.584A6.062 6.062 0 016 18.719m12 0a5.971 5.971 0 00-.941-3.197m0 0A5.995 5.995 0 0012 12.75a5.995 5.995 0 00-5.058 2.772m0 0a3 3 0 00-4.681 2.72 8.986 8.986 0 003.74.477m.94-3.197a5.971 5.971 0 00-.94 3.197M15 6.75a3 3 0 11-6 0 3 3 0 016 0zm6 3a2.25 2.25 0 11-4.5 0 2.25 2.25 0 014.5 0zm-13.5 0a2.25 2.25 0 11-4.5 0 2.25 2.25 0 014.5 0z"
                />
              </svg>
            </div>
            <h2
              className="text-lg font-semibold mb-2"
              style={{ color: 'hsl(var(--harbor-text-primary))' }}
            >
              Join a Community
            </h2>
            <p className="text-sm mb-6" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
              Communities are hosted on relay servers. Enter a relay address to join and start
              browsing boards.
            </p>
            <div className="max-w-lg mx-auto">
              <JoinCommunityForm onJoin={joinCommunity} />
            </div>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <div className="p-6 border-b" style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}>
        <div className="flex items-center justify-between mb-4">
          <div>
            <h1 className="text-2xl font-bold" style={{ color: 'hsl(var(--harbor-text-primary))' }}>
              Community Boards
            </h1>
            <p className="text-sm mt-1" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
              {activeCommunity
                ? activeCommunity.communityName || shortPeerId(activeCommunity.relayPeerId)
                : 'Select a community'}
            </p>
          </div>
          {activeCommunity && activeBoard && (
            <button
              onClick={refreshBoard}
              disabled={isLoading}
              className="px-3 py-1.5 rounded-lg text-sm font-medium transition-all"
              style={{
                background: 'hsl(var(--harbor-surface-1))',
                color: 'hsl(var(--harbor-text-primary))',
                border: '1px solid hsl(var(--harbor-border-subtle))',
                opacity: isLoading ? 0.5 : 1,
              }}
            >
              {isLoading ? 'Syncing...' : 'Refresh'}
            </button>
          )}
        </div>

        {/* Join community form */}
        <JoinCommunityForm onJoin={joinCommunity} />
      </div>

      <div className="flex-1 flex overflow-hidden">
        {/* Community sidebar */}
        <div
          className="w-56 border-r p-3 overflow-y-auto flex-shrink-0"
          style={{
            borderColor: 'hsl(var(--harbor-border-subtle))',
            background: 'hsl(var(--harbor-bg-elevated))',
          }}
        >
          <p
            className="text-xs font-semibold uppercase tracking-wider mb-2 px-2"
            style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
          >
            Communities
          </p>
          <div className="space-y-1">
            {communities.map((community) => (
              <CommunityItem
                key={community.relayPeerId}
                community={community}
                isActive={activeCommunity?.relayPeerId === community.relayPeerId}
                onSelect={() => selectCommunity(community)}
                onLeave={() => handleLeaveCommunity(community.relayPeerId)}
              />
            ))}
          </div>
        </div>

        {/* Main content area */}
        <div className="flex-1 overflow-y-auto">
          {!activeCommunity ? (
            <div className="flex items-center justify-center h-full">
              <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                Select a community to browse boards
              </p>
            </div>
          ) : (
            <div className="max-w-2xl mx-auto p-6 space-y-4">
              {/* Board tabs */}
              <BoardTabs boards={boards} activeBoard={activeBoard} onSelect={selectBoard} />

              {/* Error display */}
              {error && (
                <div
                  className="p-3 rounded-lg text-sm"
                  style={{
                    background: 'hsl(var(--harbor-error) / 0.1)',
                    color: 'hsl(var(--harbor-error))',
                    border: '1px solid hsl(var(--harbor-error) / 0.2)',
                  }}
                >
                  {error}
                </div>
              )}

              {/* Post composer */}
              {activeBoard && <PostComposer onSubmit={handleSubmitPost} />}

              {/* Posts */}
              {boardPosts.length === 0 && !isLoading ? (
                <div className="text-center py-12">
                  <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                    {activeBoard ? 'No posts yet. Be the first to post!' : 'No boards available.'}
                  </p>
                </div>
              ) : (
                <div className="space-y-3">
                  {boardPosts.map((post) => (
                    <PostCard
                      key={post.postId}
                      post={post}
                      isOwnPost={identity?.peerId === post.authorPeerId}
                      onDelete={handleDeletePost}
                    />
                  ))}
                </div>
              )}

              {/* Loading indicator */}
              {isLoading && (
                <div className="flex justify-center py-4">
                  <div
                    className="w-6 h-6 rounded-full border-2 animate-spin"
                    style={{
                      borderColor: 'hsl(var(--harbor-border-subtle))',
                      borderTopColor: 'hsl(var(--harbor-primary))',
                    }}
                  />
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
