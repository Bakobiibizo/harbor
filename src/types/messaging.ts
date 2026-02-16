/** A message in a conversation */
export interface Message {
  messageId: string;
  conversationId: string;
  senderPeerId: string;
  recipientPeerId: string;
  content: string;
  contentType: string;
  replyToMessageId: string | null;
  sentAt: number;
  deliveredAt: number | null;
  readAt: number | null;
  status: MessageStatus;
  isOutgoing: boolean;
  editedAt: number | null;
}

/** Message delivery status */
export type MessageStatus = 'pending' | 'sent' | 'delivered' | 'read' | 'failed';

/** A conversation summary */
export interface Conversation {
  conversationId: string;
  peerId: string;
  lastMessageAt: number;
  unreadCount: number;
}

/** Result of sending a message */
export interface SendMessageResult {
  messageId: string;
  conversationId: string;
  sentAt: number;
}
