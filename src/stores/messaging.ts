import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { invoke } from '@tauri-apps/api/core';
import type { Message, Conversation, SendMessageResult } from '../types';

interface MessagingState {
  // State
  conversations: Conversation[];
  messages: Record<string, Message[]>; // keyed by peerId
  activeConversation: string | null;
  selectedConversationId: string | null; // UI state for selected conversation (includes mock)
  archivedConversations: string[]; // peerId list of archived conversations
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
  markConversationRead: (peerId: string) => Promise<void>;
  clearConversationHistory: (peerId: string) => Promise<void>;
  deleteConversation: (peerId: string) => Promise<void>;
  editMessage: (messageId: string, newContent: string, peerId: string) => Promise<void>;
  archiveConversation: (peerId: string) => void;
  unarchiveConversation: (peerId: string) => void;
  isArchived: (peerId: string) => boolean;
}

export const useMessagingStore = create<MessagingState>()(
  persist(
    (set, get) => ({
      // Initial state
      conversations: [],
      messages: {},
      activeConversation: null,
      selectedConversationId: null,
      archivedConversations: [],
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
            editedAt: null,
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
          .catch((err) => log.error('Failed to refresh conversations after incoming message', err));
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

          for (const [pId, msgs] of Object.entries(state.messages)) {
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
            updatedMessages[pId] = updated;
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

      // Clear all messages in a conversation
      clearConversationHistory: async (peerId: string) => {
        try {
          await invoke('clear_conversation_history', { peerId });
          // Clear local message cache for this peer
          set((state) => {
            const newMessages = { ...state.messages };
            delete newMessages[peerId];
            return { messages: newMessages };
          });
          // Refresh conversations list
          get()
            .loadConversations()
            .catch((err) => log.error('Failed to refresh conversations after clearing history', err));
        } catch (error) {
          log.error('Failed to clear conversation history', error);
          throw error;
        }
      },

      // Delete a conversation entirely
      deleteConversation: async (peerId: string) => {
        try {
          await invoke('delete_conversation', { peerId });
          // Clear local state
          set((state) => {
            const newMessages = { ...state.messages };
            delete newMessages[peerId];
            return {
              messages: newMessages,
              selectedConversationId:
                state.selectedConversationId === `real-${peerId}`
                  ? null
                  : state.selectedConversationId,
              activeConversation:
                state.activeConversation === peerId ? null : state.activeConversation,
              archivedConversations: state.archivedConversations.filter((id) => id !== peerId),
            };
          });
          // Refresh conversations list
          get()
            .loadConversations()
            .catch((err) => log.error('Failed to refresh conversations after delete', err));
        } catch (error) {
          log.error('Failed to delete conversation', error);
          throw error;
        }
      },

      // Edit a sent message
      editMessage: async (messageId: string, newContent: string, peerId: string) => {
        try {
          await invoke('edit_message', { messageId, newContent, peerId });

          // Update local state optimistically
          set((state) => {
            const peerMessages = state.messages[peerId];
            if (!peerMessages) return state;

            return {
              messages: {
                ...state.messages,
                [peerId]: peerMessages.map((msg) =>
                  msg.messageId === messageId
                    ? { ...msg, content: newContent, editedAt: Math.floor(Date.now() / 1000) }
                    : msg,
                ),
              },
            };
          });
        } catch (error) {
          log.error('Failed to edit message', error);
          throw error;
        }
      },

      // Archive a conversation (client-side only)
      archiveConversation: (peerId: string) => {
        set((state) => ({
          archivedConversations: [...state.archivedConversations, peerId],
        }));
      },

      // Unarchive a conversation
      unarchiveConversation: (peerId: string) => {
        set((state) => ({
          archivedConversations: state.archivedConversations.filter((id) => id !== peerId),
        }));
      },

      // Check if a conversation is archived
      isArchived: (peerId: string) => {
        return get().archivedConversations.includes(peerId);
      },
    }),
    {
      name: 'harbor-messaging',
      partialize: (state) => ({
        archivedConversations: state.archivedConversations,
      }),
    },
  ),
);
