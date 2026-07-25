import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useBoardsStore } from './boards';
import { boardsService } from '../services/boards';

vi.mock('../services/boards', () => ({
  boardsService: {
    getCommunities: vi.fn(),
    joinCommunity: vi.fn(),
    leaveCommunity: vi.fn(),
    getBoards: vi.fn(),
    getBoardPosts: vi.fn(),
    submitBoardPost: vi.fn(),
    deleteBoardPost: vi.fn(),
    syncBoard: vi.fn(),
  },
}));

const mockCommunity = {
  relayPeerId: 'relay-1',
  relayAddress: '/ip4/1.2.3.4/tcp/9000',
  communityName: 'Test Community',
  joinedAt: 1700000000,
  lastSyncAt: null,
};

const mockBoard = {
  boardId: 'board-general',
  relayPeerId: 'relay-1',
  name: 'General',
  description: 'General discussion',
  isDefault: true,
};

const mockBoardPost = {
  postId: 'bp-1',
  boardId: 'board-general',
  relayPeerId: 'relay-1',
  authorPeerId: 'peer-alice',
  authorDisplayName: 'Alice',
  contentType: 'text',
  contentText: 'First board post',
  lamportClock: 1,
  createdAt: 1700000100,
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

describe('useBoardsStore', () => {
  beforeEach(() => {
    useBoardsStore.getState().reset();
    vi.clearAllMocks();
  });

  describe('loadCommunities', () => {
    it('should load communities from backend', async () => {
      vi.mocked(boardsService.getCommunities).mockResolvedValue([mockCommunity]);

      await useBoardsStore.getState().loadCommunities();

      expect(useBoardsStore.getState().communities).toEqual([mockCommunity]);
      expect(useBoardsStore.getState().isLoading).toBe(false);
    });

    it('should handle errors', async () => {
      vi.mocked(boardsService.getCommunities).mockRejectedValue(
        new Error('Load communities failed'),
      );

      await useBoardsStore.getState().loadCommunities();

      expect(useBoardsStore.getState().error).toContain('Load communities failed');
    });
  });

  describe('joinCommunity', () => {
    it('should join and reload communities', async () => {
      vi.mocked(boardsService.joinCommunity).mockResolvedValue(undefined);
      vi.mocked(boardsService.getCommunities).mockResolvedValue([mockCommunity]);

      await useBoardsStore.getState().joinCommunity('/ip4/1.2.3.4/tcp/9000');

      expect(boardsService.joinCommunity).toHaveBeenCalledWith('/ip4/1.2.3.4/tcp/9000');
      expect(useBoardsStore.getState().communities).toEqual([mockCommunity]);
    });

    it('should handle join failure', async () => {
      vi.mocked(boardsService.joinCommunity).mockRejectedValue(new Error('Join failed'));

      await expect(
        useBoardsStore.getState().joinCommunity('/ip4/1.2.3.4/tcp/9000'),
      ).rejects.toThrow('Join failed');

      expect(useBoardsStore.getState().error).toContain('Join failed');
    });
  });

  describe('leaveCommunity', () => {
    it('should leave and reload communities', async () => {
      useBoardsStore.setState({
        communities: [mockCommunity],
        activeCommunity: mockCommunity,
        boards: [mockBoard],
        boardPosts: [mockBoardPost],
        activeBoard: mockBoard,
      });

      vi.mocked(boardsService.leaveCommunity).mockResolvedValue(undefined);
      vi.mocked(boardsService.getCommunities).mockResolvedValue([]);

      await useBoardsStore.getState().leaveCommunity('relay-1');

      const state = useBoardsStore.getState();
      expect(state.communities).toEqual([]);
      expect(state.activeCommunity).toBeNull();
      expect(state.boards).toEqual([]);
      expect(state.boardPosts).toEqual([]);
      expect(state.activeBoard).toBeNull();
    });

    it('should not clear active community if leaving a different one', async () => {
      useBoardsStore.setState({
        communities: [mockCommunity],
        activeCommunity: mockCommunity,
      });

      vi.mocked(boardsService.leaveCommunity).mockResolvedValue(undefined);
      vi.mocked(boardsService.getCommunities).mockResolvedValue([mockCommunity]);

      await useBoardsStore.getState().leaveCommunity('relay-other');

      expect(useBoardsStore.getState().activeCommunity).toEqual(mockCommunity);
    });
  });

  describe('selectCommunity', () => {
    it('should set active community and load boards', async () => {
      vi.mocked(boardsService.getBoards).mockResolvedValue([mockBoard]);
      vi.mocked(boardsService.getBoardPosts).mockResolvedValue([mockBoardPost]);

      await useBoardsStore.getState().selectCommunity(mockCommunity);

      const state = useBoardsStore.getState();
      expect(state.activeCommunity).toEqual(mockCommunity);
      expect(state.boards).toEqual([mockBoard]);
      expect(state.activeBoard).toEqual(mockBoard); // default board auto-selected
    });

    it('should select the default board automatically', async () => {
      const nonDefaultBoard = { ...mockBoard, boardId: 'board-other', isDefault: false };
      const defaultBoard = { ...mockBoard, boardId: 'board-default', isDefault: true };
      vi.mocked(boardsService.getBoards).mockResolvedValue([nonDefaultBoard, defaultBoard]);
      vi.mocked(boardsService.getBoardPosts).mockResolvedValue([]);

      await useBoardsStore.getState().selectCommunity(mockCommunity);

      expect(useBoardsStore.getState().activeBoard?.boardId).toBe('board-default');
    });

    it('keeps the newest community authoritative when an older request resolves last', async () => {
      const communityB = { ...mockCommunity, relayPeerId: 'relay-2', communityName: 'Second' };
      const boardB = { ...mockBoard, relayPeerId: 'relay-2', boardId: 'board-b', name: 'Board B' };
      const postB = {
        ...mockBoardPost,
        relayPeerId: 'relay-2',
        boardId: 'board-b',
        postId: 'bp-b',
      };
      const boardsA = deferred<(typeof mockBoard)[]>();
      vi.mocked(boardsService.getBoards).mockImplementation((relayPeerId) =>
        relayPeerId === 'relay-1' ? boardsA.promise : Promise.resolve([boardB]),
      );
      vi.mocked(boardsService.getBoardPosts).mockResolvedValue([postB]);

      const loadingA = useBoardsStore.getState().selectCommunity(mockCommunity);
      await useBoardsStore.getState().selectCommunity(communityB);
      boardsA.resolve([mockBoard]);
      await loadingA;

      expect(useBoardsStore.getState()).toMatchObject({
        activeCommunity: communityB,
        activeBoard: boardB,
        boards: [boardB],
        boardPosts: [postB],
        isLoading: false,
        error: null,
      });
    });

    it('does not let an old community error replace the current view', async () => {
      const communityB = { ...mockCommunity, relayPeerId: 'relay-2', communityName: 'Second' };
      const boardB = { ...mockBoard, relayPeerId: 'relay-2', boardId: 'board-b' };
      const boardsA = deferred<(typeof mockBoard)[]>();
      vi.mocked(boardsService.getBoards).mockImplementation((relayPeerId) =>
        relayPeerId === 'relay-1' ? boardsA.promise : Promise.resolve([boardB]),
      );
      vi.mocked(boardsService.getBoardPosts).mockResolvedValue([]);

      const loadingA = useBoardsStore.getState().selectCommunity(mockCommunity);
      await useBoardsStore.getState().selectCommunity(communityB);
      boardsA.reject(new Error('stale relay failure'));
      await loadingA;

      expect(useBoardsStore.getState().activeCommunity).toEqual(communityB);
      expect(useBoardsStore.getState().error).toBeNull();
      expect(useBoardsStore.getState().isLoading).toBe(false);
    });
  });

  describe('selectBoard', () => {
    it('should set active board and reset posts', async () => {
      useBoardsStore.setState({
        activeCommunity: mockCommunity,
        boardPosts: [mockBoardPost],
      });
      vi.mocked(boardsService.getBoardPosts).mockResolvedValue([]);

      await useBoardsStore.getState().selectBoard(mockBoard);

      const state = useBoardsStore.getState();
      expect(state.activeBoard).toEqual(mockBoard);
    });

    it('discards posts and errors from a previously selected board', async () => {
      const boardB = { ...mockBoard, boardId: 'board-b', name: 'Board B', isDefault: false };
      const postB = { ...mockBoardPost, boardId: 'board-b', postId: 'bp-b' };
      const postsA = deferred<(typeof mockBoardPost)[]>();
      vi.mocked(boardsService.getBoardPosts).mockImplementation((_relay, boardId) =>
        boardId === mockBoard.boardId ? postsA.promise : Promise.resolve([postB]),
      );
      useBoardsStore.setState({ activeCommunity: mockCommunity });

      const loadingA = useBoardsStore.getState().selectBoard(mockBoard);
      await useBoardsStore.getState().selectBoard(boardB);
      postsA.resolve([mockBoardPost]);
      await loadingA;

      expect(useBoardsStore.getState().activeBoard).toEqual(boardB);
      expect(useBoardsStore.getState().boardPosts).toEqual([postB]);
      expect(useBoardsStore.getState().error).toBeNull();
      expect(useBoardsStore.getState().isLoading).toBe(false);
    });
  });

  describe('loadBoardPosts', () => {
    it('should not load if no active community or board', async () => {
      await useBoardsStore.getState().loadBoardPosts();

      expect(boardsService.getBoardPosts).not.toHaveBeenCalled();
    });

    it('should load posts when community and board are set', async () => {
      useBoardsStore.setState({
        activeCommunity: mockCommunity,
        activeBoard: mockBoard,
      });
      vi.mocked(boardsService.getBoardPosts).mockResolvedValue([mockBoardPost]);

      await useBoardsStore.getState().loadBoardPosts();

      expect(useBoardsStore.getState().boardPosts).toEqual([mockBoardPost]);
    });

    it('does not commit a delayed load after profile teardown', async () => {
      const posts = deferred<(typeof mockBoardPost)[]>();
      useBoardsStore.setState({ activeCommunity: mockCommunity, activeBoard: mockBoard });
      vi.mocked(boardsService.getBoardPosts).mockReturnValue(posts.promise);

      const loading = useBoardsStore.getState().loadBoardPosts();
      useBoardsStore.getState().reset();
      posts.resolve([mockBoardPost]);
      await loading;

      expect(useBoardsStore.getState()).toMatchObject({
        activeCommunity: null,
        activeBoard: null,
        boardPosts: [],
        isLoading: false,
        error: null,
      });
    });
  });

  describe('submitPost', () => {
    it('should submit a post and sync/reload', async () => {
      useBoardsStore.setState({
        activeCommunity: mockCommunity,
        activeBoard: mockBoard,
      });
      vi.mocked(boardsService.submitBoardPost).mockResolvedValue(undefined);
      vi.mocked(boardsService.syncBoard).mockResolvedValue(undefined);
      vi.mocked(boardsService.getBoardPosts).mockResolvedValue([mockBoardPost]);

      await useBoardsStore.getState().submitPost('Hello board!');

      expect(boardsService.submitBoardPost).toHaveBeenCalledWith(
        'relay-1',
        'board-general',
        'Hello board!',
      );
      expect(boardsService.syncBoard).toHaveBeenCalledWith('relay-1', 'board-general');
    });

    it('rejects submission if no active community/board', async () => {
      await expect(useBoardsStore.getState().submitPost('content')).rejects.toThrow(
        'Select a community board',
      );

      expect(boardsService.submitBoardPost).not.toHaveBeenCalled();
    });
  });

  describe('deletePost', () => {
    it('should delete a post and remove from local state', async () => {
      useBoardsStore.setState({
        activeCommunity: mockCommunity,
        boardPosts: [
          mockBoardPost,
          { ...mockBoardPost, postId: 'bp-2', contentText: 'Second post' },
        ],
      });
      vi.mocked(boardsService.deleteBoardPost).mockResolvedValue(undefined);

      await useBoardsStore.getState().deletePost('bp-1');

      const posts = useBoardsStore.getState().boardPosts;
      expect(posts).toHaveLength(1);
      expect(posts[0].postId).toBe('bp-2');
    });

    it('rejects deletion if no active community', async () => {
      await expect(useBoardsStore.getState().deletePost('bp-1')).rejects.toThrow(
        'Select a community',
      );

      expect(boardsService.deleteBoardPost).not.toHaveBeenCalled();
    });
  });

  describe('refreshBoard', () => {
    it('should sync and reload board posts', async () => {
      useBoardsStore.setState({
        activeCommunity: mockCommunity,
        activeBoard: mockBoard,
      });
      vi.mocked(boardsService.syncBoard).mockResolvedValue(undefined);
      vi.mocked(boardsService.getBoardPosts).mockResolvedValue([mockBoardPost]);

      await useBoardsStore.getState().refreshBoard();

      expect(boardsService.syncBoard).toHaveBeenCalled();
      expect(useBoardsStore.getState().boardPosts).toEqual([mockBoardPost]);
    });
  });
});
