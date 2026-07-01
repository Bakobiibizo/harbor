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

  describe('wall preview and RSS helpers', () => {
    it('should request guest wall preview for public-only filtering', async () => {
      vi.mocked(invoke).mockResolvedValue([]);

      await feedService.getWallPreview('guest', 10, 1700000000);

      expect(invoke).toHaveBeenCalledWith('get_wall_preview', {
        perspective: 'guest',
        limit: 10,
        beforeTimestamp: 1700000000,
      });
    });

    it('should request wall visibility stats', async () => {
      const stats = {
        totalPosts: 2,
        publicPosts: 1,
        contactsOnlyPosts: 1,
        guestVisible: 1,
        contactVisible: 2,
      };
      vi.mocked(invoke).mockResolvedValue(stats);

      await expect(feedService.getWallVisibilityStats()).resolves.toEqual(stats);

      expect(invoke).toHaveBeenCalledWith('get_wall_visibility_stats');
    });

    it('should invoke RSS generation for backend public-only output', async () => {
      vi.mocked(invoke).mockResolvedValue('<rss />');

      await feedService.generateRssFeed({
        base_url: 'harbor://peer/me',
        title: 'My Wall',
        description: 'Public posts',
        max_items: 20,
      });

      expect(invoke).toHaveBeenCalledWith('generate_rss_feed', {
        config: {
          base_url: 'harbor://peer/me',
          title: 'My Wall',
          description: 'Public posts',
          max_items: 20,
        },
      });
    });

    it('should request the shareable Harbor feed URI without treating it as hosted RSS', async () => {
      vi.mocked(invoke).mockResolvedValue('harbor://feed/peer-me');

      await expect(feedService.getRssFeedUrl()).resolves.toBe('harbor://feed/peer-me');

      expect(invoke).toHaveBeenCalledWith('get_rss_feed_url');
    });
  });
});
