/** A message in a conversation */
export type MessageContentState =
  | { kind: 'plaintext'; text: string }
  | { kind: 'tampered' }
  | { kind: 'wrong_key' }
  | { kind: 'unsupported_version'; version: number }
  | { kind: 'corrupt_payload' };

export interface Message {
  messageId: string;
  conversationId: string;
  senderPeerId: string;
  recipientPeerId: string;
  contentState: MessageContentState;
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
export type MessageStatus = 'queued' | 'sent' | 'delivered' | 'read' | 'failed';

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
  status: MessageStatus;
}
