import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useMessagingStore } from './messaging';
import { invoke } from '@tauri-apps/api/core';

describe('useMessagingStore', () => {
  beforeEach(() => {
    useMessagingStore.setState({
      conversations: [],
      messages: {},
      activeConversation: null,
      selectedConversationId: null,
      archivedConversations: [],
      isLoading: false,
      error: null,
    });
    vi.clearAllMocks();
  });

  describe('loadConversations', () => {
    it('should load conversations from backend', async () => {
      const mockConversations = [
        {
          conversationId: 'conv-1',
          peerId: 'peer-alice',
          lastMessage: 'Hello',
          lastMessageAt: 1700000100,
          unreadCount: 2,
        },
      ];
      vi.mocked(invoke).mockResolvedValue(mockConversations);

      await useMessagingStore.getState().loadConversations();

      expect(invoke).toHaveBeenCalledWith('get_conversations');
      expect(useMessagingStore.getState().conversations).toEqual(mockConversations);
      expect(useMessagingStore.getState().isLoading).toBe(false);
    });

    it('should handle load errors', async () => {
      vi.mocked(invoke).mockRejectedValue(new Error('Load failed'));

      await useMessagingStore.getState().loadConversations();

      expect(useMessagingStore.getState().error).toContain('Load failed');
      expect(useMessagingStore.getState().isLoading).toBe(false);
    });
  });

  describe('loadMessages', () => {
    it('should load messages for a peer', async () => {
      const mockMessages = [
        {
          messageId: 'msg-1',
          conversationId: 'conv-1',
          senderPeerId: 'peer-alice',
          recipientPeerId: 'peer-me',
          content: 'Hello!',
          contentType: 'text',
          replyToMessageId: null,
          sentAt: 1700000100,
          deliveredAt: null,
          readAt: null,
          status: 'delivered' as const,
          isOutgoing: false,
          editedAt: null,
        },
      ];
      vi.mocked(invoke).mockResolvedValue(mockMessages);

      await useMessagingStore.getState().loadMessages('peer-alice');

      expect(invoke).toHaveBeenCalledWith('get_messages', { peerId: 'peer-alice', limit: 100 });
      expect(useMessagingStore.getState().messages['peer-alice']).toEqual(mockMessages);
    });

    it('should handle message load errors', async () => {
      vi.mocked(invoke).mockRejectedValue(new Error('Messages error'));

      await useMessagingStore.getState().loadMessages('peer-alice');

      expect(useMessagingStore.getState().error).toContain('Messages error');
    });
  });

  describe('sendMessage', () => {
    it('should send a message and add it to local state', async () => {
      const sendResult = {
        messageId: 'msg-new',
        conversationId: 'conv-1',
        sentAt: 1700000200,
      };
      // First call: send_message, subsequent calls: get_conversations (from loadConversations)
      vi.mocked(invoke).mockResolvedValueOnce(sendResult).mockResolvedValueOnce([]); // loadConversations

      const result = await useMessagingStore.getState().sendMessage('peer-alice', 'Hi Alice!');

      expect(result).toEqual(sendResult);
      expect(invoke).toHaveBeenCalledWith('send_message', {
        peerId: 'peer-alice',
        content: 'Hi Alice!',
        contentType: 'text',
      });

      const messages = useMessagingStore.getState().messages['peer-alice'];
      expect(messages).toHaveLength(1);
      expect(messages[0].content).toBe('Hi Alice!');
      expect(messages[0].isOutgoing).toBe(true);
    });

    it('should throw on send failure', async () => {
      vi.mocked(invoke).mockRejectedValue(new Error('Send failed'));

      await expect(useMessagingStore.getState().sendMessage('peer-alice', 'Hi!')).rejects.toThrow(
        'Send failed',
      );
    });
  });

  describe('setActiveConversation', () => {
    it('should set active conversation and load messages', async () => {
      vi.mocked(invoke).mockResolvedValue([]);

      useMessagingStore.getState().setActiveConversation('peer-alice');

      expect(useMessagingStore.getState().activeConversation).toBe('peer-alice');
      expect(invoke).toHaveBeenCalledWith('get_messages', { peerId: 'peer-alice', limit: 100 });
    });

    it('should clear active conversation when null', () => {
      useMessagingStore.setState({ activeConversation: 'peer-alice' });

      useMessagingStore.getState().setActiveConversation(null);

      expect(useMessagingStore.getState().activeConversation).toBeNull();
    });
  });

  describe('setSelectedConversation', () => {
    it('should update selectedConversationId', () => {
      useMessagingStore.getState().setSelectedConversation('conv-1');
      expect(useMessagingStore.getState().selectedConversationId).toBe('conv-1');
    });
  });

  describe('clearConversationSelection', () => {
    it('should clear both selected and active conversation', () => {
      useMessagingStore.setState({
        selectedConversationId: 'conv-1',
        activeConversation: 'peer-alice',
      });

      useMessagingStore.getState().clearConversationSelection();

      expect(useMessagingStore.getState().selectedConversationId).toBeNull();
      expect(useMessagingStore.getState().activeConversation).toBeNull();
    });
  });

  describe('handleIncomingMessage', () => {
    it('should add incoming message and refresh conversations', () => {
      vi.mocked(invoke).mockResolvedValue([]);

      const message = {
        messageId: 'msg-incoming',
        conversationId: 'conv-1',
        senderPeerId: 'peer-alice',
        recipientPeerId: 'peer-me',
        content: 'Hey there!',
        contentType: 'text',
        replyToMessageId: null,
        sentAt: 1700000300,
        deliveredAt: null,
        readAt: null,
        status: 'delivered' as const,
        isOutgoing: false,
        editedAt: null,
      };

      useMessagingStore.getState().handleIncomingMessage(message);

      const messages = useMessagingStore.getState().messages['peer-alice'];
      expect(messages).toHaveLength(1);
      expect(messages[0].content).toBe('Hey there!');
    });

    it('should append to existing messages for a peer', () => {
      useMessagingStore.setState({
        messages: {
          'peer-alice': [
            {
              messageId: 'msg-1',
              conversationId: 'conv-1',
              senderPeerId: 'peer-alice',
              recipientPeerId: 'peer-me',
              content: 'First message',
              contentType: 'text',
              replyToMessageId: null,
              sentAt: 1700000100,
              deliveredAt: null,
              readAt: null,
              status: 'delivered' as const,
              isOutgoing: false,
              editedAt: null,
            },
          ],
        },
      });
      vi.mocked(invoke).mockResolvedValue([]);

      const newMessage = {
        messageId: 'msg-2',
        conversationId: 'conv-1',
        senderPeerId: 'peer-alice',
        recipientPeerId: 'peer-me',
        content: 'Second message',
        contentType: 'text',
        replyToMessageId: null,
        sentAt: 1700000200,
        deliveredAt: null,
        readAt: null,
        status: 'delivered' as const,
        isOutgoing: false,
        editedAt: null,
      };

      useMessagingStore.getState().handleIncomingMessage(newMessage);

      expect(useMessagingStore.getState().messages['peer-alice']).toHaveLength(2);
    });
  });

  describe('archiveConversation / unarchiveConversation', () => {
    it('should archive a conversation', () => {
      useMessagingStore.getState().archiveConversation('peer-alice');

      expect(useMessagingStore.getState().archivedConversations).toContain('peer-alice');
    });

    it('should unarchive a conversation', () => {
      useMessagingStore.setState({ archivedConversations: ['peer-alice', 'peer-bob'] });

      useMessagingStore.getState().unarchiveConversation('peer-alice');

      expect(useMessagingStore.getState().archivedConversations).toEqual(['peer-bob']);
    });
  });

  describe('isArchived', () => {
    it('should return true for archived conversations', () => {
      useMessagingStore.setState({ archivedConversations: ['peer-alice'] });

      expect(useMessagingStore.getState().isArchived('peer-alice')).toBe(true);
    });

    it('should return false for non-archived conversations', () => {
      expect(useMessagingStore.getState().isArchived('peer-alice')).toBe(false);
    });
  });

  describe('editMessage', () => {
    it('should update message content and set editedAt', async () => {
      useMessagingStore.setState({
        messages: {
          'peer-alice': [
            {
              messageId: 'msg-1',
              conversationId: 'conv-1',
              senderPeerId: 'peer-me',
              recipientPeerId: 'peer-alice',
              content: 'Original',
              contentType: 'text',
              replyToMessageId: null,
              sentAt: 1700000100,
              deliveredAt: null,
              readAt: null,
              status: 'sent' as const,
              isOutgoing: true,
              editedAt: null,
            },
          ],
        },
      });

      vi.mocked(invoke).mockResolvedValue(undefined);

      await useMessagingStore.getState().editMessage('msg-1', 'Edited content', 'peer-alice');

      const msg = useMessagingStore.getState().messages['peer-alice'][0];
      expect(msg.content).toBe('Edited content');
      expect(msg.editedAt).not.toBeNull();
    });

    it('should throw on edit failure', async () => {
      useMessagingStore.setState({
        messages: {
          'peer-alice': [
            {
              messageId: 'msg-1',
              conversationId: 'conv-1',
              senderPeerId: 'peer-me',
              recipientPeerId: 'peer-alice',
              content: 'Original',
              contentType: 'text',
              replyToMessageId: null,
              sentAt: 1700000100,
              deliveredAt: null,
              readAt: null,
              status: 'sent' as const,
              isOutgoing: true,
              editedAt: null,
            },
          ],
        },
      });

      vi.mocked(invoke).mockRejectedValue(new Error('Edit failed'));

      await expect(
        useMessagingStore.getState().editMessage('msg-1', 'New content', 'peer-alice'),
      ).rejects.toThrow('Edit failed');
    });
  });

  describe('clearConversationHistory', () => {
    it('should clear messages and refresh conversations', async () => {
      useMessagingStore.setState({
        messages: {
          'peer-alice': [
            {
              messageId: 'msg-1',
              conversationId: 'conv-1',
              senderPeerId: 'peer-alice',
              recipientPeerId: 'peer-me',
              content: 'message',
              contentType: 'text',
              replyToMessageId: null,
              sentAt: 1700000100,
              deliveredAt: null,
              readAt: null,
              status: 'delivered' as const,
              isOutgoing: false,
              editedAt: null,
            },
          ],
        },
      });

      vi.mocked(invoke)
        .mockResolvedValueOnce(undefined) // clear_conversation_history
        .mockResolvedValueOnce([]); // get_conversations

      await useMessagingStore.getState().clearConversationHistory('peer-alice');

      expect(useMessagingStore.getState().messages['peer-alice']).toBeUndefined();
    });
  });

  describe('deleteConversation', () => {
    it('should remove conversation and clear related state', async () => {
      useMessagingStore.setState({
        messages: {
          'peer-alice': [
            {
              messageId: 'msg-1',
              conversationId: 'conv-1',
              senderPeerId: 'peer-alice',
              recipientPeerId: 'peer-me',
              content: 'message',
              contentType: 'text',
              replyToMessageId: null,
              sentAt: 1700000100,
              deliveredAt: null,
              readAt: null,
              status: 'delivered' as const,
              isOutgoing: false,
              editedAt: null,
            },
          ],
        },
        selectedConversationId: 'real-peer-alice',
        activeConversation: 'peer-alice',
        archivedConversations: ['peer-alice'],
      });

      vi.mocked(invoke)
        .mockResolvedValueOnce(undefined) // delete_conversation
        .mockResolvedValueOnce([]); // get_conversations

      await useMessagingStore.getState().deleteConversation('peer-alice');

      const state = useMessagingStore.getState();
      expect(state.messages['peer-alice']).toBeUndefined();
      expect(state.selectedConversationId).toBeNull();
      expect(state.activeConversation).toBeNull();
      expect(state.archivedConversations).not.toContain('peer-alice');
    });
  });
});
