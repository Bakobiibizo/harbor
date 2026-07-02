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
});
