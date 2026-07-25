import { useCallback, useEffect, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import toast from 'react-hot-toast';
import { PostMedia, type PostMediaItem } from '../components/common/PostMedia';
import { FeedIcon } from '../components/icons';
import { useContactWallStore, useContactsStore, useIdentityStore } from '../stores';
import { postsService } from '../services/posts';
import { getContactColor, getInitials, formatDate } from '../utils/formatting';
import { createLogger } from '../utils/logger';
import { safePeerLabel } from '../utils/relayName';

const log = createLogger('ContactWall');
const PAGE_SIZE = 20;

export function publicOnlyText(canReadContactsOnly: boolean | null): string {
  if (canReadContactsOnly) {
    return 'Contact access is active: public and contacts-only posts are visible.';
  }
  if (canReadContactsOnly === false) {
    return 'Contacts-only access is not active: it may be ungranted, expired, or revoked. New private posts are not served; previously downloaded posts may remain on this device.';
  }
  return 'Checking contact access. Public posts remain available while Harbor verifies permission.';
}

interface ContactWallPageProps {
  peerIdOverride?: string;
  verifiedQualifiedNameOverride?: string;
}

export function ContactWallPage({
  peerIdOverride,
  verifiedQualifiedNameOverride,
}: ContactWallPageProps = {}) {
  const { peerId: routePeerId } = useParams<{ peerId: string }>();
  const peerId = peerIdOverride || routePeerId;
  const navigate = useNavigate();
  const identityState = useIdentityStore((state) => state.state);
  const currentPeerId =
    identityState.status === 'unlocked' || identityState.status === 'locked'
      ? identityState.identity.peerId
      : '';
  const { loadContacts } = useContactsStore();
  const {
    wallItems,
    isLoading,
    isSyncing,
    error,
    syncError,
    lastSyncAt,
    syncStatus,
    hasMore,
    canReadContactsOnly,
    comments,
    commentCounts,
    expandedComments,
    loadingComments,
    loadWall,
    loadMore,
    refreshWall,
    toggleLike,
    toggleComments,
    addComment,
    deleteComment,
    reset,
  } = useContactWallStore();
  const [postMediaMap, setPostMediaMap] = useState<Record<string, PostMediaItem[]>>({});
  const [newComments, setNewComments] = useState<Record<string, string>>({});

  useEffect(() => {
    loadContacts().catch((err) => log.warn('Failed to load contacts for contact wall', err));
  }, [loadContacts]);

  useEffect(() => {
    if (peerId) {
      loadWall(peerId, PAGE_SIZE).catch((err) => log.error('Failed to load contact wall', err));
    } else {
      reset();
    }
    return () => reset();
  }, [peerId, loadWall, reset]);

  useEffect(() => {
    let cancelled = false;
    const loadMedia = async () => {
      const next: Record<string, PostMediaItem[]> = {};
      await Promise.allSettled(
        wallItems.map(async (item) => {
          try {
            const media = await postsService.getPostMedia(item.postId);
            if (media.length > 0) {
              next[item.postId] = media.map((m) => ({
                type:
                  m.mediaType === 'video' ? 'video' : m.mediaType === 'audio' ? 'audio' : 'image',
                url: m.mediaHash,
                name: m.fileName,
                sourcePeerId: item.authorPeerId,
                mimeType: m.mimeType,
                totalBytes: m.fileSize,
              }));
            }
          } catch (err) {
            log.warn('Failed to load contact wall media', err);
          }
        }),
      );
      if (!cancelled) setPostMediaMap(next);
    };

    if (wallItems.length === 0) {
      setPostMediaMap({});
    } else {
      loadMedia();
    }

    return () => {
      cancelled = true;
    };
  }, [wallItems]);

  const displayName = safePeerLabel(
    peerId || 'unknown',
    verifiedQualifiedNameOverride || wallItems[0]?.authorVerifiedQualifiedName,
    wallItems[0]?.authorDisplayName,
  );
  const authorPeerId = peerId || '';

  const handleRefresh = useCallback(async () => {
    try {
      await refreshWall(PAGE_SIZE);
      toast.success('Posts refreshed');
    } catch {
      toast.error('Failed to refresh posts');
    }
  }, [refreshWall]);

  const handleLike = useCallback(
    async (postId: string) => {
      try {
        await toggleLike(postId);
      } catch (err) {
        log.error('Failed to update contact wall reaction', err);
        toast.error('Could not update reaction');
      }
    },
    [toggleLike],
  );

  const handleAddComment = async (postId: string) => {
    const content = (newComments[postId] || '').trim();
    if (!content) return;
    try {
      await addComment(postId, content);
      setNewComments((current) => ({ ...current, [postId]: '' }));
      toast.success('Comment added');
    } catch {
      toast.error('Failed to add comment');
    }
  };

  if (!peerId) {
    return (
      <div
        className="h-full flex items-center justify-center"
        style={{ background: 'hsl(var(--harbor-bg-primary))' }}
      >
        <p style={{ color: 'hsl(var(--harbor-text-secondary))' }}>Missing profile link.</p>
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col" style={{ background: 'hsl(var(--harbor-bg-primary))' }}>
      <header
        className="harbor-page-gutter-x flex-shrink-0 border-b py-4"
        style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
      >
        <div className="max-w-3xl mx-auto flex items-start justify-between gap-4">
          <div className="flex items-start gap-4 min-w-0">
            <button
              onClick={() => navigate(-1)}
              className="p-2 -ml-2 rounded-lg transition-colors hover:bg-white/5"
              style={{ color: 'hsl(var(--harbor-text-secondary))' }}
              title="Back"
            >
              <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={1.5}
                  d="M15 19l-7-7 7-7"
                />
              </svg>
            </button>
            <div
              className="w-12 h-12 rounded-full flex items-center justify-center text-base font-semibold text-white flex-shrink-0"
              style={{ background: getContactColor(authorPeerId) }}
            >
              {getInitials(displayName)}
            </div>
            <div className="min-w-0">
              <h1
                className="text-2xl font-bold truncate"
                style={{ color: 'hsl(var(--harbor-text-primary))' }}
              >
                {displayName}'s profile
              </h1>
              <p className="text-sm mt-2" style={{ color: 'hsl(var(--harbor-text-secondary))' }}>
                {publicOnlyText(canReadContactsOnly)}
              </p>
              <p
                className="text-xs mt-2"
                style={{
                  color:
                    syncStatus === 'partial_failure'
                      ? 'hsl(var(--harbor-warning))'
                      : 'hsl(var(--harbor-text-tertiary))',
                }}
              >
                {isSyncing
                  ? 'Sync in progress… local posts remain available.'
                  : lastSyncAt
                    ? `Last synced ${formatDate(new Date(lastSyncAt * 1000))}${syncError ? ' · Partial sync failure, retry available.' : ''}`
                    : 'Not synced yet. Refresh to request these posts from the relay.'}
              </p>
            </div>
          </div>
          <button
            onClick={handleRefresh}
            disabled={isLoading || isSyncing}
            className="px-4 py-2 rounded-lg text-sm font-medium transition-all duration-200 flex-shrink-0"
            style={{
              background: 'hsl(var(--harbor-surface-1))',
              color: 'hsl(var(--harbor-text-secondary))',
              border: '1px solid hsl(var(--harbor-border-subtle))',
              opacity: isLoading || isSyncing ? 0.6 : 1,
            }}
          >
            {isSyncing ? 'Syncing…' : isLoading ? 'Loading…' : 'Refresh'}
          </button>
        </div>
      </header>

      <div className="harbor-page-gutter flex-1 overflow-y-auto">
        <div className="max-w-3xl mx-auto space-y-6">
          {error && (
            <div
              className="rounded-lg p-4"
              style={{
                background: 'hsl(var(--harbor-error) / 0.12)',
                color: 'hsl(var(--harbor-error))',
              }}
            >
              {error}
            </div>
          )}

          {wallItems.length === 0 && !isLoading ? (
            <div className="text-center py-16">
              <div
                className="w-20 h-20 rounded-lg flex items-center justify-center mx-auto mb-4"
                style={{ background: 'hsl(var(--harbor-surface-1))' }}
              >
                <FeedIcon
                  className="w-10 h-10"
                  style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
                />
              </div>
              <h3
                className="text-lg font-semibold mb-2"
                style={{ color: 'hsl(var(--harbor-text-primary))' }}
              >
                No visible posts
              </h3>
              <p
                className="text-sm max-w-sm mx-auto"
                style={{ color: 'hsl(var(--harbor-text-tertiary))' }}
              >
                This contact has no public posts available locally. Refresh to target the relay, or
                ask them to approve contact access if you expect contacts-only posts.
              </p>
            </div>
          ) : (
            wallItems.map((item) => (
              <article
                key={item.postId}
                className="rounded-lg overflow-hidden"
                style={{
                  background: 'hsl(var(--harbor-bg-elevated))',
                  border: '1px solid hsl(var(--harbor-border-subtle))',
                }}
              >
                <div
                  className="px-5 py-4 flex items-center justify-between border-b"
                  style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
                >
                  <div className="flex items-center gap-3 min-w-0">
                    <div
                      className="w-10 h-10 rounded-full flex items-center justify-center text-sm font-semibold text-white flex-shrink-0"
                      style={{ background: getContactColor(item.authorPeerId) }}
                    >
                      {getInitials(displayName)}
                    </div>
                    <div className="min-w-0">
                      <p
                        className="font-semibold text-sm truncate"
                        style={{ color: 'hsl(var(--harbor-text-primary))' }}
                      >
                        {safePeerLabel(
                          item.authorPeerId,
                          item.authorVerifiedQualifiedName,
                          item.authorDisplayName,
                        )}
                      </p>
                      <p className="text-xs" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                        {formatDate(new Date(item.createdAt * 1000))} ·{' '}
                        {item.visibility === 'public' ? 'Public' : 'Contacts-only'}
                      </p>
                    </div>
                  </div>
                </div>

                <div className="px-5 py-5 space-y-4">
                  {item.contentText && (
                    <p
                      className="text-base leading-relaxed whitespace-pre-wrap"
                      style={{ color: 'hsl(var(--harbor-text-primary))' }}
                    >
                      {item.contentText}
                    </p>
                  )}
                  {postMediaMap[item.postId] && <PostMedia media={postMediaMap[item.postId]} />}
                </div>

                <div
                  className="px-5 py-3 flex items-center gap-6 border-t"
                  style={{ borderColor: 'hsl(var(--harbor-border-subtle))' }}
                >
                  <button
                    onClick={() => handleLike(item.postId)}
                    className="flex items-center gap-2 transition-colors duration-200"
                    style={{
                      color: item.likedByUser
                        ? 'hsl(var(--harbor-error))'
                        : 'hsl(var(--harbor-text-secondary))',
                    }}
                    aria-pressed={item.likedByUser ?? false}
                    title={item.likedByUser ? 'Unlike post' : 'Like post'}
                  >
                    <svg
                      className="w-5 h-5"
                      fill={item.likedByUser ? 'currentColor' : 'none'}
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
                    <span className="text-sm">{item.likes ?? 0}</span>
                  </button>

                  <button
                    onClick={() => toggleComments(item.postId)}
                    className="flex items-center gap-2 transition-colors duration-200"
                    style={{
                      color: expandedComments.has(item.postId)
                        ? 'hsl(var(--harbor-primary))'
                        : 'hsl(var(--harbor-text-secondary))',
                    }}
                  >
                    <span className="text-sm">
                      {(commentCounts[item.postId] || 0) > 0
                        ? `Comments (${commentCounts[item.postId]})`
                        : 'Comment'}
                    </span>
                  </button>
                </div>

                {expandedComments.has(item.postId) && (
                  <div
                    className="px-5 py-4 border-t space-y-3"
                    style={{
                      borderColor: 'hsl(var(--harbor-border-subtle))',
                      background: 'hsl(var(--harbor-surface-1) / 0.35)',
                    }}
                  >
                    {loadingComments.has(item.postId) ? (
                      <p className="text-sm" style={{ color: 'hsl(var(--harbor-text-tertiary))' }}>
                        Loading comments…
                      </p>
                    ) : (
                      (comments[item.postId] || []).map((comment) => (
                        <div
                          key={comment.commentId}
                          className="rounded-lg p-3"
                          style={{ background: 'hsl(var(--harbor-bg-elevated))' }}
                        >
                          <div className="flex items-start justify-between gap-3">
                            <div>
                              <p
                                className="text-xs font-medium"
                                style={{ color: 'hsl(var(--harbor-text-secondary))' }}
                              >
                                {safePeerLabel(comment.authorPeerId, undefined, comment.authorName)}
                              </p>
                              <p
                                className="text-sm mt-1"
                                style={{ color: 'hsl(var(--harbor-text-primary))' }}
                              >
                                {comment.content}
                              </p>
                            </div>
                            {comment.authorPeerId === currentPeerId && (
                              <button
                                onClick={() => deleteComment(item.postId, comment.commentId)}
                                className="text-xs"
                                style={{ color: 'hsl(var(--harbor-error))' }}
                              >
                                Delete
                              </button>
                            )}
                          </div>
                        </div>
                      ))
                    )}
                    <div className="flex gap-2">
                      <input
                        value={newComments[item.postId] || ''}
                        onChange={(event) =>
                          setNewComments((current) => ({
                            ...current,
                            [item.postId]: event.target.value,
                          }))
                        }
                        onKeyDown={(event) => {
                          if (event.key === 'Enter') handleAddComment(item.postId);
                        }}
                        placeholder="Write a comment…"
                        className="flex-1 px-3 py-2 rounded-lg text-sm"
                        style={{
                          background: 'hsl(var(--harbor-bg-elevated))',
                          border: '1px solid hsl(var(--harbor-border-subtle))',
                          color: 'hsl(var(--harbor-text-primary))',
                        }}
                      />
                      <button
                        onClick={() => handleAddComment(item.postId)}
                        className="px-3 py-2 rounded-lg text-sm font-medium"
                        style={{ background: 'hsl(var(--harbor-primary))', color: 'white' }}
                      >
                        Send
                      </button>
                    </div>
                  </div>
                )}
              </article>
            ))
          )}

          {hasMore && wallItems.length > 0 && (
            <div className="text-center">
              <button
                onClick={() => loadMore(PAGE_SIZE)}
                disabled={isLoading}
                className="px-4 py-2 rounded-lg text-sm font-medium transition-all duration-200"
                style={{
                  background: 'hsl(var(--harbor-surface-1))',
                  color: 'hsl(var(--harbor-text-secondary))',
                  border: '1px solid hsl(var(--harbor-border-subtle))',
                  opacity: isLoading ? 0.6 : 1,
                }}
              >
                {isLoading ? 'Loading…' : 'Load more'}
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
