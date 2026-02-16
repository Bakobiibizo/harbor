import { describe, it, expect, vi, beforeEach } from 'vitest';
import { feedService } from './feed';
import { invoke } from '@tauri-apps/api/core';

describe('feedService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('getFeed', () => {
    it('should invoke get_feed with limit and beforeTimestamp', async () => {
      vi.mocked(invoke).mockResolvedValue([]);

      await feedService.getFeed(50, 1700000000);

      expect(invoke).toHaveBeenCalledWith('get_feed', {
        limit: 50,
        beforeTimestamp: 1700000000,
      });
    });

    it('should invoke get_feed with optional params as undefined', async () => {
      vi.mocked(invoke).mockResolvedValue([]);

      await feedService.getFeed();

      expect(invoke).toHaveBeenCalledWith('get_feed', {
        limit: undefined,
        beforeTimestamp: undefined,
      });
    });
  });

  describe('getWall', () => {
    it('should invoke get_wall with authorPeerId and pagination', async () => {
      vi.mocked(invoke).mockResolvedValue([]);

      await feedService.getWall('peer-alice', 25, 1700000000);

      expect(invoke).toHaveBeenCalledWith('get_wall', {
        authorPeerId: 'peer-alice',
        limit: 25,
        beforeTimestamp: 1700000000,
      });
    });
  });
});
