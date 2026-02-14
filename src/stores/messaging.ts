import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { createLogger } from '../utils/logger';
import type { Message, Conversation, SendMessageResult } from '../types';

const log = createLogger('MessagingStore');

interface MessagingState {
  // State
  conversations: Conversation[];
  messages: Record<string, Message[]>; // keyed by peerId
  activeConversation: string | null;
  selectedConversationId: string | null; // UI state for selected conversation (includes mock)
  isLoading: boolean;
  error: string | null;

  // Actions
  loadConversations: () => Promise<void>;
  loadMessages: (peerId: string) => Promise<void>;
  sendMessage: (
    peerId: string,
    content: string,
    contentType?: string,
  ) => Promise<SendMessageResult>;
  setActiveConversation: (peerId: string | null) => void;
  setSelectedConversation: (id: string | null) => void;
  clearConversationSelection: () => void;
  handleIncomingMessage: (message: Message) => void;
  updateMessageStatus: (
    messageId: string,
    status: Message['status'],
    deliveredAt?: number | null,
    readAt?: number | null,
  ) => void;
  markConversationRead: (peerId: string) => Promise<void>;
}

export const useMessagingStore = create<MessagingState>((set, get) => ({
  // Initial state
  conversations: [],
  messages: {},
  activeConversation: null,
  selectedConversationId: null,
  isLoading: false,
  error: null,

  // Load all conversations
  loadConversations: async () => {
    set({ isLoading: true, error: null });
    try {
      const conversations = await invoke<Conversation[]>('get_conversations');
      set({ conversations, isLoading: false });
    } catch (error) {
      log.error('Failed to load conversations', error);
      set({ error: String(error), isLoading: false });
    }
  },

  // Load messages for a specific conversation
  loadMessages: async (peerId: string) => {
    set({ isLoading: true, error: null });
    try {
      const messages = await invoke<Message[]>('get_messages', {
        peerId,
        limit: 100,
      });
      set((state) => ({
        messages: { ...state.messages, [peerId]: messages },
        isLoading: false,
      }));
    } catch (error) {
      log.error('Failed to load messages', error);
      set({ error: String(error), isLoading: false });
    }
  },

  // Send a message
  sendMessage: async (peerId: string, content: string, contentType: string = 'text') => {
    try {
      const result = await invoke<SendMessageResult>('send_message', {
        peerId,
        content,
        contentType,
      });

      // Add the sent message to local state optimistically
      const newMessage: Message = {
        messageId: result.messageId,
        conversationId: result.conversationId,
        senderPeerId: '', // Will be filled by backend
        recipientPeerId: peerId,
        content,
        contentType,
        replyToMessageId: null,
        sentAt: result.sentAt,
        deliveredAt: null,
        readAt: null,
        status: 'sent',
        isOutgoing: true,
      };

      set((state) => ({
        messages: {
          ...state.messages,
          [peerId]: [...(state.messages[peerId] || []), newMessage],
        },
      }));

      // Refresh conversations to update last message
      get()
        .loadConversations()
        .catch((err) => log.error('Failed to refresh conversations after send', err));

      return result;
    } catch (error) {
      log.error('Failed to send message', error);
      throw error;
    }
  },

  // Set active conversation
  setActiveConversation: (peerId: string | null) => {
    set({ activeConversation: peerId });
    if (peerId) {
      get()
        .loadMessages(peerId)
        .catch((err) => log.error('Failed to load messages for active conversation', err));
    }
  },

  // Set selected conversation (UI state, includes mock conversations)
  setSelectedConversation: (id: string | null) => {
    set({ selectedConversationId: id });
  },

  // Clear conversation selection (used when clicking Messages in sidebar)
  clearConversationSelection: () => {
    set({ selectedConversationId: null, activeConversation: null });
  },

  // Handle incoming message from Tauri event
  handleIncomingMessage: (message: Message) => {
    const peerId = message.isOutgoing ? message.recipientPeerId : message.senderPeerId;

    set((state) => ({
      messages: {
        ...state.messages,
        [peerId]: [...(state.messages[peerId] || []), message],
      },
    }));

    // Refresh conversations to update last message
    get()
      .loadConversations()
      .catch((err) =>
        log.error('Failed to refresh conversations after incoming message', err),
      );
  },

  // Update a message's status (e.g., when an ACK is received)
  updateMessageStatus: (
    messageId: string,
    status: Message['status'],
    deliveredAt?: number | null,
    readAt?: number | null,
  ) => {
    set((state) => {
      const updatedMessages: Record<string, Message[]> = {};
      let found = false;

      for (const [peerId, msgs] of Object.entries(state.messages)) {
        const updated = msgs.map((msg) => {
          if (msg.messageId === messageId) {
            found = true;
            return {
              ...msg,
              status,
              deliveredAt: deliveredAt !== undefined ? deliveredAt : msg.deliveredAt,
              readAt: readAt !== undefined ? readAt : msg.readAt,
            };
          }
          return msg;
        });
        updatedMessages[peerId] = updated;
      }

      if (!found) {
        return state;
      }

      return { messages: updatedMessages };
    });
  },

  // Mark conversation as read
  markConversationRead: async (peerId: string) => {
    try {
      await invoke('mark_conversation_read', { peerId });
      // Refresh conversations to update unread count
      get()
        .loadConversations()
        .catch((err) => log.error('Failed to refresh conversations after marking read', err));
    } catch (error) {
      log.error('Failed to mark conversation read', error);
    }
  },
}));
