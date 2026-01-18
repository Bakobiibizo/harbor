//! Tauri commands for direct messaging

use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;
use tauri::State;
use tracing::info;

use crate::commands::network::NetworkState;
use crate::db::repositories::Conversation;
use crate::error::AppError;
use crate::p2p::protocols::messaging::{DirectMessage, MessagingCodec, MessagingMessage};
use crate::services::{DecryptedMessage, MessagingService, OutgoingMessage};

/// Message info for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageInfo {
    pub message_id: String,
    pub conversation_id: String,
    pub sender_peer_id: String,
    pub recipient_peer_id: String,
    pub content: String,
    pub content_type: String,
    pub reply_to_message_id: Option<String>,
    pub sent_at: i64,
    pub delivered_at: Option<i64>,
    pub read_at: Option<i64>,
    pub status: String,
    pub is_outgoing: bool,
}

impl From<DecryptedMessage> for MessageInfo {
    fn from(msg: DecryptedMessage) -> Self {
        Self {
            message_id: msg.message_id,
            conversation_id: msg.conversation_id,
            sender_peer_id: msg.sender_peer_id,
            recipient_peer_id: msg.recipient_peer_id,
            content: msg.content,
            content_type: msg.content_type,
            reply_to_message_id: msg.reply_to_message_id,
            sent_at: msg.sent_at,
            delivered_at: msg.delivered_at,
            read_at: msg.read_at,
            status: msg.status,
            is_outgoing: msg.is_outgoing,
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
}

/// Convert OutgoingMessage to DirectMessage for network transmission
fn outgoing_to_direct_message(outgoing: &OutgoingMessage) -> DirectMessage {
    DirectMessage {
        message_id: outgoing.message_id.clone(),
        conversation_id: outgoing.conversation_id.clone(),
        sender_peer_id: outgoing.sender_peer_id.clone(),
        recipient_peer_id: outgoing.recipient_peer_id.clone(),
        content_encrypted: outgoing.content_encrypted.clone(),
        content_type: outgoing.content_type.clone(),
        reply_to: outgoing.reply_to.clone(),
        nonce_counter: outgoing.nonce_counter,
        lamport_clock: outgoing.lamport_clock,
        timestamp: outgoing.timestamp,
        signature: outgoing.signature.clone(),
    }
}

/// Send a message to a peer
#[tauri::command]
pub async fn send_message(
    messaging_service: State<'_, Arc<MessagingService>>,
    network: State<'_, NetworkState>,
    peer_id: String,
    content: String,
    content_type: Option<String>,
    reply_to: Option<String>,
) -> Result<SendMessageResult, AppError> {
    let content_type = content_type.unwrap_or_else(|| "text".to_string());

    // Create the encrypted, signed message
    let outgoing =
        messaging_service.send_message(&peer_id, &content, &content_type, reply_to.as_deref())?;

    // Convert to DirectMessage and encode for network transmission
    let direct_msg = outgoing_to_direct_message(&outgoing);
    let msg_wrapper = MessagingMessage::Message(direct_msg);
    let payload = MessagingCodec::encode(&msg_wrapper)
        .map_err(|e| AppError::Internal(format!("Failed to encode message: {}", e)))?;

    // Parse the peer ID
    let libp2p_peer_id = PeerId::from_str(&peer_id)
        .map_err(|e| AppError::Validation(format!("Invalid peer ID: {}", e)))?;

    // Send over the network
    let handle = network.get_handle().await?;
    handle
        .send_message(libp2p_peer_id, "message".to_string(), payload)
        .await?;

    info!("Message {} sent to peer {}", outgoing.message_id, peer_id);

    Ok(SendMessageResult {
        message_id: outgoing.message_id,
        conversation_id: outgoing.conversation_id,
        sent_at: outgoing.timestamp,
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
