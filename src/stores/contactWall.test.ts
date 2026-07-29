import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useContactWallStore } from './contactWall';
import { feedService } from '../services/feed';
import { commentsService } from '../services/comments';
import { likesService } from '../services/likes';
import { permissionsService } from '../services/permissions';

vi.mock('../services/feed', () => ({
  feedService: {
    fetchContactWall: vi.fn(),
    fetchWallSocialEvents: vi.fn(),
    getWall: vi.fn(),
    syncWallSocialEventsToRelay: vi.fn(),
  },
}));

vi.mock('../services/comments', () => ({
  commentsService: {
    getComments: vi.fn(),
    addComment: vi.fn(),
    deleteComment: vi.fn(),
    getCommentCounts: vi.fn(),
  },
}));

vi.mock('../services/likes', () => ({
  likesService: {
    getPostsLikesBatch: vi.fn(),
    likePost: vi.fn(),
    unlikePost: vi.fn(),
  },
}));

vi.mock('../services/permissions', () => ({
  permissionsService: {
    weHaveCapability: vi.fn(),
  },
}));

vi.mock('../services/media', () => ({
  mediaService: {
    preloadMissingMedia: vi.fn().mockResolvedValue(undefined),
  },
}));

const wallItems = [
  {
    postId: 'post-1',
    authorPeerId: 'peer-alice',
    authorDisplayName: 'Alice',
    contentType: 'text',
    contentText: 'Public update',
    visibility: 'public',
    lamportClock: 1,
    createdAt: 1700000100,
    updatedAt: 1700000100,
    isLocal: false,
  },
  {
    postId: 'post-2',
    authorPeerId: 'peer-alice',
    authorDisplayName: 'Alice',
    contentType: 'text',
    contentText: 'Contacts update',
    visibility: 'contacts',
    lamportClock: 2,
    createdAt: 1700000000,
    updatedAt: 1700000000,
    isLocal: false,
  },
];

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

describe('useContactWallStore', () => {
  beforeEach(() => {
    useContactWallStore.getState().reset();
    vi.clearAllMocks();
    vi.mocked(commentsService.getCommentCounts).mockResolvedValue([]);
    vi.mocked(likesService.getPostsLikesBatch).mockResolvedValue([]);
    vi.mocked(feedService.fetchWallSocialEvents).mockResolvedValue(undefined);
    vi.mocked(feedService.syncWallSocialEventsToRelay).mockResolvedValue(0);
  });

  it('targets relay sync before loading a contact wall from backend state', async () => {
    vi.mocked(feedService.fetchContactWall).mockResolvedValue(undefined);
    vi.mocked(feedService.getWall).mockResolvedValue(wallItems);
    vi.mocked(permissionsService.weHaveCapability).mockResolvedValue(true);

    await useContactWallStore.getState().loadWall('peer-alice', 20);

    expect(feedService.fetchContactWall).toHaveBeenCalledWith('peer-alice');
    expect(feedService.getWall).toHaveBeenCalledWith('peer-alice', 20);
    expect(feedService.fetchWallSocialEvents).toHaveBeenCalledWith('peer-alice', [
      'post-1',
      'post-2',
    ]);
    expect(useContactWallStore.getState().wallItems).toEqual(
      wallItems.map((item) => ({ ...item, likes: 0, likedByUser: false })),
    );
    expect(useContactWallStore.getState().canReadContactsOnly).toBe(true);
    expect(useContactWallStore.getState().syncStatus).toBe('success');
    expect(useContactWallStore.getState().lastSyncAt).toBeGreaterThan(0);
    expect(commentsService.getCommentCounts).toHaveBeenCalledWith(['post-1', 'post-2']);
    expect(likesService.getPostsLikesBatch).toHaveBeenCalledWith(['post-1', 'post-2']);
  });

  it('still displays locally authorized public-only wall data when relay sync fails', async () => {
    vi.mocked(feedService.fetchContactWall).mockRejectedValue(new Error('relay offline'));
    vi.mocked(feedService.getWall).mockResolvedValue([wallItems[0]]);
    vi.mocked(permissionsService.weHaveCapability).mockResolvedValue(false);

    await useContactWallStore.getState().loadWall('peer-alice', 20);

    const state = useContactWallStore.getState();
    expect(state.syncError).toContain('relay offline');
    expect(state.syncStatus).toBe('partial_failure');
    expect(state.lastSyncAt).toBeGreaterThan(0);
    expect(state.wallItems).toEqual([{ ...wallItems[0], likes: 0, likedByUser: false }]);
    expect(feedService.fetchWallSocialEvents).toHaveBeenCalledWith('peer-alice', ['post-1']);
    expect(state.canReadContactsOnly).toBe(false);
    expect(state.error).toBeNull();
  });

  it('keeps the newest route peer authoritative when the old wall resolves last', async () => {
    const peerBItems = wallItems.map((item, index) => ({
      ...item,
      postId: `peer-b-${index}`,
      authorPeerId: 'peer-bob',
      authorDisplayName: 'Bob',
    }));
    const wallA = deferred<typeof wallItems>();
    vi.mocked(feedService.fetchContactWall).mockResolvedValue(undefined);
    vi.mocked(feedService.getWall).mockImplementation((peerId) =>
      peerId === 'peer-alice' ? wallA.promise : Promise.resolve(peerBItems),
    );
    vi.mocked(permissionsService.weHaveCapability).mockImplementation((peerId) =>
      Promise.resolve(peerId === 'peer-bob'),
    );

    const loadingA = useContactWallStore.getState().loadWall('peer-alice', 20);
    await vi.waitFor(() => expect(feedService.getWall).toHaveBeenCalledWith('peer-alice', 20));
    await useContactWallStore.getState().loadWall('peer-bob', 20);
    wallA.resolve(wallItems);
    await loadingA;

    expect(useContactWallStore.getState()).toMatchObject({
      authorPeerId: 'peer-bob',
      wallItems: peerBItems.map((item) => ({ ...item, likes: 0, likedByUser: false })),
      canReadContactsOnly: true,
      isLoading: false,
      isSyncing: false,
      error: null,
    });
  });

  it('ignores loading and error completion from an old route peer', async () => {
    const wallA = deferred<typeof wallItems>();
    vi.mocked(feedService.fetchContactWall).mockResolvedValue(undefined);
    vi.mocked(feedService.getWall).mockImplementation((peerId) =>
      peerId === 'peer-alice' ? wallA.promise : Promise.resolve([]),
    );
    vi.mocked(permissionsService.weHaveCapability).mockResolvedValue(false);

    const loadingA = useContactWallStore.getState().loadWall('peer-alice', 20);
    await vi.waitFor(() => expect(feedService.getWall).toHaveBeenCalledWith('peer-alice', 20));
    await useContactWallStore.getState().loadWall('peer-bob', 20);
    wallA.reject(new Error('stale wall failure'));
    await loadingA;

    expect(useContactWallStore.getState()).toMatchObject({
      authorPeerId: 'peer-bob',
      wallItems: [],
      isLoading: false,
      isSyncing: false,
      error: null,
      syncError: null,
    });
  });

  it('does not commit a delayed wall after profile teardown', async () => {
    const wallA = deferred<typeof wallItems>();
    vi.mocked(feedService.fetchContactWall).mockResolvedValue(undefined);
    vi.mocked(feedService.getWall).mockReturnValue(wallA.promise);
    vi.mocked(permissionsService.weHaveCapability).mockResolvedValue(true);

    const loading = useContactWallStore.getState().loadWall('peer-alice', 20);
    await vi.waitFor(() => expect(feedService.getWall).toHaveBeenCalled());
    useContactWallStore.getState().reset();
    wallA.resolve(wallItems);
    await loading;

    expect(useContactWallStore.getState()).toMatchObject({
      authorPeerId: null,
      wallItems: [],
      isLoading: false,
      isSyncing: false,
      error: null,
    });
  });

  it('reconciles event-driven wall changes from local state without starting another sync', async () => {
    useContactWallStore.setState({ authorPeerId: 'peer-alice' });
    vi.mocked(feedService.getWall).mockResolvedValue(wallItems);
    vi.mocked(permissionsService.weHaveCapability).mockResolvedValue(true);

    await useContactWallStore.getState().reconcileWall(20);

    expect(feedService.fetchContactWall).not.toHaveBeenCalled();
    expect(feedService.fetchWallSocialEvents).not.toHaveBeenCalled();
    expect(feedService.getWall).toHaveBeenCalledWith('peer-alice', 20);
    expect(useContactWallStore.getState().wallItems).toHaveLength(2);
  });

  it('paginates contact wall posts using the last loaded timestamp', async () => {
    useContactWallStore.setState({
      authorPeerId: 'peer-alice',
      wallItems: [wallItems[0]],
      hasMore: true,
      isLoading: false,
    });
    vi.mocked(feedService.getWall).mockResolvedValue([wallItems[1]]);

    await useContactWallStore.getState().loadMore(10);

    expect(feedService.getWall).toHaveBeenCalledWith('peer-alice', 10, wallItems[0].createdAt);
    expect(useContactWallStore.getState().wallItems).toEqual([
      wallItems[0],
      { ...wallItems[1], likes: 0, likedByUser: false },
    ]);
  });

  it('does not fetch more when no additional contact wall page is available', async () => {
    useContactWallStore.setState({
      authorPeerId: 'peer-alice',
      wallItems: [wallItems[0]],
      hasMore: false,
      isLoading: false,
    });

    await useContactWallStore.getState().loadMore(10);

    expect(feedService.getWall).not.toHaveBeenCalled();
  });

  it('loads signed reaction summaries and toggles contact wall likes', async () => {
    vi.mocked(feedService.fetchContactWall).mockResolvedValue(undefined);
    vi.mocked(feedService.getWall).mockResolvedValue(wallItems);
    vi.mocked(permissionsService.weHaveCapability).mockResolvedValue(true);
    vi.mocked(likesService.getPostsLikesBatch).mockResolvedValue([
      { postId: 'post-1', totalLikes: 2, userHasLiked: false },
      { postId: 'post-2', totalLikes: 1, userHasLiked: true },
    ]);

    await useContactWallStore.getState().loadWall('peer-alice', 20);

    expect(useContactWallStore.getState().wallItems[0]).toMatchObject({
      postId: 'post-1',
      likes: 2,
      likedByUser: false,
    });
    expect(useContactWallStore.getState().wallItems[1]).toMatchObject({
      postId: 'post-2',
      likes: 1,
      likedByUser: true,
    });

    vi.mocked(likesService.likePost).mockResolvedValue({
      postId: 'post-1',
      totalLikes: 3,
      userHasLiked: true,
    });
    await useContactWallStore.getState().toggleLike('post-1');

    expect(likesService.likePost).toHaveBeenCalledWith('post-1');
    expect(feedService.syncWallSocialEventsToRelay).toHaveBeenCalledTimes(1);
    expect(useContactWallStore.getState().wallItems[0]).toMatchObject({
      likes: 3,
      likedByUser: true,
    });

    vi.mocked(likesService.unlikePost).mockResolvedValue({
      postId: 'post-2',
      totalLikes: 0,
      userHasLiked: false,
    });
    await useContactWallStore.getState().toggleLike('post-2');

    expect(likesService.unlikePost).toHaveBeenCalledWith('post-2');
    expect(feedService.syncWallSocialEventsToRelay).toHaveBeenCalledTimes(2);
    expect(useContactWallStore.getState().wallItems[1]).toMatchObject({
      likes: 0,
      likedByUser: false,
    });
  });

  it('loads and updates contact wall comments', async () => {
    vi.mocked(commentsService.getComments).mockResolvedValue([
      {
        id: 1,
        commentId: 'comment-1',
        postId: 'post-1',
        authorPeerId: 'peer-bob',
        authorName: 'Bob',
        content: 'Nice',
        createdAt: 1700000200,
        deletedAt: null,
      },
    ]);

    await useContactWallStore.getState().loadComments('post-1');

    expect(useContactWallStore.getState().comments['post-1']).toHaveLength(1);
    expect(useContactWallStore.getState().commentCounts['post-1']).toBe(1);
  });

  it('does not attach delayed comments to a newly selected contact wall', async () => {
    const oldComments = deferred<Awaited<ReturnType<typeof commentsService.getComments>>>();
    useContactWallStore.setState({ authorPeerId: 'peer-alice' });
    vi.mocked(commentsService.getComments).mockReturnValue(oldComments.promise);
    vi.mocked(feedService.fetchContactWall).mockResolvedValue(undefined);
    vi.mocked(feedService.getWall).mockResolvedValue([]);
    vi.mocked(permissionsService.weHaveCapability).mockResolvedValue(false);

    const loadingComments = useContactWallStore.getState().loadComments('post-1');
    await useContactWallStore.getState().loadWall('peer-bob', 20);
    oldComments.resolve([
      {
        id: 1,
        commentId: 'old-comment',
        postId: 'post-1',
        authorPeerId: 'peer-alice',
        authorName: 'Alice',
        content: 'old wall only',
        createdAt: 1,
        deletedAt: null,
      },
    ]);
    await loadingComments;

    expect(useContactWallStore.getState().authorPeerId).toBe('peer-bob');
    expect(useContactWallStore.getState().comments).toEqual({});
    expect(useContactWallStore.getState().loadingComments.size).toBe(0);
  });
});
