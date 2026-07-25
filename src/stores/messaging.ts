import { create } from 'zustand';
import { createLogger } from '../utils/logger';
import type { Message, Conversation, SendMessageResult } from '../types';
import { messagingService } from '../services/messaging';
import { requireProfileId } from '../services/profileSession';
import { migrateLegacyProfileValue, profileStorageKey } from '../services/profileStorage';
import { getErrorMessage } from '../utils/errors';

const log = createLogger('MessagingStore');
const MESSAGING_LEGACY_KEY = 'harbor-messaging';
const MESSAGING_PROFILE_NAMESPACE = 'messaging';
const MESSAGING_PROFILE_VERSION = 1;
let lifecycleGeneration = 0;
let messageSelectionGeneration = 0;
let messageRequestGeneration = 0;

function writeArchivedConversations(archivedConversations: string[]): void {
  localStorage.setItem(
    profileStorageKey(MESSAGING_PROFILE_NAMESPACE, MESSAGING_PROFILE_VERSION),
    JSON.stringify({ state: { archivedConversations }, version: MESSAGING_PROFILE_VERSION }),
  );
}

function readArchivedConversations(): string[] {
  const raw = migrateLegacyProfileValue(
    MESSAGING_LEGACY_KEY,
    MESSAGING_PROFILE_NAMESPACE,
    MESSAGING_PROFILE_VERSION,
  );
  if (!raw) return [];
  try {
    const parsed = JSON.parse(raw) as { state?: { archivedConversations?: unknown } };
    const value = parsed.state?.archivedConversations;
    return Array.isArray(value)
      ? [...new Set(value.filter((item): item is string => typeof item === 'string'))]
      : [];
  } catch {
    return [];
  }
}

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
  updateMessageStatus: (
    messageId: string,
    status: Message['status'],
    deliveredAt?: number | null,
    readAt?: number | null,
  ) => void;
  markConversationRead: (peerId: string) => Promise<void>;
  clearConversationHistory: (peerId: string) => Promise<void>;
  deleteConversation: (peerId: string) => Promise<void>;
  editMessage: (messageId: string, newContent: string, peerId: string) => Promise<void>;
  archiveConversation: (peerId: string) => void;
  unarchiveConversation: (peerId: string) => void;
  isArchived: (peerId: string) => boolean;
}

export const useMessagingStore = create<MessagingState>()((set, get) => ({
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
    const generation = lifecycleGeneration;
    set({ isLoading: true, error: null });
    try {
      const conversations = await messagingService.getConversations();
      if (generation !== lifecycleGeneration) return;
      set({ conversations, isLoading: false });
    } catch (error) {
      if (generation !== lifecycleGeneration) return;
      log.error('Failed to load conversations', error);
      set({ error: getErrorMessage(error), isLoading: false });
    }
  },

  // Load messages for a specific conversation
  loadMessages: async (peerId: string) => {
    const generation = lifecycleGeneration;
    const selection = messageSelectionGeneration;
    const activeConversation = get().activeConversation;
    const request = ++messageRequestGeneration;
    const isCurrent = () =>
      generation === lifecycleGeneration &&
      selection === messageSelectionGeneration &&
      request === messageRequestGeneration &&
      activeConversation === get().activeConversation &&
      (activeConversation === null || activeConversation === peerId);
    set({ isLoading: true, error: null });
    try {
      const messages = await messagingService.getMessages(peerId, 100);
      if (!isCurrent()) return;
      set((state) => ({
        messages: { ...state.messages, [peerId]: messages },
        isLoading: false,
      }));
    } catch (error) {
      if (!isCurrent()) return;
      log.error('Failed to load messages', error);
      set({ error: getErrorMessage(error), isLoading: false });
    }
  },

  // Send a message
  sendMessage: async (peerId: string, content: string, contentType: string = 'text') => {
    const generation = lifecycleGeneration;
    try {
      const result = await messagingService.sendMessage(peerId, content, contentType);

      if (generation !== lifecycleGeneration) return result;

      // Add the sent message to local state optimistically
      const newMessage: Message = {
        messageId: result.messageId,
        conversationId: result.conversationId,
        senderPeerId: '', // Will be filled by backend
        recipientPeerId: peerId,
        contentState: { kind: 'plaintext', text: content },
        contentType,
        replyToMessageId: null,
        sentAt: result.sentAt,
        deliveredAt: null,
        readAt: null,
        status: result.status,
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
    messageSelectionGeneration += 1;
    messageRequestGeneration += 1;
    set({ activeConversation: peerId, isLoading: false, error: null });
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
    messageSelectionGeneration += 1;
    messageRequestGeneration += 1;
    set({ selectedConversationId: null, activeConversation: null, isLoading: false, error: null });
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
    const generation = lifecycleGeneration;
    try {
      await messagingService.markConversationRead(peerId);
      if (generation !== lifecycleGeneration) return;
      // Refresh conversations to update unread count
      get()
        .loadConversations()
        .catch((err) => log.error('Failed to refresh conversations after marking read', err));
    } catch (error) {
      if (generation === lifecycleGeneration) {
        log.error('Failed to mark conversation read', error);
        set({ error: getErrorMessage(error) });
      }
      throw error;
    }
  },

  // Clear all messages in a conversation
  clearConversationHistory: async (peerId: string) => {
    const generation = lifecycleGeneration;
    try {
      await messagingService.clearConversationHistory(peerId);
      if (generation !== lifecycleGeneration) return;
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
      if (generation === lifecycleGeneration) {
        log.error('Failed to clear conversation history', error);
        set({ error: getErrorMessage(error) });
      }
      throw error;
    }
  },

  // Delete a conversation entirely
  deleteConversation: async (peerId: string) => {
    const generation = lifecycleGeneration;
    try {
      await messagingService.deleteConversation(peerId);
      if (generation !== lifecycleGeneration) return;
      // Clear local state
      set((state) => {
        const newMessages = { ...state.messages };
        delete newMessages[peerId];
        return {
          messages: newMessages,
          selectedConversationId:
            state.selectedConversationId === `real-${peerId}` ? null : state.selectedConversationId,
          activeConversation: state.activeConversation === peerId ? null : state.activeConversation,
          archivedConversations: state.archivedConversations.filter((id) => id !== peerId),
        };
      });
      // Refresh conversations list
      get()
        .loadConversations()
        .catch((err) => log.error('Failed to refresh conversations after delete', err));
    } catch (error) {
      if (generation === lifecycleGeneration) {
        log.error('Failed to delete conversation', error);
        set({ error: getErrorMessage(error) });
      }
      throw error;
    }
  },

  // Edit a sent message
  editMessage: async (messageId: string, newContent: string, peerId: string) => {
    const generation = lifecycleGeneration;
    try {
      await messagingService.editMessage(messageId, newContent, peerId);
      if (generation !== lifecycleGeneration) return;

      // Update local state optimistically
      set((state) => {
        const peerMessages = state.messages[peerId];
        if (!peerMessages) return state;

        return {
          messages: {
            ...state.messages,
            [peerId]: peerMessages.map((msg) =>
              msg.messageId === messageId
                ? {
                    ...msg,
                    contentState: { kind: 'plaintext', text: newContent },
                    editedAt: Math.floor(Date.now() / 1000),
                  }
                : msg,
            ),
          },
        };
      });
    } catch (error) {
      if (generation === lifecycleGeneration) {
        log.error('Failed to edit message', error);
        set({ error: getErrorMessage(error) });
      }
      throw error;
    }
  },

  // Archive a conversation (client-side only)
  archiveConversation: (peerId: string) => {
    requireProfileId();
    set((state) => ({
      archivedConversations: state.archivedConversations.includes(peerId)
        ? state.archivedConversations
        : [...state.archivedConversations, peerId],
    }));
    writeArchivedConversations(get().archivedConversations);
  },

  // Unarchive a conversation
  unarchiveConversation: (peerId: string) => {
    requireProfileId();
    set((state) => ({
      archivedConversations: state.archivedConversations.filter((id) => id !== peerId),
    }));
    writeArchivedConversations(get().archivedConversations);
  },

  // Check if a conversation is archived
  isArchived: (peerId: string) => {
    return get().archivedConversations.includes(peerId);
  },
}));

export function hydrateMessagingProfile(): void {
  useMessagingStore.setState({ archivedConversations: readArchivedConversations() });
}

export function resetMessagingProfileMemory(): void {
  lifecycleGeneration += 1;
  messageSelectionGeneration += 1;
  messageRequestGeneration += 1;
  useMessagingStore.setState({
    conversations: [],
    messages: {},
    activeConversation: null,
    selectedConversationId: null,
    archivedConversations: [],
    isLoading: false,
    error: null,
  });
}
