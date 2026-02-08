//! Protocol definitions for Harbor P2P communication
//!
//! These must match the protocol definitions in the main Harbor app's behaviour.rs

use serde::{Deserialize, Serialize};

// ============================================================================
// Identity Exchange Protocol
// ============================================================================

/// Request for identity information from a peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityRequest {
    /// The peer ID of the requester
    pub requester_peer_id: String,
    /// Timestamp of the request
    pub timestamp: i64,
    /// Signature over (requester_peer_id, timestamp) using requester's Ed25519 key
    pub signature: Vec<u8>,
}

/// Response containing identity information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityResponse {
    /// The peer ID (derived from public key)
    pub peer_id: String,
    /// Ed25519 public key
    pub public_key: Vec<u8>,
    /// X25519 public key for key agreement
    pub x25519_public: Vec<u8>,
    /// Display name
    pub display_name: String,
    /// Avatar hash (SHA-256 of avatar image)
    pub avatar_hash: Option<String>,
    /// Bio/description
    pub bio: Option<String>,
    /// Timestamp
    pub timestamp: i64,
    /// Signature over all fields above
    pub signature: Vec<u8>,
}

// ============================================================================
// Messaging Protocol
// ============================================================================

/// Messaging request (matches Harbor's MessagingRequest)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagingRequest {
    pub message_type: String,
    pub payload: Vec<u8>,
}

/// Messaging response (matches Harbor's MessagingResponse)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagingResponse {
    pub success: bool,
    pub message_id: Option<String>,
    pub error: Option<String>,
}

// ============================================================================
// Direct Message Types (for reply functionality)
// ============================================================================

/// A direct message between two peers (matches Harbor's DirectMessage)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectMessage {
    /// Unique message ID (UUID v4)
    pub message_id: String,
    /// Conversation ID (derived from sorted peer IDs)
    pub conversation_id: String,
    /// Sender's peer ID
    pub sender_peer_id: String,
    /// Recipient's peer ID
    pub recipient_peer_id: String,
    /// Encrypted message content (AES-256-GCM with nonce derived from nonce_counter)
    pub content_encrypted: Vec<u8>,
    /// Content type (text, image, etc.)
    pub content_type: String,
    /// ID of message being replied to (optional)
    pub reply_to: Option<String>,
    /// Random value used for AES-GCM nonce derivation and replay protection
    pub nonce_counter: u64,
    /// Lamport timestamp for ordering
    pub lamport_clock: u64,
    /// Unix timestamp when message was created
    pub timestamp: i64,
    /// Signature over all fields above (excluding signature itself)
    pub signature: Vec<u8>,
}

/// Message acknowledgment status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AckStatus {
    Delivered,
    Read,
}

/// Acknowledgment of message delivery/read
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageAck {
    /// ID of the message being acknowledged
    pub message_id: String,
    /// Conversation ID
    pub conversation_id: String,
    /// Peer ID of the one sending the ack
    pub peer_id: String,
    /// Status: delivered or read
    pub status: AckStatus,
    /// Unix timestamp
    pub timestamp: i64,
    /// Signature over all fields above
    pub signature: Vec<u8>,
}

/// Request/response wrapper for messaging protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessagingMessage {
    /// A direct message
    Message(DirectMessage),
    /// An acknowledgment
    Ack(MessageAck),
}

/// Helper to derive conversation ID from two peer IDs
pub fn derive_conversation_id(peer_a: &str, peer_b: &str) -> String {
    use sha2::{Sha256, Digest};

    // Sort peer IDs to ensure consistent conversation ID regardless of direction
    let (first, second) = if peer_a < peer_b {
        (peer_a, peer_b)
    } else {
        (peer_b, peer_a)
    };

    let mut hasher = Sha256::new();
    hasher.update(first.as_bytes());
    hasher.update(b":");
    hasher.update(second.as_bytes());
    let result = hasher.finalize();

    hex::encode(&result[..16]) // First 16 bytes = 32 hex chars
}
