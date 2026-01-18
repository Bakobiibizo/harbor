import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { Message, Conversation, SendMessageResult } from '../types';

interface MessagingState {
  // State
  conversations: Conversation[];
  messages: Record<string, Message[]>; // keyed by peerId
  activeConversation: string | null;
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
  handleIncomingMessage: (message: Message) => void;
  markConversationRead: (peerId: string) => Promise<void>;
}

export const useMessagingStore = create<MessagingState>((set, get) => ({
  // Initial state
  conversations: [],
  messages: {},
  activeConversation: null,
  isLoading: false,
  error: null,

  // Load all conversations
  loadConversations: async () => {
    set({ isLoading: true, error: null });
    try {
      const conversations = await invoke<Conversation[]>('get_conversations');
      set({ conversations, isLoading: false });
    } catch (error) {
      console.error('Failed to load conversations:', error);
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
      console.error('Failed to load messages:', error);
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
      get().loadConversations();

      return result;
    } catch (error) {
      console.error('Failed to send message:', error);
      throw error;
    }
  },

  // Set active conversation
  setActiveConversation: (peerId: string | null) => {
    set({ activeConversation: peerId });
    if (peerId) {
      get().loadMessages(peerId);
    }
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
    get().loadConversations();
  },

  // Mark conversation as read
  markConversationRead: async (peerId: string) => {
    try {
      await invoke('mark_conversation_read', { peerId });
      // Refresh conversations to update unread count
      get().loadConversations();
    } catch (error) {
      console.error('Failed to mark conversation read:', error);
    }
  },
}));
