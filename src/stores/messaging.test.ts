import { describe, it, expect, vi, beforeEach } from 'vitest';
import { resetMessagingProfileMemory, useMessagingStore } from './messaging';
import { invoke } from '@tauri-apps/api/core';
import { activateProfile, suspendProfile } from '../services/profileSession';

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

describe('useMessagingStore', () => {
  beforeEach(() => {
    suspendProfile();
    activateProfile('test-profile');
    localStorage.clear();
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
    it('does not apply a delayed result after profile teardown', async () => {
      let resolve!: (value: unknown) => void;
      vi.mocked(invoke).mockImplementationOnce(
        () => new Promise((resolvePromise) => (resolve = resolvePromise)),
      );

      const loading = useMessagingStore.getState().loadConversations();
      resetMessagingProfileMemory();
      resolve([
        {
          conversationId: 'profile-a-conversation',
          peerId: 'peer-a',
          lastMessage: 'secret',
          lastMessageAt: 1,
          unreadCount: 0,
        },
      ]);
      await loading;

      expect(useMessagingStore.getState().conversations).toEqual([]);
      expect(useMessagingStore.getState().isLoading).toBe(false);
    });

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
          contentState: { kind: 'plaintext' as const, text: 'Hello!' },
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

    it('keeps the newest peer authoritative when an old peer resolves last', async () => {
      const oldMessages = deferred<unknown>();
      const messageB = {
        messageId: 'msg-b',
        conversationId: 'conv-b',
        senderPeerId: 'peer-bob',
        recipientPeerId: 'peer-me',
        contentState: { kind: 'plaintext' as const, text: 'current peer' },
        contentType: 'text',
        replyToMessageId: null,
        sentAt: 2,
        deliveredAt: null,
        readAt: null,
        status: 'delivered' as const,
        isOutgoing: false,
        editedAt: null,
      };
      vi.mocked(invoke).mockImplementation((command, args) => {
        if (command !== 'get_messages') return Promise.resolve([]);
        return (args as { peerId: string }).peerId === 'peer-alice'
          ? oldMessages.promise
          : Promise.resolve([messageB]);
      });

      useMessagingStore.getState().setActiveConversation('peer-alice');
      useMessagingStore.getState().setActiveConversation('peer-bob');
      await vi.waitFor(() =>
        expect(useMessagingStore.getState().messages['peer-bob']).toEqual([messageB]),
      );
      oldMessages.resolve([
        {
          ...messageB,
          messageId: 'msg-a',
          senderPeerId: 'peer-alice',
          contentState: { kind: 'plaintext' as const, text: 'stale peer' },
        },
      ]);
      await Promise.resolve();

      expect(useMessagingStore.getState()).toMatchObject({
        activeConversation: 'peer-bob',
        isLoading: false,
        error: null,
      });
      expect(useMessagingStore.getState().messages['peer-alice']).toBeUndefined();
    });

    it('does not let an old peer error replace the current peer state', async () => {
      const oldMessages = deferred<unknown>();
      vi.mocked(invoke).mockImplementation((command, args) => {
        if (command !== 'get_messages') return Promise.resolve([]);
        return (args as { peerId: string }).peerId === 'peer-alice'
          ? oldMessages.promise
          : Promise.resolve([]);
      });

      useMessagingStore.getState().setActiveConversation('peer-alice');
      useMessagingStore.getState().setActiveConversation('peer-bob');
      await vi.waitFor(() => expect(useMessagingStore.getState().isLoading).toBe(false));
      oldMessages.reject(new Error('stale peer failure'));
      await Promise.resolve();

      expect(useMessagingStore.getState()).toMatchObject({
        activeConversation: 'peer-bob',
        isLoading: false,
        error: null,
      });
    });
  });

  describe('sendMessage', () => {
    it('should send a message and add it to local state', async () => {
      const sendResult = {
        messageId: 'msg-new',
        conversationId: 'conv-1',
        sentAt: 1700000200,
        status: 'queued' as const,
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
      expect(messages[0].contentState).toEqual({ kind: 'plaintext', text: 'Hi Alice!' });
      expect(messages[0].isOutgoing).toBe(true);
      expect(messages[0].status).toBe('queued');
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
        contentState: { kind: 'plaintext' as const, text: 'Hey there!' },
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
      expect(messages[0].contentState).toEqual({ kind: 'plaintext', text: 'Hey there!' });
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
              contentState: { kind: 'plaintext', text: 'First message' },
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
        contentState: { kind: 'plaintext' as const, text: 'Second message' },
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
              contentState: { kind: 'plaintext', text: 'Original' },
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
      expect(msg.contentState).toEqual({ kind: 'plaintext', text: 'Edited content' });
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
              contentState: { kind: 'plaintext', text: 'Original' },
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
              contentState: { kind: 'plaintext', text: 'message' },
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

    it('rejects a structured failure without clearing the confirmed message history', async () => {
      const message = {
        messageId: 'msg-1',
        conversationId: 'conv-1',
        senderPeerId: 'peer-alice',
        recipientPeerId: 'peer-me',
        contentState: { kind: 'plaintext' as const, text: 'keep me' },
        contentType: 'text',
        replyToMessageId: null,
        sentAt: 1700000100,
        deliveredAt: null,
        readAt: null,
        status: 'delivered' as const,
        isOutgoing: false,
        editedAt: null,
      };
      useMessagingStore.setState({ messages: { 'peer-alice': [message] } });
      vi.mocked(invoke).mockRejectedValue({
        code: 'DATABASE_ERROR',
        message: 'History could not be cleared',
      });

      await expect(
        useMessagingStore.getState().clearConversationHistory('peer-alice'),
      ).rejects.toMatchObject({ code: 'DATABASE_ERROR' });

      expect(useMessagingStore.getState().messages['peer-alice']).toEqual([message]);
      expect(useMessagingStore.getState().error).toBe('History could not be cleared');
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
              contentState: { kind: 'plaintext', text: 'message' },
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
