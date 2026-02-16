import { create } from 'zustand';
import { boardsService } from '../services/boards';
import type { CommunityInfo, BoardInfo, BoardPost } from '../types/boards';

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
}

export const useBoardsStore = create<BoardsState>((set, get) => ({
  // Initial state
  communities: [],
  boards: [],
  boardPosts: [],
  activeCommunity: null,
  activeBoard: null,
  isLoading: false,
  error: null,
  hasMore: true,

  loadCommunities: async () => {
    set({ isLoading: true, error: null });
    try {
      const communities = await boardsService.getCommunities();
      set({ communities, isLoading: false });
    } catch (error) {
      console.error('Failed to load communities:', error);
      set({ error: String(error), isLoading: false });
    }
  },

  joinCommunity: async (relayAddress: string) => {
    set({ isLoading: true, error: null });
    try {
      await boardsService.joinCommunity(relayAddress);
      // Reload communities list
      const communities = await boardsService.getCommunities();
      set({ communities, isLoading: false });
    } catch (error) {
      console.error('Failed to join community:', error);
      set({ error: String(error), isLoading: false });
      throw error;
    }
  },

  leaveCommunity: async (relayPeerId: string) => {
    try {
      await boardsService.leaveCommunity(relayPeerId);
      const { activeCommunity } = get();
      const communities = await boardsService.getCommunities();
      set({
        communities,
        // Clear active community if it was the one we left
        ...(activeCommunity?.relayPeerId === relayPeerId
          ? { activeCommunity: null, boards: [], boardPosts: [], activeBoard: null }
          : {}),
      });
    } catch (error) {
      console.error('Failed to leave community:', error);
      set({ error: String(error) });
      throw error;
    }
  },

  selectCommunity: async (community: CommunityInfo) => {
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
      const defaultBoard = boards.find((b) => b.isDefault) || boards[0] || null;
      set({ boards, activeBoard: defaultBoard, isLoading: false });

      // Auto-load posts for the default board
      if (defaultBoard) {
        get().loadBoardPosts();
      }
    } catch (error) {
      console.error('Failed to load boards:', error);
      set({ error: String(error), isLoading: false });
    }
  },

  selectBoard: async (board: BoardInfo) => {
    set({ activeBoard: board, boardPosts: [], hasMore: true });
    get().loadBoardPosts();
  },

  loadBoardPosts: async (limit: number = 50) => {
    const { activeCommunity, activeBoard } = get();
    if (!activeCommunity || !activeBoard) return;

    set({ isLoading: true, error: null });
    try {
      const posts = await boardsService.getBoardPosts(
        activeCommunity.relayPeerId,
        activeBoard.boardId,
        limit,
      );
      set({
        boardPosts: posts,
        isLoading: false,
        hasMore: posts.length === limit,
      });
    } catch (error) {
      console.error('Failed to load board posts:', error);
      set({ error: String(error), isLoading: false });
    }
  },

  loadMorePosts: async (limit: number = 50) => {
    const { activeCommunity, activeBoard, boardPosts, isLoading, hasMore } = get();
    if (!activeCommunity || !activeBoard || isLoading || !hasMore) return;

    set({ isLoading: true });
    try {
      const lastPost = boardPosts[boardPosts.length - 1];
      const posts = await boardsService.getBoardPosts(
        activeCommunity.relayPeerId,
        activeBoard.boardId,
        limit,
        lastPost?.createdAt,
      );
      set({
        boardPosts: [...boardPosts, ...posts],
        isLoading: false,
        hasMore: posts.length === limit,
      });
    } catch (error) {
      console.error('Failed to load more posts:', error);
      set({ error: String(error), isLoading: false });
    }
  },

  submitPost: async (contentText: string) => {
    const { activeCommunity, activeBoard } = get();
    if (!activeCommunity || !activeBoard) return;

    try {
      await boardsService.submitBoardPost(
        activeCommunity.relayPeerId,
        activeBoard.boardId,
        contentText,
      );
      // Sync and reload after posting
      await boardsService.syncBoard(activeCommunity.relayPeerId, activeBoard.boardId);
      get().loadBoardPosts();
    } catch (error) {
      console.error('Failed to submit post:', error);
      set({ error: String(error) });
      throw error;
    }
  },

  deletePost: async (postId: string) => {
    const { activeCommunity } = get();
    if (!activeCommunity) return;

    try {
      await boardsService.deleteBoardPost(activeCommunity.relayPeerId, postId);
      // Remove from local state
      set((state) => ({
        boardPosts: state.boardPosts.filter((p) => p.postId !== postId),
      }));
    } catch (error) {
      console.error('Failed to delete post:', error);
      set({ error: String(error) });
      throw error;
    }
  },

  refreshBoard: async () => {
    const { activeCommunity, activeBoard } = get();
    if (!activeCommunity || !activeBoard) return;

    set({ isLoading: true, error: null });
    try {
      await boardsService.syncBoard(activeCommunity.relayPeerId, activeBoard.boardId);
      const posts = await boardsService.getBoardPosts(
        activeCommunity.relayPeerId,
        activeBoard.boardId,
        50,
      );
      set({
        boardPosts: posts,
        isLoading: false,
        hasMore: posts.length === 50,
      });
    } catch (error) {
      console.error('Failed to refresh board:', error);
      set({ error: String(error), isLoading: false });
    }
  },
}));
