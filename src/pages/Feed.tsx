import { useState, useMemo } from "react";
import toast from "react-hot-toast";
import { FeedIcon, EllipsisIcon } from "../components/icons";
import { useMockPeersStore } from "../stores";

export function FeedPage() {
  const { getAllFeedPosts, likePost, toggleSavePost, isPostSaved, peers } = useMockPeersStore();
  const [isRefreshing, setIsRefreshing] = useState(false);

  // Get all feed posts from mock peers, sorted chronologically
  const posts = useMemo(() => getAllFeedPosts(), [peers]);

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

  const handleRefresh = () => {
    setIsRefreshing(true);
    // Simulate refresh
    setTimeout(() => {
      setIsRefreshing(false);
      toast.success("Feed refreshed!");
    }, 1000);
  };

  const handleLike = (peerId: string, postId: string, alreadyLiked: boolean) => {
    likePost(peerId, postId);
    if (!alreadyLiked) {
      toast.success("Post liked!");
    }
  };

  const handleSave = (peerId: string, postId: string) => {
    const wasSaved = isPostSaved(peerId, postId);
    toggleSavePost(peerId, postId);
    if (!wasSaved) {
      toast.success("Post saved to your collection!");
    } else {
      toast.success("Post removed from saved");
    }
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
              const saved = isPostSaved(post.author.peerId, post.id);

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
                        <p
                          className="font-semibold text-sm"
                          style={{ color: "hsl(var(--harbor-text-primary))" }}
                        >
                          {post.author.name}
                        </p>
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
                      onClick={() => handleLike(post.author.peerId, post.id, post.likedByUser)}
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
                      onClick={() => handleSave(post.author.peerId, post.id)}
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
