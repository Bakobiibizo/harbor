import { create } from 'zustand';
import { boardsService } from '../services/boards';
import type { CommunityInfo, BoardInfo, BoardPost } from '../types/boards';
import { getErrorMessage } from '../utils/errors';

interface BoardsState {
  // State
  communities: CommunityInfo[];
  boards: BoardInfo[];
  boardPosts: BoardPost[];
  activeCommunity: CommunityInfo | null;
  activeBoard: BoardInfo | null;
  isLoading: boolean;
  error: string | null;
  hasMore: boolean;

  // Actions
  loadCommunities: () => Promise<void>;
  joinCommunity: (relayAddress: string) => Promise<void>;
  leaveCommunity: (relayPeerId: string) => Promise<void>;
  selectCommunity: (community: CommunityInfo) => Promise<void>;
  selectBoard: (board: BoardInfo) => Promise<void>;
  loadBoardPosts: (limit?: number) => Promise<void>;
  loadMorePosts: (limit?: number) => Promise<void>;
  submitPost: (contentText: string) => Promise<void>;
  deletePost: (postId: string) => Promise<void>;
  refreshBoard: () => Promise<void>;
  reset: () => void;
}

let lifecycleGeneration = 0;
let selectionGeneration = 0;
let requestGeneration = 0;

function isCurrentRequest(lifecycle: number, request: number): boolean {
  return lifecycle === lifecycleGeneration && request === requestGeneration;
}

function isCurrentSelection(
  get: () => BoardsState,
  lifecycle: number,
  selection: number,
  relayPeerId: string,
  boardId?: string,
): boolean {
  const state = get();
  return (
    lifecycle === lifecycleGeneration &&
    selection === selectionGeneration &&
    state.activeCommunity?.relayPeerId === relayPeerId &&
    (boardId === undefined || state.activeBoard?.boardId === boardId)
  );
}

const initialState = {
  communities: [] as CommunityInfo[],
  boards: [] as BoardInfo[],
  boardPosts: [] as BoardPost[],
  activeCommunity: null as CommunityInfo | null,
  activeBoard: null as BoardInfo | null,
  isLoading: false,
  error: null as string | null,
  hasMore: true,
};

export const useBoardsStore = create<BoardsState>((set, get) => ({
  ...initialState,

  loadCommunities: async () => {
    const generation = lifecycleGeneration;
    const request = ++requestGeneration;
    set({ isLoading: true, error: null });
    try {
      const communities = await boardsService.getCommunities();
      if (isCurrentRequest(generation, request)) set({ communities, isLoading: false });
    } catch (error) {
      if (!isCurrentRequest(generation, request)) return;
      console.error('Failed to load communities:', error);
      set({ error: getErrorMessage(error), isLoading: false });
    }
  },

  joinCommunity: async (relayAddress: string) => {
    const generation = lifecycleGeneration;
    const request = ++requestGeneration;
    set({ isLoading: true, error: null });
    try {
      await boardsService.joinCommunity(relayAddress);
      if (!isCurrentRequest(generation, request)) return;
      // Reload communities list
      const communities = await boardsService.getCommunities();
      if (!isCurrentRequest(generation, request)) return;
      set({ communities, isLoading: false });
    } catch (error) {
      if (isCurrentRequest(generation, request)) {
        console.error('Failed to join community:', error);
        set({ error: getErrorMessage(error), isLoading: false });
      }
      throw error;
    }
  },

  leaveCommunity: async (relayPeerId: string) => {
    const generation = lifecycleGeneration;
    const selection = selectionGeneration;
    const request = ++requestGeneration;
    try {
      await boardsService.leaveCommunity(relayPeerId);
      if (!isCurrentRequest(generation, request)) return;
      const { activeCommunity } = get();
      const communities = await boardsService.getCommunities();
      if (!isCurrentRequest(generation, request)) return;
      const leavingActiveCommunity =
        selection === selectionGeneration && activeCommunity?.relayPeerId === relayPeerId;
      if (leavingActiveCommunity) selectionGeneration += 1;
      set({
        communities,
        // Clear active community if it was the one we left
        ...(leavingActiveCommunity
          ? { activeCommunity: null, boards: [], boardPosts: [], activeBoard: null }
          : {}),
      });
    } catch (error) {
      if (isCurrentRequest(generation, request)) {
        console.error('Failed to leave community:', error);
        set({ error: getErrorMessage(error) });
      }
      throw error;
    }
  },

  selectCommunity: async (community: CommunityInfo) => {
    const generation = lifecycleGeneration;
    const selection = ++selectionGeneration;
    const request = ++requestGeneration;
    set({
      activeCommunity: community,
      activeBoard: null,
      boards: [],
      boardPosts: [],
      isLoading: true,
      error: null,
    });
    try {
      const boards = await boardsService.getBoards(community.relayPeerId);
      if (
        !isCurrentRequest(generation, request) ||
        !isCurrentSelection(get, generation, selection, community.relayPeerId)
      )
        return;
      const defaultBoard = boards.find((b) => b.isDefault) || boards[0] || null;
      set({ boards, activeBoard: defaultBoard, isLoading: false });

      // Auto-load posts for the default board
      if (defaultBoard) {
        await get().loadBoardPosts();
      }
    } catch (error) {
      if (
        !isCurrentRequest(generation, request) ||
        !isCurrentSelection(get, generation, selection, community.relayPeerId)
      )
        return;
      console.error('Failed to load boards:', error);
      set({ error: getErrorMessage(error), isLoading: false });
    }
  },

  selectBoard: async (board: BoardInfo) => {
    selectionGeneration += 1;
    requestGeneration += 1;
    set({ activeBoard: board, boardPosts: [], hasMore: true });
    await get().loadBoardPosts();
  },

  loadBoardPosts: async (limit: number = 50) => {
    const generation = lifecycleGeneration;
    const { activeCommunity, activeBoard } = get();
    if (!activeCommunity || !activeBoard) return;
    const selection = selectionGeneration;
    const request = ++requestGeneration;

    set({ isLoading: true, error: null });
    try {
      const posts = await boardsService.getBoardPosts(
        activeCommunity.relayPeerId,
        activeBoard.boardId,
        limit,
      );
      if (
        !isCurrentRequest(generation, request) ||
        !isCurrentSelection(
          get,
          generation,
          selection,
          activeCommunity.relayPeerId,
          activeBoard.boardId,
        )
      )
        return;
      set({
        boardPosts: posts,
        isLoading: false,
        hasMore: posts.length === limit,
      });
    } catch (error) {
      if (
        !isCurrentRequest(generation, request) ||
        !isCurrentSelection(
          get,
          generation,
          selection,
          activeCommunity.relayPeerId,
          activeBoard.boardId,
        )
      )
        return;
      console.error('Failed to load board posts:', error);
      set({ error: getErrorMessage(error), isLoading: false });
    }
  },

  loadMorePosts: async (limit: number = 50) => {
    const generation = lifecycleGeneration;
    const { activeCommunity, activeBoard, boardPosts, isLoading, hasMore } = get();
    if (!activeCommunity || !activeBoard || isLoading || !hasMore) return;
    const selection = selectionGeneration;
    const request = ++requestGeneration;

    set({ isLoading: true });
    try {
      const lastPost = boardPosts[boardPosts.length - 1];
      const posts = await boardsService.getBoardPosts(
        activeCommunity.relayPeerId,
        activeBoard.boardId,
        limit,
        lastPost?.createdAt,
      );
      if (
        !isCurrentRequest(generation, request) ||
        !isCurrentSelection(
          get,
          generation,
          selection,
          activeCommunity.relayPeerId,
          activeBoard.boardId,
        )
      )
        return;
      set({
        boardPosts: [...boardPosts, ...posts],
        isLoading: false,
        hasMore: posts.length === limit,
      });
    } catch (error) {
      if (
        !isCurrentRequest(generation, request) ||
        !isCurrentSelection(
          get,
          generation,
          selection,
          activeCommunity.relayPeerId,
          activeBoard.boardId,
        )
      )
        return;
      console.error('Failed to load more posts:', error);
      set({ error: getErrorMessage(error), isLoading: false });
    }
  },

  submitPost: async (contentText: string) => {
    const generation = lifecycleGeneration;
    const { activeCommunity, activeBoard } = get();
    if (!activeCommunity || !activeBoard) {
      throw new Error('Select a community board before posting.');
    }
    const selection = selectionGeneration;
    const request = ++requestGeneration;

    try {
      await boardsService.submitBoardPost(
        activeCommunity.relayPeerId,
        activeBoard.boardId,
        contentText,
      );
      if (
        !isCurrentRequest(generation, request) ||
        !isCurrentSelection(
          get,
          generation,
          selection,
          activeCommunity.relayPeerId,
          activeBoard.boardId,
        )
      )
        return;
      // Sync and reload after posting
      await boardsService.syncBoard(activeCommunity.relayPeerId, activeBoard.boardId);
      if (!isCurrentRequest(generation, request)) return;
      await get().loadBoardPosts();
    } catch (error) {
      if (
        isCurrentRequest(generation, request) &&
        isCurrentSelection(
          get,
          generation,
          selection,
          activeCommunity.relayPeerId,
          activeBoard.boardId,
        )
      ) {
        console.error('Failed to submit post:', error);
        set({ error: getErrorMessage(error) });
      }
      throw error;
    }
  },

  deletePost: async (postId: string) => {
    const generation = lifecycleGeneration;
    const { activeCommunity } = get();
    if (!activeCommunity) throw new Error('Select a community before deleting a post.');
    const selection = selectionGeneration;
    const request = ++requestGeneration;

    try {
      await boardsService.deleteBoardPost(activeCommunity.relayPeerId, postId);
      if (
        !isCurrentRequest(generation, request) ||
        !isCurrentSelection(get, generation, selection, activeCommunity.relayPeerId)
      )
        return;
      // Remove from local state
      set((state) => ({
        boardPosts: state.boardPosts.filter((p) => p.postId !== postId),
      }));
    } catch (error) {
      if (
        isCurrentRequest(generation, request) &&
        isCurrentSelection(get, generation, selection, activeCommunity.relayPeerId)
      ) {
        console.error('Failed to delete post:', error);
        set({ error: getErrorMessage(error) });
      }
      throw error;
    }
  },

  refreshBoard: async () => {
    const generation = lifecycleGeneration;
    const { activeCommunity, activeBoard } = get();
    if (!activeCommunity || !activeBoard) return;
    const selection = selectionGeneration;
    const request = ++requestGeneration;

    set({ isLoading: true, error: null });
    try {
      await boardsService.syncBoard(activeCommunity.relayPeerId, activeBoard.boardId);
      if (
        !isCurrentRequest(generation, request) ||
        !isCurrentSelection(
          get,
          generation,
          selection,
          activeCommunity.relayPeerId,
          activeBoard.boardId,
        )
      )
        return;
      const posts = await boardsService.getBoardPosts(
        activeCommunity.relayPeerId,
        activeBoard.boardId,
        50,
      );
      if (
        !isCurrentRequest(generation, request) ||
        !isCurrentSelection(
          get,
          generation,
          selection,
          activeCommunity.relayPeerId,
          activeBoard.boardId,
        )
      )
        return;
      set({
        boardPosts: posts,
        isLoading: false,
        hasMore: posts.length === 50,
      });
    } catch (error) {
      if (
        !isCurrentRequest(generation, request) ||
        !isCurrentSelection(
          get,
          generation,
          selection,
          activeCommunity.relayPeerId,
          activeBoard.boardId,
        )
      )
        return;
      console.error('Failed to refresh board:', error);
      set({ error: getErrorMessage(error), isLoading: false });
    }
  },
  reset: () => {
    lifecycleGeneration += 1;
    selectionGeneration += 1;
    requestGeneration += 1;
    set({ ...initialState, communities: [], boards: [], boardPosts: [] });
  },
}));
