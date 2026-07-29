//! Tauri commands for direct messaging

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use tracing::{info, warn};

use crate::db::repositories::Conversation;
use crate::error::AppError;
use crate::services::{
    DecryptedMessage, MessageContentState, MessagingPrivacyPolicy, MessagingService,
};

/// Message info for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageInfo {
    pub message_id: String,
    pub conversation_id: String,
    pub sender_peer_id: String,
    pub recipient_peer_id: String,
    pub content_state: MessageContentState,
    pub content_type: String,
    pub reply_to_message_id: Option<String>,
    pub sent_at: i64,
    pub delivered_at: Option<i64>,
    pub read_at: Option<i64>,
    pub status: String,
    pub is_outgoing: bool,
    pub edited_at: Option<i64>,
}

impl From<DecryptedMessage> for MessageInfo {
    fn from(msg: DecryptedMessage) -> Self {
        Self {
            message_id: msg.message_id,
            conversation_id: msg.conversation_id,
            sender_peer_id: msg.sender_peer_id,
            recipient_peer_id: msg.recipient_peer_id,
            content_state: msg.content_state,
            content_type: msg.content_type,
            reply_to_message_id: msg.reply_to_message_id,
            sent_at: msg.sent_at,
            delivered_at: msg.delivered_at,
            read_at: msg.read_at,
            status: msg.status,
            is_outgoing: msg.is_outgoing,
            edited_at: msg.edited_at,
        }
    }
}

/// Conversation info for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationInfo {
    pub conversation_id: String,
    pub peer_id: String,
    pub last_message_at: i64,
    pub unread_count: i64,
}

impl From<Conversation> for ConversationInfo {
    fn from(conv: Conversation) -> Self {
        Self {
            conversation_id: conv.conversation_id,
            peer_id: conv.peer_id,
            last_message_at: conv.last_message_at,
            unread_count: conv.unread_count,
        }
    }
}

/// Send result for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageResult {
    pub message_id: String,
    pub conversation_id: String,
    pub sent_at: i64,
    pub status: String,
}

/// Send a message to a peer
#[tauri::command]
pub async fn send_message(
    messaging_service: State<'_, Arc<MessagingService>>,
    peer_id: String,
    content: String,
    content_type: Option<String>,
    reply_to: Option<String>,
) -> Result<SendMessageResult, AppError> {
    let content_type = content_type.unwrap_or_else(|| "text".to_string());

    // Create the encrypted, signed message
    let outgoing =
        messaging_service.send_message(&peer_id, &content, &content_type, reply_to.as_deref())?;

    info!(
        "Message {} queued for peer {}",
        outgoing.message_id, peer_id
    );

    Ok(SendMessageResult {
        message_id: outgoing.message_id,
        conversation_id: outgoing.conversation_id,
        sent_at: outgoing.timestamp,
        status: "queued".to_string(),
    })
}

/// Get messages for a conversation
#[tauri::command]
pub async fn get_messages(
    messaging_service: State<'_, Arc<MessagingService>>,
    peer_id: String,
    limit: Option<i64>,
    before_timestamp: Option<i64>,
) -> Result<Vec<MessageInfo>, AppError> {
    let limit = limit.unwrap_or(50);

    let messages =
        messaging_service.get_conversation_messages(&peer_id, limit, before_timestamp)?;

    Ok(messages.into_iter().map(MessageInfo::from).collect())
}

/// Get all conversations
#[tauri::command]
pub async fn get_conversations(
    messaging_service: State<'_, Arc<MessagingService>>,
) -> Result<Vec<ConversationInfo>, AppError> {
    let conversations = messaging_service.get_conversations()?;
    Ok(conversations
        .into_iter()
        .map(ConversationInfo::from)
        .collect())
}

/// Mark a conversation as read
#[tauri::command]
pub async fn mark_conversation_read(
    messaging_service: State<'_, Arc<MessagingService>>,
    peer_id: String,
) -> Result<i64, AppError> {
    messaging_service.mark_conversation_read(&peer_id)
}

/// Return the selected profile's authoritative messaging privacy policy.
#[tauri::command]
pub async fn get_messaging_privacy_policy(
    messaging_service: State<'_, Arc<MessagingService>>,
) -> Result<MessagingPrivacyPolicy, AppError> {
    messaging_service.privacy_policy()
}

/// Persist read-receipt policy before the UI reflects the requested value.
#[tauri::command]
pub async fn set_read_receipts_enabled(
    messaging_service: State<'_, Arc<MessagingService>>,
    enabled: bool,
) -> Result<MessagingPrivacyPolicy, AppError> {
    messaging_service.set_read_receipts_enabled(enabled)
}

/// Get unread count for a conversation
#[tauri::command]
pub async fn get_unread_count(
    messaging_service: State<'_, Arc<MessagingService>>,
    peer_id: String,
) -> Result<i64, AppError> {
    messaging_service.get_unread_count(&peer_id)
}

/// Get total unread count across all conversations
#[tauri::command]
pub async fn get_total_unread_count(
    messaging_service: State<'_, Arc<MessagingService>>,
) -> Result<i64, AppError> {
    let conversations = messaging_service.get_conversations()?;
    let total: i64 = conversations.iter().map(|c| c.unread_count).sum();
    Ok(total)
}

/// Clear all messages in a conversation (keeps the conversation available)
#[tauri::command]
pub async fn clear_conversation_history(
    messaging_service: State<'_, Arc<MessagingService>>,
    peer_id: String,
) -> Result<i64, AppError> {
    info!("Clearing conversation history with peer {}", peer_id);
    messaging_service.clear_conversation_history(&peer_id)
}

/// Delete a conversation and all its messages
#[tauri::command]
pub async fn delete_conversation(
    messaging_service: State<'_, Arc<MessagingService>>,
    peer_id: String,
) -> Result<i64, AppError> {
    info!("Deleting conversation with peer {}", peer_id);
    messaging_service.delete_conversation(&peer_id)
}

/// Edit a sent message's content
#[tauri::command]
pub async fn edit_message(
    messaging_service: State<'_, Arc<MessagingService>>,
    message_id: String,
    new_content: String,
    peer_id: String,
) -> Result<(), AppError> {
    info!("Editing message {}", message_id);

    let outgoing = messaging_service.edit_message(&message_id, &new_content)?;
    if outgoing.recipient_peer_id != peer_id {
        warn!(
            "Ignoring caller-supplied edit recipient {}; original message is bound to {}",
            peer_id, outgoing.recipient_peer_id
        );
    }
    info!(
        "Edit event {} for message {} queued for peer {}",
        outgoing.event_id, message_id, outgoing.recipient_peer_id
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message_with(content_state: MessageContentState) -> MessageInfo {
        MessageInfo::from(DecryptedMessage {
            message_id: "message-1".to_string(),
            conversation_id: "conversation-1".to_string(),
            sender_peer_id: "sender".to_string(),
            recipient_peer_id: "recipient".to_string(),
            content_state,
            content_type: "text".to_string(),
            reply_to_message_id: None,
            sent_at: 1,
            delivered_at: None,
            read_at: None,
            status: "delivered".to_string(),
            is_outgoing: false,
            edited_at: None,
        })
    }

    #[test]
    fn message_ipc_serializes_plaintext_as_a_tagged_state() {
        let json = serde_json::to_value(message_with(MessageContentState::Plaintext {
            text: "hello".to_string(),
        }))
        .unwrap();

        assert_eq!(json["contentState"]["kind"], "plaintext");
        assert_eq!(json["contentState"]["text"], "hello");
        assert!(json.get("content").is_none());
    }

    #[test]
    fn message_ipc_failure_state_contains_no_authored_or_encrypted_material() {
        let json = serde_json::to_value(message_with(MessageContentState::Tampered)).unwrap();

        assert_eq!(json["contentState"]["kind"], "tampered");
        assert!(json["contentState"].get("text").is_none());
        assert!(json.get("content").is_none());
        assert!(json.get("ciphertext").is_none());
    }
}
