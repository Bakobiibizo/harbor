import { useState, useMemo, useEffect, useRef } from 'react';
import toast from 'react-hot-toast';
import { FeedIcon, EllipsisIcon } from '../components/icons';
import { useFeedStore, useContactsStore } from '../stores';
import type { FeedItem } from '../types';

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

// Unified post type for both real and mock posts
interface UnifiedPost {
  id: string;
  content: string;
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
}

// Generate consistent avatar color from peer ID
function getContactColor(peerId: string): string {
  const colors = [
    'linear-gradient(135deg, hsl(220 91% 54%), hsl(262 83% 58%))',
    'linear-gradient(135deg, hsl(262 83% 58%), hsl(330 81% 60%))',
    'linear-gradient(135deg, hsl(152 69% 40%), hsl(180 70% 45%))',
    'linear-gradient(135deg, hsl(36 90% 55%), hsl(15 80% 55%))',
    'linear-gradient(135deg, hsl(200 80% 50%), hsl(220 91% 54%))',
    'linear-gradient(135deg, hsl(340 75% 55%), hsl(10 80% 60%))',
  ];
  let hash = 0;
  for (let i = 0; i < peerId.length; i++) {
    hash = peerId.charCodeAt(i) + ((hash << 5) - hash);
  }
  return colors[Math.abs(hash) % colors.length];
}

type FeedTab = 'all' | 'saved';

export function FeedPage() {
  const { feedItems, loadFeed, refreshFeed } = useFeedStore();
  const { contacts, loadContacts } = useContactsStore();
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [activeTab, setActiveTab] = useState<FeedTab>('all');
  const [openMenuId, setOpenMenuId] = useState<string | null>(null);

  // Load real feed and contacts on mount
  useEffect(() => {
    loadFeed();
    loadContacts();
  }, [loadFeed, loadContacts]);

  // Convert real feed items to unified format
  const allPosts: UnifiedPost[] = useMemo(() => {
    return feedItems
      .map((item: FeedItem): UnifiedPost => {
        const contact = contacts.find((c) => c.peerId === item.authorPeerId);
        return {
          id: `real-${item.postId}`,
          content: item.contentText || '',
          timestamp: new Date(item.createdAt * 1000),
          likes: 0,
          comments: 0,
          likedByUser: false,
          author: {
            peerId: item.authorPeerId,
            name: item.authorDisplayName || contact?.displayName || 'Unknown',
            avatarGradient: getContactColor(item.authorPeerId),
          },
          isReal: true,
        };
      })
      .sort((a, b) => b.timestamp.getTime() - a.timestamp.getTime());
  }, [feedItems, contacts]);

  // Select posts based on active tab (saved tab placeholder for future)
  const posts: UnifiedPost[] = activeTab === 'saved' ? [] : allPosts;

  const formatDate = (date: Date) => {
    const now = new Date();
    const diff = now.getTime() - date.getTime();
    const mins = Math.floor(diff / 60000);
    const hours = Math.floor(diff / 3600000);
    const days = Math.floor(diff / 86400000);

    if (mins < 1) return 'Just now';
    if (mins < 60) return `${mins}m ago`;
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

  const handleRefresh = async () => {
    setIsRefreshing(true);
    try {
      await refreshFeed();
      toast.success('Feed refreshed!');
    } catch {
      toast.error('Failed to refresh feed');
    } finally {
      setIsRefreshing(false);
    }
  };

  const handleLike = (_post: UnifiedPost) => {
    toast('Likes coming soon!');
  };

  const handleSave = (_post: UnifiedPost) => {
    toast('Saving posts coming soon!');
  };

  const isSaved = (_post: UnifiedPost): boolean => {
    return false;
  };

  const handleHidePost = (_post: UnifiedPost) => {
    toast('Hiding posts coming soon!');
  };

  const handleSnoozeUser = (_post: UnifiedPost, _hours: number) => {
    toast('Snoozing contacts coming soon!');
  };

  return (
    <div className="h-full flex flex-col" style={{ background: 'hsl(var(--harbor-bg-primary))' }}>
      {/* Header */}
      <header
        className="px-6 py-4 border-b flex-shrink-0"
        style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
      >
        <div className="max-w-3xl mx-auto">
          <div className="flex items-center justify-between mb-4">
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
            </div>

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

          {/* Tabs */}
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
              {[].length > 0 && (
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
                  {[].length}
                </span>
              )}
            </button>
          </div>
        </div>
      </header>

      <div className="flex-1 overflow-y-auto p-6">
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
                <button
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
                          <p
                            className="font-semibold text-sm"
                            style={{ color: 'hsl(var(--harbor-text-primary))' }}
                          >
                            {post.author.name}
                          </p>
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
                      onClick={() => toast('Comments coming soon!')}
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
                          d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z"
                        />
                      </svg>
                      <span className="text-sm">{post.comments}</span>
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
                </article>
              );
            })
          )}
        </div>
      </div>
    </div>
  );
}
