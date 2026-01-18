import { invoke } from '@tauri-apps/api/core';
import type { Message, Conversation, SendMessageResult } from '../types';

/** Messaging service - wraps Tauri commands */
export const messagingService = {
  /** Send a message to a peer */
  async sendMessage(
    peerId: string,
    content: string,
    contentType?: string,
    replyTo?: string,
  ): Promise<SendMessageResult> {
    return invoke<SendMessageResult>('send_message', {
      peerId,
      content,
      contentType,
      replyTo,
    });
  },

  /** Get messages for a conversation */
  async getMessages(peerId: string, limit?: number, beforeTimestamp?: number): Promise<Message[]> {
    return invoke<Message[]>('get_messages', {
      peerId,
      limit,
      beforeTimestamp,
    });
  },

  /** Get all conversations */
  async getConversations(): Promise<Conversation[]> {
    return invoke<Conversation[]>('get_conversations');
  },

  /** Mark a conversation as read */
  async markConversationRead(peerId: string): Promise<number> {
    return invoke<number>('mark_conversation_read', { peerId });
  },

  /** Get unread count for a conversation */
  async getUnreadCount(peerId: string): Promise<number> {
    return invoke<number>('get_unread_count', { peerId });
  },

  /** Get total unread count across all conversations */
  async getTotalUnreadCount(): Promise<number> {
    return invoke<number>('get_total_unread_count');
  },
};
