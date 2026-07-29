import type { Message, Conversation, SendMessageResult } from '../types';
import { invokeCommand } from './command';
import { publishingPolicy } from './publishingPolicy';

/** Messaging service - wraps Tauri commands */
export const messagingService = {
  /** Send a message to a peer */
  async sendMessage(
    peerId: string,
    content: string,
    contentType?: string,
    replyTo?: string,
  ): Promise<SendMessageResult> {
    publishingPolicy.assertAllowed();
    return invokeCommand('send_message', {
      peerId,
      content,
      contentType,
      replyTo,
    });
  },

  /** Get messages for a conversation */
  async getMessages(peerId: string, limit?: number, beforeTimestamp?: number): Promise<Message[]> {
    return invokeCommand('get_messages', {
      peerId,
      limit,
      beforeTimestamp,
    });
  },

  /** Get all conversations */
  async getConversations(): Promise<Conversation[]> {
    return invokeCommand('get_conversations');
  },

  /** Mark a conversation as read */
  async markConversationRead(peerId: string): Promise<number> {
    return invokeCommand('mark_conversation_read', { peerId });
  },

  /** Get unread count for a conversation */
  async getUnreadCount(peerId: string): Promise<number> {
    return invokeCommand('get_unread_count', { peerId });
  },

  /** Get total unread count across all conversations */
  async getTotalUnreadCount(): Promise<number> {
    return invokeCommand('get_total_unread_count');
  },

  /** Permanently remove all messages in a conversation while keeping its contact. */
  async clearConversationHistory(peerId: string): Promise<void> {
    return invokeCommand('clear_conversation_history', { peerId });
  },

  /** Permanently remove a conversation and its messages. */
  async deleteConversation(peerId: string): Promise<void> {
    return invokeCommand('delete_conversation', { peerId });
  },

  /** Edit a message already sent to a peer. */
  async editMessage(messageId: string, newContent: string, peerId: string): Promise<void> {
    return invokeCommand('edit_message', { messageId, newContent, peerId });
  },
};
