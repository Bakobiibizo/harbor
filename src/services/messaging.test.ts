import { describe, it, expect, vi, beforeEach } from 'vitest';
import { messagingService } from './messaging';
import { invoke } from '@tauri-apps/api/core';

describe('messagingService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('sendMessage', () => {
    it('should invoke send_message with required args', async () => {
      const mockResult = { messageId: 'msg-1', conversationId: 'conv-1', sentAt: 1700000000 };
      vi.mocked(invoke).mockResolvedValue(mockResult);

      const result = await messagingService.sendMessage('peer-alice', 'Hello!');

      expect(invoke).toHaveBeenCalledWith('send_message', {
        peerId: 'peer-alice',
        content: 'Hello!',
        contentType: undefined,
        replyTo: undefined,
      });
      expect(result).toEqual(mockResult);
    });

    it('should pass optional contentType and replyTo', async () => {
      vi.mocked(invoke).mockResolvedValue({});

      await messagingService.sendMessage('peer-alice', 'Image', 'image', 'msg-reply');

      expect(invoke).toHaveBeenCalledWith('send_message', {
        peerId: 'peer-alice',
        content: 'Image',
        contentType: 'image',
        replyTo: 'msg-reply',
      });
    });
  });

  describe('getMessages', () => {
    it('should invoke get_messages with peerId and pagination', async () => {
      vi.mocked(invoke).mockResolvedValue([]);

      await messagingService.getMessages('peer-alice', 100, 1700000000);

      expect(invoke).toHaveBeenCalledWith('get_messages', {
        peerId: 'peer-alice',
        limit: 100,
        beforeTimestamp: 1700000000,
      });
    });
  });

  describe('getConversations', () => {
    it('should invoke get_conversations', async () => {
      vi.mocked(invoke).mockResolvedValue([]);

      await messagingService.getConversations();

      expect(invoke).toHaveBeenCalledWith('get_conversations');
    });
  });

  describe('markConversationRead', () => {
    it('should invoke mark_conversation_read', async () => {
      vi.mocked(invoke).mockResolvedValue(3);

      const result = await messagingService.markConversationRead('peer-alice');

      expect(invoke).toHaveBeenCalledWith('mark_conversation_read', { peerId: 'peer-alice' });
      expect(result).toBe(3);
    });
  });

  describe('getUnreadCount', () => {
    it('should invoke get_unread_count', async () => {
      vi.mocked(invoke).mockResolvedValue(5);

      const result = await messagingService.getUnreadCount('peer-alice');

      expect(invoke).toHaveBeenCalledWith('get_unread_count', { peerId: 'peer-alice' });
      expect(result).toBe(5);
    });
  });

  describe('getTotalUnreadCount', () => {
    it('should invoke get_total_unread_count', async () => {
      vi.mocked(invoke).mockResolvedValue(12);

      const result = await messagingService.getTotalUnreadCount();

      expect(invoke).toHaveBeenCalledWith('get_total_unread_count');
      expect(result).toBe(12);
    });
  });
});
