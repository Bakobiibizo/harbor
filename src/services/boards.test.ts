import { describe, it, expect, vi, beforeEach } from 'vitest';
import { boardsService } from './boards';
import { invoke } from '@tauri-apps/api/core';

describe('boardsService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('getCommunities', () => {
    it('should invoke get_communities', async () => {
      vi.mocked(invoke).mockResolvedValue([]);

      const result = await boardsService.getCommunities();

      expect(invoke).toHaveBeenCalledWith('get_communities');
      expect(result).toEqual([]);
    });
  });

  describe('joinCommunity', () => {
    it('should invoke join_community with relayAddress', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);

      await boardsService.joinCommunity('/ip4/1.2.3.4/tcp/9000');

      expect(invoke).toHaveBeenCalledWith('join_community', {
        relayAddress: '/ip4/1.2.3.4/tcp/9000',
      });
    });
  });

  describe('leaveCommunity', () => {
    it('should invoke leave_community with relayPeerId', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);

      await boardsService.leaveCommunity('relay-1');

      expect(invoke).toHaveBeenCalledWith('leave_community', { relayPeerId: 'relay-1' });
    });
  });

  describe('getBoards', () => {
    it('should invoke get_boards with relayPeerId', async () => {
      vi.mocked(invoke).mockResolvedValue([]);

      await boardsService.getBoards('relay-1');

      expect(invoke).toHaveBeenCalledWith('get_boards', { relayPeerId: 'relay-1' });
    });
  });

  describe('getBoardPosts', () => {
    it('should invoke get_board_posts with all parameters', async () => {
      vi.mocked(invoke).mockResolvedValue([]);

      await boardsService.getBoardPosts('relay-1', 'board-general', 50, 1700000000);

      expect(invoke).toHaveBeenCalledWith('get_board_posts', {
        relayPeerId: 'relay-1',
        boardId: 'board-general',
        limit: 50,
        beforeTimestamp: 1700000000,
      });
    });
  });

  describe('submitBoardPost', () => {
    it('should invoke submit_board_post', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);

      await boardsService.submitBoardPost('relay-1', 'board-general', 'Hello board!');

      expect(invoke).toHaveBeenCalledWith('submit_board_post', {
        relayPeerId: 'relay-1',
        boardId: 'board-general',
        contentText: 'Hello board!',
      });
    });
  });

  describe('deleteBoardPost', () => {
    it('should invoke delete_board_post', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);

      await boardsService.deleteBoardPost('relay-1', 'bp-1');

      expect(invoke).toHaveBeenCalledWith('delete_board_post', {
        relayPeerId: 'relay-1',
        postId: 'bp-1',
      });
    });
  });

  describe('syncBoard', () => {
    it('should invoke sync_board', async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);

      await boardsService.syncBoard('relay-1', 'board-general');

      expect(invoke).toHaveBeenCalledWith('sync_board', {
        relayPeerId: 'relay-1',
        boardId: 'board-general',
      });
    });
  });
});
