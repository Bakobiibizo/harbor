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
