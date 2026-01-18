import { useState, useMemo, useEffect } from "react";
import toast from "react-hot-toast";
import { FeedIcon, EllipsisIcon } from "../components/icons";
import { useMockPeersStore, useFeedStore, useContactsStore } from "../stores";
import type { FeedItem } from "../types";

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
    "linear-gradient(135deg, hsl(220 91% 54%), hsl(262 83% 58%))",
    "linear-gradient(135deg, hsl(262 83% 58%), hsl(330 81% 60%))",
    "linear-gradient(135deg, hsl(152 69% 40%), hsl(180 70% 45%))",
    "linear-gradient(135deg, hsl(36 90% 55%), hsl(15 80% 55%))",
    "linear-gradient(135deg, hsl(200 80% 50%), hsl(220 91% 54%))",
    "linear-gradient(135deg, hsl(340 75% 55%), hsl(10 80% 60%))",
  ];
  let hash = 0;
  for (let i = 0; i < peerId.length; i++) {
    hash = peerId.charCodeAt(i) + ((hash << 5) - hash);
  }
  return colors[Math.abs(hash) % colors.length];
}

export function FeedPage() {
  const { getAllFeedPosts, likePost, toggleSavePost, isPostSaved, peers } = useMockPeersStore();
  const { feedItems, loadFeed, refreshFeed } = useFeedStore();
  const { contacts, loadContacts } = useContactsStore();
  const [isRefreshing, setIsRefreshing] = useState(false);

  // Load real feed and contacts on mount
  useEffect(() => {
    loadFeed();
    loadContacts();
  }, [loadFeed, loadContacts]);

  // Get mock feed posts
  const mockPosts = useMemo(() => getAllFeedPosts(), [peers]);

  // Convert real feed items to unified format
  const realPosts: UnifiedPost[] = useMemo(() => {
    return feedItems.map((item: FeedItem): UnifiedPost => {
      const contact = contacts.find(c => c.peerId === item.authorPeerId);
      return {
        id: `real-${item.postId}`,
        content: item.contentText || "",
        timestamp: new Date(item.createdAt * 1000),
        likes: 0, // Real posts don't have like counts yet
        comments: 0,
        likedByUser: false,
        author: {
          peerId: item.authorPeerId,
          name: item.authorDisplayName || contact?.displayName || "Unknown",
          avatarGradient: getContactColor(item.authorPeerId),
        },
        isReal: true,
      };
    });
  }, [feedItems, contacts]);

  // Convert mock posts to unified format
  const mockUnifiedPosts: UnifiedPost[] = useMemo(() => {
    return mockPosts.map((post): UnifiedPost => ({
      id: post.id,
      content: post.content,
      timestamp: post.timestamp,
      likes: post.likes,
      comments: post.comments,
      likedByUser: post.likedByUser,
      author: {
        peerId: post.author.peerId,
        name: post.author.name,
        avatarGradient: post.author.avatarGradient,
      },
      isReal: false,
    }));
  }, [mockPosts]);

  // Combine and sort all posts by timestamp (newest first)
  const posts: UnifiedPost[] = useMemo(() => {
    return [...realPosts, ...mockUnifiedPosts].sort(
      (a, b) => b.timestamp.getTime() - a.timestamp.getTime()
    );
  }, [realPosts, mockUnifiedPosts]);

  const formatDate = (date: Date) => {
    const now = new Date();
    const diff = now.getTime() - date.getTime();
    const mins = Math.floor(diff / 60000);
    const hours = Math.floor(diff / 3600000);
    const days = Math.floor(diff / 86400000);

    if (mins < 1) return "Just now";
    if (mins < 60) return `${mins}m ago`;
    if (hours < 24) return `${hours}h ago`;
    if (days < 7) return `${days}d ago`;
    return date.toLocaleDateString("en-US", {
      month: "short",
      day: "numeric",
      year: date.getFullYear() !== now.getFullYear() ? "numeric" : undefined
    });
  };

  const getInitials = (name: string) => {
    return name
      .split(" ")
      .map((n) => n[0])
      .join("")
      .toUpperCase()
      .slice(0, 2);
  };

  const handleRefresh = async () => {
    setIsRefreshing(true);
    try {
      await refreshFeed();
      toast.success("Feed refreshed!");
    } catch (error) {
      toast.error("Failed to refresh feed");
    } finally {
      setIsRefreshing(false);
    }
  };

  const handleLike = (post: UnifiedPost) => {
    if (post.isReal) {
      // Real posts don't support likes yet
      toast("Likes for P2P posts coming soon!", { icon: "💜" });
    } else {
      likePost(post.author.peerId, post.id);
      if (!post.likedByUser) {
        toast.success("Post liked!");
      }
    }
  };

  const handleSave = (post: UnifiedPost) => {
    if (post.isReal) {
      // Real posts don't support saving yet
      toast("Saving P2P posts coming soon!", { icon: "🔖" });
    } else {
      const wasSaved = isPostSaved(post.author.peerId, post.id);
      toggleSavePost(post.author.peerId, post.id);
      if (!wasSaved) {
        toast.success("Post saved to your collection!");
      } else {
        toast.success("Post removed from saved");
      }
    }
  };

  const isSaved = (post: UnifiedPost): boolean => {
    if (post.isReal) return false;
    return isPostSaved(post.author.peerId, post.id);
  };

  const handlePostMenu = (authorName: string) => {
    toast(`Post options for ${authorName}'s post`, {
      icon: "📋",
    });
  };

  return (
    <div
      className="h-full flex flex-col"
      style={{ background: "hsl(var(--harbor-bg-primary))" }}
    >
      {/* Header */}
      <header
        className="px-6 py-4 border-b flex-shrink-0"
        style={{ borderColor: "hsl(var(--harbor-border-subtle))" }}
      >
        <div className="max-w-3xl mx-auto flex items-center justify-between">
          <div>
            <h1
              className="text-2xl font-bold"
              style={{ color: "hsl(var(--harbor-text-primary))" }}
            >
              Feed
            </h1>
            <p
              className="text-sm mt-1"
              style={{ color: "hsl(var(--harbor-text-secondary))" }}
            >
              Updates from your contacts
            </p>
          </div>

          <button
            onClick={handleRefresh}
            disabled={isRefreshing}
            className="px-4 py-2 rounded-lg text-sm font-medium transition-all duration-200"
            style={{
              background: "hsl(var(--harbor-surface-1))",
              color: "hsl(var(--harbor-text-secondary))",
              border: "1px solid hsl(var(--harbor-border-subtle))",
              opacity: isRefreshing ? 0.6 : 1,
            }}
          >
            {isRefreshing ? "Refreshing..." : "Refresh"}
          </button>
        </div>
      </header>

      <div className="flex-1 overflow-y-auto p-6">
        <div className="max-w-3xl mx-auto space-y-8">
          {posts.length === 0 ? (
            <div className="text-center py-16">
              <div
                className="w-20 h-20 rounded-lg flex items-center justify-center mx-auto mb-4"
                style={{ background: "hsl(var(--harbor-surface-1))" }}
              >
                <FeedIcon
                  className="w-10 h-10"
                  style={{ color: "hsl(var(--harbor-text-tertiary))" }}
                />
              </div>
              <h3
                className="text-lg font-semibold mb-2"
                style={{ color: "hsl(var(--harbor-text-primary))" }}
              >
                Your feed is empty
              </h3>
              <p
                className="text-sm max-w-xs mx-auto mb-4"
                style={{ color: "hsl(var(--harbor-text-tertiary))" }}
              >
                When your contacts share posts and grant you permission to view them, they'll appear here.
              </p>
              <button
                className="px-4 py-2 rounded-lg text-sm font-medium transition-all duration-200"
                style={{
                  background: "linear-gradient(135deg, hsl(var(--harbor-primary)), hsl(var(--harbor-accent)))",
                  color: "white",
                  boxShadow: "0 4px 12px hsl(var(--harbor-primary) / 0.3)",
                }}
              >
                Find Contacts
              </button>
            </div>
          ) : (
            posts.map((post) => {
              const saved = isSaved(post);

              return (
                <article
                  key={post.id}
                  className="rounded-lg overflow-hidden"
                  style={{
                    background: "hsl(var(--harbor-bg-elevated))",
                    border: "1px solid hsl(var(--harbor-border-subtle))",
                  }}
                >
                  {/* Post header */}
                  <div
                    className="px-5 py-4 flex items-center justify-between border-b"
                    style={{ borderColor: "hsl(var(--harbor-border-subtle))" }}
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
                            style={{ color: "hsl(var(--harbor-text-primary))" }}
                          >
                            {post.author.name}
                          </p>
                          {post.isReal ? (
                            <span
                              className="text-[10px] px-1.5 py-0.5 rounded"
                              style={{
                                background: "hsl(var(--harbor-success) / 0.15)",
                                color: "hsl(var(--harbor-success))",
                              }}
                            >
                              P2P
                            </span>
                          ) : (
                            <span
                              className="text-[10px] px-1.5 py-0.5 rounded"
                              style={{
                                background: "hsl(var(--harbor-text-tertiary) / 0.15)",
                                color: "hsl(var(--harbor-text-tertiary))",
                              }}
                            >
                              Demo
                            </span>
                          )}
                        </div>
                        <p
                          className="text-xs"
                          style={{ color: "hsl(var(--harbor-text-tertiary))" }}
                        >
                          {formatDate(post.timestamp)}
                        </p>
                      </div>
                    </div>

                    <button
                      onClick={() => handlePostMenu(post.author.name)}
                      className="p-2 rounded-lg transition-colors duration-200 hover:bg-white/5"
                      style={{
                        color: "hsl(var(--harbor-text-tertiary))",
                      }}
                    >
                      <EllipsisIcon className="w-5 h-5" />
                    </button>
                  </div>

                  {/* Post content */}
                  <div className="px-5 py-5">
                    <p
                      className="text-base leading-relaxed whitespace-pre-wrap"
                      style={{ color: "hsl(var(--harbor-text-primary))" }}
                    >
                      {post.content}
                    </p>
                  </div>

                  {/* Post actions */}
                  <div
                    className="px-5 py-3 flex items-center gap-6 border-t"
                    style={{ borderColor: "hsl(var(--harbor-border-subtle))" }}
                  >
                    <button
                      onClick={() => handleLike(post)}
                      className="flex items-center gap-2 transition-colors duration-200"
                      style={{
                        color: post.likedByUser
                          ? "hsl(var(--harbor-error))"
                          : "hsl(var(--harbor-text-secondary))",
                      }}
                    >
                      <svg
                        className="w-5 h-5"
                        fill={post.likedByUser ? "currentColor" : "none"}
                        viewBox="0 0 24 24"
                        stroke="currentColor"
                      >
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M4.318 6.318a4.5 4.5 0 000 6.364L12 20.364l7.682-7.682a4.5 4.5 0 00-6.364-6.364L12 7.636l-1.318-1.318a4.5 4.5 0 00-6.364 0z" />
                      </svg>
                      <span className="text-sm">{post.likes}</span>
                    </button>

                    <button
                      onClick={() => toast("Comments coming soon!")}
                      className="flex items-center gap-2 transition-colors duration-200"
                      style={{
                        color: "hsl(var(--harbor-text-secondary))",
                      }}
                    >
                      <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z" />
                      </svg>
                      <span className="text-sm">{post.comments}</span>
                    </button>

                    <button
                      onClick={() => handleSave(post)}
                      className="flex items-center gap-2 transition-colors duration-200 ml-auto"
                      style={{
                        color: saved
                          ? "hsl(var(--harbor-primary))"
                          : "hsl(var(--harbor-text-secondary))",
                      }}
                    >
                      <svg
                        className="w-5 h-5"
                        fill={saved ? "currentColor" : "none"}
                        viewBox="0 0 24 24"
                        stroke="currentColor"
                      >
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z" />
                      </svg>
                      <span className="text-sm">{saved ? "Saved" : "Save"}</span>
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
