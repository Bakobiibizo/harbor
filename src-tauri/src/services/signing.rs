//! Canonical Signing Rules
//!
//! This module defines exactly how data is signed and verified in the chat application.
//! Following these rules strictly prevents subtle security bugs.
//!
//! ## Key Principles
//!
//! 1. **Signature is NEVER part of signed bytes**: The signature field is excluded from
//!    the data being signed. We sign the payload, then attach the signature.
//!
//! 2. **CBOR encoding is deterministic**: We use RFC 8949 canonical CBOR encoding
//!    (sorted keys, minimal encoding) via ciborium.
//!
//! 3. **All signable structs have a `Signable` variant**: For each protocol message,
//!    we define a version without the signature field for signing.
//!
//! ## Signing Process
//!
//! 1. Create the signable payload (struct without signature)
//! 2. CBOR-encode with canonical encoding
//! 3. Sign the raw bytes with Ed25519
//! 4. Create the final message with payload + signature
//!
//! ## Verification Process
//!
//! 1. Extract signature from message
//! 2. Create signable payload from message fields (excluding signature)
//! 3. CBOR-encode with canonical encoding
//! 4. Verify signature against raw bytes

use crate::error::{AppError, Result};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

/// Trait for types that can be canonically signed
pub trait Signable: Serialize {
    /// Get the bytes to be signed (canonical CBOR encoding)
    fn signable_bytes(&self) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        ciborium::into_writer(self, &mut bytes)
            .map_err(|e| AppError::Serialization(format!("CBOR encoding failed: {}", e)))?;
        Ok(bytes)
    }
}

/// Sign data with an Ed25519 key
pub fn sign(signing_key: &SigningKey, signable: &impl Signable) -> Result<Vec<u8>> {
    let bytes = signable.signable_bytes()?;
    let signature = signing_key.sign(&bytes);
    Ok(signature.to_bytes().to_vec())
}

/// Verify a signature against signable data
pub fn verify(
    verifying_key: &VerifyingKey,
    signable: &impl Signable,
    signature_bytes: &[u8],
) -> Result<bool> {
    let bytes = signable.signable_bytes()?;

    let signature = Signature::from_slice(signature_bytes)
        .map_err(|e| AppError::Crypto(format!("Invalid signature format: {}", e)))?;

    Ok(verifying_key.verify(&bytes, &signature).is_ok())
}

// ============================================================
// IDENTITY MESSAGES
// ============================================================

/// Signable version of IdentityRequest (excludes signature)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignableIdentityRequest {
    pub requester_peer_id: String,
    pub timestamp: i64,
}

impl Signable for SignableIdentityRequest {}

/// Signable version of IdentityResponse (excludes signature)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignableIdentityResponse {
    pub peer_id: String,
    pub public_key: Vec<u8>,
    pub x25519_public: Vec<u8>,
    pub display_name: String,
    pub avatar_hash: Option<String>,
    pub bio: Option<String>,
    pub identity_version: String, // hash for caching
    pub timestamp: i64,
}

impl Signable for SignableIdentityResponse {}

// ============================================================
// PERMISSION MESSAGES
// ============================================================

/// Signable version of PermissionRequest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignablePermissionRequest {
    pub request_id: String,
    pub requester_peer_id: String,
    pub capability: String,
    pub message: Option<String>,
    pub lamport_clock: u64,
    pub timestamp: i64,
}

impl Signable for SignablePermissionRequest {}

/// Signable version of PermissionGrant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignablePermissionGrant {
    pub grant_id: String,
    pub issuer_peer_id: String,
    pub subject_peer_id: String,
    pub capability: String,
    pub scope: Option<serde_json::Value>,
    pub lamport_clock: u64,
    pub issued_at: i64,
    pub expires_at: Option<i64>,
}

impl Signable for SignablePermissionGrant {}

/// Signable version of PermissionRevoke
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignablePermissionRevoke {
    pub grant_id: String,
    pub issuer_peer_id: String,
    pub lamport_clock: u64,
    pub revoked_at: i64,
}

impl Signable for SignablePermissionRevoke {}

// ============================================================
// MESSAGE TYPES
// ============================================================

/// Signable version of DirectMessage (excludes signature)
///
/// # Nonce Counter Field
///
/// The `nonce_counter` is included in the signed payload, which means:
/// - Attacker cannot modify the counter without invalidating signature
/// - Each message has a cryptographically bound nonce
/// - Replay of exact message is detected via `check_and_record_nonce()`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignableDirectMessage {
    pub message_id: String,
    pub conversation_id: String,
    pub sender_peer_id: String,
    pub recipient_peer_id: String,
    pub content_encrypted: Vec<u8>,
    pub content_type: String,
    pub reply_to: Option<String>,
    pub nonce_counter: u64, // For replay protection - bound to signature
    pub lamport_clock: u64,
    pub timestamp: i64,
}

impl Signable for SignableDirectMessage {}

/// Signable version of MessageAck (excludes signature)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignableMessageAck {
    pub message_id: String,
    pub conversation_id: String,
    pub ack_sender_peer_id: String,
    pub status: String, // "delivered" or "read"
    pub timestamp: i64,
}

impl Signable for SignableMessageAck {}

// ============================================================
// POST MESSAGES
// ============================================================

/// Signable version of Post (excludes signature)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignablePost {
    pub post_id: String,
    pub author_peer_id: String,
    pub content_type: String,
    pub content_text: Option<String>,
    pub media_hashes: Vec<String>,
    pub visibility: String,
    pub lamport_clock: u64,
    pub created_at: i64,
}

impl Signable for SignablePost {}

/// Signable version of PostUpdate (excludes signature)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignablePostUpdate {
    pub post_id: String,
    pub author_peer_id: String,
    pub content_text: Option<String>,
    pub lamport_clock: u64,
    pub updated_at: i64,
}

impl Signable for SignablePostUpdate {}

/// Signable version of PostDelete (excludes signature)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignablePostDelete {
    pub post_id: String,
    pub author_peer_id: String,
    pub lamport_clock: u64,
    pub deleted_at: i64,
}

impl Signable for SignablePostDelete {}

/// Signable version of PostLike (excludes signature)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignablePostLike {
    pub post_id: String,
    pub liker_peer_id: String,
    pub reaction_type: String,
    pub timestamp: i64,
}

impl Signable for SignablePostLike {}

// ============================================================
// BOARD MESSAGES
// ============================================================

/// Signable version of a board post submission (excludes signature)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignableBoardPost {
    pub post_id: String,
    pub board_id: String,
    pub author_peer_id: String,
    pub content_type: String,
    pub content_text: Option<String>,
    pub lamport_clock: u64,
    pub created_at: i64,
}

impl Signable for SignableBoardPost {}

/// Signable version of a board post delete (excludes signature)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignableBoardPostDelete {
    pub post_id: String,
    pub author_peer_id: String,
    pub timestamp: i64,
}

impl Signable for SignableBoardPostDelete {}

/// Signable version of a peer registration (excludes signature)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignablePeerRegistration {
    pub peer_id: String,
    pub display_name: String,
    pub timestamp: i64,
}

impl Signable for SignablePeerRegistration {}

/// Signable version of a board list request (excludes signature)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignableBoardListRequest {
    pub requester_peer_id: String,
    pub timestamp: i64,
}

impl Signable for SignableBoardListRequest {}

/// Signable version of a board posts request (excludes signature)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignableBoardPostsRequest {
    pub requester_peer_id: String,
    pub board_id: String,
    pub timestamp: i64,
}

impl Signable for SignableBoardPostsRequest {}

// ============================================================
// WALL POST MESSAGES (relay-synced personal posts)
// ============================================================

/// Signable version of a wall post submission request (excludes request_signature).
/// The inner post `signature` field is included as data being signed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignableWallPostSubmit {
    pub author_peer_id: String,
    pub post_id: String,
    pub content_type: String,
    pub content_text: Option<String>,
    pub visibility: String,
    pub lamport_clock: i64,
    pub created_at: i64,
    pub signature: Vec<u8>,
    pub timestamp: i64,
}

impl Signable for SignableWallPostSubmit {}

/// Signable version of a wall posts retrieval request (excludes signature)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignableGetWallPosts {
    pub requester_peer_id: String,
    pub author_peer_id: String,
    pub since_lamport_clock: i64,
    pub limit: u32,
    pub timestamp: i64,
}

impl Signable for SignableGetWallPosts {}

/// Signable version of a wall post delete request (excludes signature)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignableWallPostDelete {
    pub author_peer_id: String,
    pub post_id: String,
    pub timestamp: i64,
}

impl Signable for SignableWallPostDelete {}

// ============================================================
// SIGNALING (Voice Calls)
// ============================================================

/// Signable version of SignalingOffer (excludes signature)
///
/// The SDP (Session Description Protocol) blob contains WebRTC parameters.
/// Signing prevents MITM from modifying the offer during relay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignableSignalingOffer {
    pub call_id: String,
    pub caller_peer_id: String,
    pub callee_peer_id: String,
    pub sdp: String,
    pub timestamp: i64,
}

impl Signable for SignableSignalingOffer {}

/// Signable version of SignalingAnswer (excludes signature)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignableSignalingAnswer {
    pub call_id: String,
    pub caller_peer_id: String,
    pub callee_peer_id: String,
    pub sdp: String,
    pub timestamp: i64,
}

impl Signable for SignableSignalingAnswer {}

/// Signable version of SignalingIce (excludes signature)
///
/// ICE candidates are signed to prevent injection attacks
/// where an attacker could redirect media streams.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignableSignalingIce {
    pub call_id: String,
    pub sender_peer_id: String,
    pub candidate: String,
    pub sdp_mid: Option<String>,
    pub sdp_mline_index: Option<u32>,
    pub timestamp: i64,
}

impl Signable for SignableSignalingIce {}

/// Signable version of SignalingHangup (excludes signature)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignableSignalingHangup {
    pub call_id: String,
    pub sender_peer_id: String,
    pub reason: String, // "normal", "busy", "declined", "error"
    pub timestamp: i64,
}

impl Signable for SignableSignalingHangup {}

// ============================================================
// CONTENT SYNC
// ============================================================

/// Signable version of ContentManifestRequest (excludes signature)
///
/// The `cursor` is a map of author_peer_id -> last_seen_lamport_clock.
/// This replaces timestamp-based sync with lamport-based sync, ensuring
/// no events are missed due to clock skew.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignableContentManifestRequest {
    pub requester_peer_id: String,
    /// Map of author_peer_id -> highest lamport clock seen from that author
    /// Empty map means "give me everything"
    pub cursor: std::collections::HashMap<String, u64>,
    pub limit: u32,
    pub timestamp: i64,
}

impl Signable for SignableContentManifestRequest {}

/// Signable version of ContentManifestResponse (excludes signature)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignableContentManifestResponse {
    pub responder_peer_id: String,
    /// Posts included in this response
    pub posts: Vec<PostSummary>,
    /// Whether there are more posts to fetch
    pub has_more: bool,
    /// Updated cursor for next request (author_peer_id -> lamport_clock)
    pub next_cursor: std::collections::HashMap<String, u64>,
    pub timestamp: i64,
}

impl Signable for SignableContentManifestResponse {}

/// Summary of a post for manifest responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostSummary {
    pub post_id: String,
    pub author_peer_id: String,
    pub lamport_clock: u64,
    pub content_type: String,
    pub has_media: bool,
    pub media_hashes: Vec<String>,
    pub created_at: i64,
}

/// Permission proof for content requests
/// This is what gets sent to prove you have access
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionProof {
    /// The signed grant object (CBOR bytes)
    pub grant_cbor: Vec<u8>,
    /// Signature over the grant
    pub grant_signature: Vec<u8>,
    /// Optional: latest known revocation timestamp (if any)
    pub latest_revoke_check: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;

    #[test]
    fn test_sign_and_verify() {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();

        let request = SignableIdentityRequest {
            requester_peer_id: "12D3KooWTest".to_string(),
            timestamp: 1234567890,
        };

        let signature = sign(&signing_key, &request).unwrap();
        assert!(verify(&verifying_key, &request, &signature).unwrap());
    }

    #[test]
    fn test_modified_data_fails_verification() {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();

        let request = SignableIdentityRequest {
            requester_peer_id: "12D3KooWTest".to_string(),
            timestamp: 1234567890,
        };

        let signature = sign(&signing_key, &request).unwrap();

        // Modify the data
        let modified_request = SignableIdentityRequest {
            requester_peer_id: "12D3KooWTest".to_string(),
            timestamp: 1234567891, // Different timestamp
        };

        // Verification should fail
        assert!(!verify(&verifying_key, &modified_request, &signature).unwrap());
    }

    #[test]
    fn test_wrong_key_fails_verification() {
        let signing_key = SigningKey::generate(&mut OsRng);
        let wrong_key = SigningKey::generate(&mut OsRng);
        let wrong_verifying_key = wrong_key.verifying_key();

        let request = SignableIdentityRequest {
            requester_peer_id: "12D3KooWTest".to_string(),
            timestamp: 1234567890,
        };

        let signature = sign(&signing_key, &request).unwrap();

        // Verification with wrong key should fail
        assert!(!verify(&wrong_verifying_key, &request, &signature).unwrap());
    }

    #[test]
    fn test_canonical_cbor_deterministic() {
        let request1 = SignableIdentityRequest {
            requester_peer_id: "12D3KooWTest".to_string(),
            timestamp: 1234567890,
        };

        let request2 = SignableIdentityRequest {
            requester_peer_id: "12D3KooWTest".to_string(),
            timestamp: 1234567890,
        };

        let bytes1 = request1.signable_bytes().unwrap();
        let bytes2 = request2.signable_bytes().unwrap();

        assert_eq!(bytes1, bytes2, "Same data should produce same bytes");
    }
}
