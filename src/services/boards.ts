import { publishingPolicy } from './publishingPolicy';
import { invokeCommand } from './command';
import type { CommunityInfo, BoardInfo, BoardPost } from '../types/boards';

/** Boards service - wraps Tauri commands for community board functionality */
export const boardsService = {
  /** Get all joined communities */
  async getCommunities(): Promise<CommunityInfo[]> {
    return invokeCommand('get_communities');
  },

  /** Join a community by relay address */
  async joinCommunity(relayAddress: string): Promise<void> {
    return invokeCommand('join_community', { relayAddress });
  },

  /** Leave a community */
  async leaveCommunity(relayPeerId: string): Promise<void> {
    return invokeCommand('leave_community', { relayPeerId });
  },

  /** Get boards for a community */
  async getBoards(relayPeerId: string): Promise<BoardInfo[]> {
    return invokeCommand('get_boards', { relayPeerId });
  },

  /** Get posts for a board */
  async getBoardPosts(
    relayPeerId: string,
    boardId: string,
    limit?: number,
    beforeTimestamp?: number,
  ): Promise<BoardPost[]> {
    return invokeCommand('get_board_posts', {
      relayPeerId,
      boardId,
      limit,
      beforeTimestamp,
    });
  },

  /** Submit a post to a board */
  async submitBoardPost(relayPeerId: string, boardId: string, contentText: string): Promise<void> {
    publishingPolicy.assertAllowed();
    return invokeCommand('submit_board_post', {
      relayPeerId,
      boardId,
      contentText,
    });
  },

  /** Delete a board post */
  async deleteBoardPost(relayPeerId: string, postId: string): Promise<void> {
    return invokeCommand('delete_board_post', { relayPeerId, postId });
  },

  /** Sync a board (fetch latest from relay) */
  async syncBoard(relayPeerId: string, boardId: string): Promise<void> {
    return invokeCommand('sync_board', { relayPeerId, boardId });
  },
};
