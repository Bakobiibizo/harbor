//! Server-side board logic for the relay server

use crate::db::{BoardPostCursor, RelayDatabase, WallSocialEventRow, WallSocialEventWriteOutcome};
use crate::peer_binding::{verify_peer_key_binding, verify_registration_time};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tracing::{info, warn};

const MAX_POST_MEDIA_BYTES: i64 = 10 * 1024 * 1024;
pub const RELAY_READ_DENIED: &str = "RELAY_READ_DENIED";
pub const RELAY_READ_DATABASE: &str = "RELAY_READ_DATABASE";
pub const RELAY_READ_SIGNATURE_INVALID: &str = "RELAY_READ_SIGNATURE_INVALID";
pub const RELAY_READ_GRANT_INVALID: &str = "RELAY_READ_GRANT_INVALID";
pub const RELAY_READ_SCOPE_UNSUPPORTED: &str = "RELAY_READ_SCOPE_UNSUPPORTED";
pub const RELAY_AUTH_DATABASE: &str = "RELAY_AUTH_DATABASE";
pub const RELAY_PEER_SIGNATURE_INVALID: &str = "RELAY_PEER_SIGNATURE_INVALID";
pub const RELAY_POST_INVALID: &str = "RELAY_POST_INVALID";
pub const RELAY_POST_SIGNATURE_INVALID: &str = "RELAY_POST_SIGNATURE_INVALID";
pub const RELAY_INTEGER_RANGE: &str = crate::db::RELAY_INTEGER_RANGE;

// ============================================================
// Signable types (must match the client-side definitions exactly)
// ============================================================

/// Trait for types that can be canonically signed via CBOR encoding.
/// This mirrors the client-side `Signable` trait in `services/signing.rs`.
trait Signable: Serialize {
    fn signable_bytes(&self) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::new();
        ciborium::into_writer(self, &mut bytes)
            .map_err(|encode_error| format!("CBOR encoding failed: {}", encode_error))?;
        Ok(bytes)
    }
}

/// Signable version of a board post submission (excludes signature field).
/// Must match `SignableBoardPost` on the client side field-for-field.
#[derive(Debug, Clone, Serialize)]
struct SignableBoardPost {
    pub post_id: String,
    pub board_id: String,
    pub author_peer_id: String,
    pub content_type: String,
    pub content_text: Option<String>,
    pub lamport_clock: u64,
    pub created_at: i64,
}

impl Signable for SignableBoardPost {}

/// Signable version of a board post delete (excludes signature field).
/// Must match `SignableBoardPostDelete` on the client side.
#[derive(Debug, Clone, Serialize)]
struct SignableBoardPostDelete {
    pub post_id: String,
    pub author_peer_id: String,
    pub timestamp: i64,
}

impl Signable for SignableBoardPostDelete {}

/// Signable version of a peer registration (excludes signature field).
/// Must match `SignablePeerRegistration` on the client side.
#[derive(Debug, Clone, Serialize)]
struct SignablePeerRegistration {
    pub peer_id: String,
    pub display_name: String,
    pub timestamp: i64,
}

impl Signable for SignablePeerRegistration {}

/// Signable version of a board list request (excludes signature field).
/// Must match `SignableBoardListRequest` on the client side.
#[derive(Debug, Clone, Serialize)]
struct SignableBoardListRequest {
    pub requester_peer_id: String,
    pub timestamp: i64,
}

impl Signable for SignableBoardListRequest {}

/// Signable version of a board posts request (excludes signature field).
/// Must match `SignableBoardPostsRequest` on the client side.
#[derive(Debug, Clone, Serialize)]
struct SignableBoardPostsRequest {
    pub requester_peer_id: String,
    pub board_id: String,
    pub after_timestamp: Option<i64>,
    pub limit: u32,
    pub timestamp: i64,
}

impl Signable for SignableBoardPostsRequest {}

#[derive(Debug, Clone, Serialize)]
struct SignableOlderBoardPostsRequest {
    pub requester_peer_id: String,
    pub board_id: String,
    pub before: Option<BoardPostCursor>,
    pub limit: u32,
    pub timestamp: i64,
}

impl Signable for SignableOlderBoardPostsRequest {}

/// Signable version of a wall post submission request (excludes request_signature).
/// Must match `SignableWallPostSubmit` on the client side.
#[derive(Debug, Clone, Serialize)]
struct SignableWallPostSubmit {
    pub author_peer_id: String,
    pub post_id: String,
    pub content_type: String,
    pub content_text: Option<String>,
    pub visibility: String,
    pub lamport_clock: i64,
    pub created_at: i64,
    pub signature: Vec<u8>,
    pub media_hashes: Vec<String>,
    pub media_items: Vec<crate::WallPostMediaItemProto>,
    pub timestamp: i64,
}

impl Signable for SignableWallPostSubmit {}

/// Signable version of a wall post (must match client SignablePost).
#[derive(Debug, Clone, Serialize)]
struct SignablePost {
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

/// Signable version of media metadata (must match client SignablePostMedia).
#[derive(Debug, Clone, Serialize)]
struct SignablePostMedia {
    pub post_id: String,
    pub author_peer_id: String,
    pub media_hash: String,
    pub media_type: String,
    pub mime_type: String,
    pub file_name: String,
    pub file_size: i64,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub duration_seconds: Option<i32>,
    pub sort_order: i32,
}

impl Signable for SignablePostMedia {}

/// Signable version of a wall posts retrieval request (excludes signature).
/// Must match `SignableGetWallPosts` on the client side.
#[derive(Debug, Clone, Serialize)]
struct SignableGetWallPosts {
    pub requester_peer_id: String,
    pub author_peer_id: String,
    pub since_lamport_clock: i64,
    pub limit: u32,
    pub timestamp: i64,
}

impl Signable for SignableGetWallPosts {}

#[derive(Debug, Clone, Serialize)]
struct SignableWallSocialEventSubmit {
    pub event_id: String,
    pub event_type: String,
    pub post_id: String,
    pub actor_peer_id: String,
    pub author_name: Option<String>,
    pub comment_id: Option<String>,
    pub content: Option<String>,
    pub reaction_type: Option<String>,
    pub timestamp: i64,
    pub payload_cbor: Vec<u8>,
    pub signature: Vec<u8>,
    pub request_timestamp: i64,
}
impl Signable for SignableWallSocialEventSubmit {}

#[derive(Debug, Clone, Serialize)]
struct SignableWallCommentCreate {
    event_id: String,
    post_id: String,
    comment_id: String,
    actor_peer_id: String,
    author_name: String,
    content: String,
    timestamp: i64,
}
impl Signable for SignableWallCommentCreate {}

#[derive(Debug, Clone, Serialize)]
struct SignableWallCommentDelete {
    event_id: String,
    post_id: String,
    comment_id: String,
    actor_peer_id: String,
    timestamp: i64,
}
impl Signable for SignableWallCommentDelete {}

#[derive(Debug, Clone, Serialize)]
struct SignableWallReaction {
    event_id: String,
    post_id: String,
    actor_peer_id: String,
    reaction_type: String,
    timestamp: i64,
}
impl Signable for SignableWallReaction {}

#[derive(Debug, Clone, Serialize)]
struct SignableGetWallSocialEvents {
    pub requester_peer_id: String,
    pub author_peer_id: String,
    pub post_ids: Vec<String>,
    pub after_timestamp: i64,
    pub limit: u32,
    pub timestamp: i64,
}
impl Signable for SignableGetWallSocialEvents {}

fn canonical_uuid(value: &str) -> bool {
    uuid::Uuid::parse_str(value).is_ok_and(|parsed| parsed.to_string() == value)
}

fn valid_social_text(value: &str, max_chars: usize, allow_newlines: bool) -> bool {
    !value.is_empty()
        && value.trim() == value
        && value.chars().count() <= max_chars
        && !value.chars().any(|character| {
            character.is_control() && !(allow_newlines && matches!(character, '\n' | '\r' | '\t'))
        })
}

fn canonical_social_payload(
    event: &crate::WallSocialEventItemProto,
) -> Result<(Vec<u8>, Option<&str>), String> {
    if !canonical_uuid(&event.event_id)
        || event.post_id.is_empty()
        || event.post_id.len() > 128
        || event.post_id.chars().any(char::is_control)
        || event.actor_peer_id.is_empty()
        || event.timestamp <= 0
        || event.payload_cbor.is_empty()
        || event.payload_cbor.len() > 16 * 1024
        || event.signature.len() != 64
    {
        return Err("RELAY_SOCIAL_INVALID".into());
    }
    let (bytes, expected_author_name) = match event.event_type.as_str() {
        "comment_create" => {
            let (Some(author_name), Some(comment_id), Some(content), None) = (
                event.author_name.as_deref(),
                event.comment_id.as_deref(),
                event.content.as_deref(),
                event.reaction_type.as_deref(),
            ) else {
                return Err("RELAY_SOCIAL_INVALID".into());
            };
            if !canonical_uuid(comment_id)
                || !valid_social_text(author_name, 128, false)
                || !valid_social_text(content, 4_096, true)
            {
                return Err("RELAY_SOCIAL_INVALID".into());
            }
            (
                SignableWallCommentCreate {
                    event_id: event.event_id.clone(),
                    post_id: event.post_id.clone(),
                    comment_id: comment_id.into(),
                    actor_peer_id: event.actor_peer_id.clone(),
                    author_name: author_name.into(),
                    content: content.into(),
                    timestamp: event.timestamp,
                }
                .signable_bytes()?,
                Some(author_name),
            )
        }
        "comment_delete" => {
            let (None, Some(comment_id), None, None) = (
                event.author_name.as_deref(),
                event.comment_id.as_deref(),
                event.content.as_deref(),
                event.reaction_type.as_deref(),
            ) else {
                return Err("RELAY_SOCIAL_INVALID".into());
            };
            if !canonical_uuid(comment_id) {
                return Err("RELAY_SOCIAL_INVALID".into());
            }
            (
                SignableWallCommentDelete {
                    event_id: event.event_id.clone(),
                    post_id: event.post_id.clone(),
                    comment_id: comment_id.into(),
                    actor_peer_id: event.actor_peer_id.clone(),
                    timestamp: event.timestamp,
                }
                .signable_bytes()?,
                None,
            )
        }
        "reaction_add" | "reaction_remove" => {
            let (None, None, None, Some(reaction_type)) = (
                event.author_name.as_deref(),
                event.comment_id.as_deref(),
                event.content.as_deref(),
                event.reaction_type.as_deref(),
            ) else {
                return Err("RELAY_SOCIAL_INVALID".into());
            };
            if !valid_social_text(reaction_type, 64, false) {
                return Err("RELAY_SOCIAL_INVALID".into());
            }
            (
                SignableWallReaction {
                    event_id: event.event_id.clone(),
                    post_id: event.post_id.clone(),
                    actor_peer_id: event.actor_peer_id.clone(),
                    reaction_type: reaction_type.into(),
                    timestamp: event.timestamp,
                }
                .signable_bytes()?,
                None,
            )
        }
        _ => return Err("RELAY_SOCIAL_INVALID".into()),
    };
    if bytes != event.payload_cbor {
        return Err("RELAY_SOCIAL_PAYLOAD_MISMATCH".into());
    }
    Ok((bytes, expected_author_name))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallReadGrantProof {
    pub grant_id: String,
    pub issuer_peer_id: String,
    pub subject_peer_id: String,
    pub capability: String,
    pub scope: Option<serde_json::Value>,
    pub lamport_clock: u64,
    pub issued_at: i64,
    pub expires_at: Option<i64>,
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, Serialize)]
struct SignablePermissionGrant {
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

#[derive(Debug, Clone, Serialize)]
struct SignablePermissionRevoke {
    pub grant_id: String,
    pub issuer_peer_id: String,
    pub lamport_clock: u64,
    pub revoked_at: i64,
}

impl Signable for SignablePermissionRevoke {}

/// Signable version of a wall post delete (excludes signature).
/// Must match `SignableWallPostDelete` on the client side.
#[derive(Debug, Clone, Serialize)]
struct SignableWallPostDelete {
    pub post_id: String,
    pub author_peer_id: String,
    pub lamport_clock: u64,
    pub deleted_at: i64,
}

impl Signable for SignableWallPostDelete {}

// ============================================================
// Signature verification helpers
// ============================================================

/// Verify an ed25519 signature against signable data using raw public key bytes.
fn verify_signature(
    public_key_bytes: &[u8],
    signable: &impl Signable,
    signature_bytes: &[u8],
) -> Result<(), String> {
    let key_array: [u8; 32] = public_key_bytes.try_into().map_err(|_| {
        format!(
            "Invalid public key length: expected 32 bytes, got {}",
            public_key_bytes.len()
        )
    })?;

    let verifying_key = VerifyingKey::from_bytes(&key_array)
        .map_err(|key_error| format!("Invalid Ed25519 public key: {}", key_error))?;

    let encoded_payload = signable.signable_bytes()?;

    let signature = Signature::from_slice(signature_bytes)
        .map_err(|sig_error| format!("Invalid signature format: {}", sig_error))?;

    verifying_key
        .verify(&encoded_payload, &signature)
        .map_err(|_| "Signature verification failed".to_string())
}

/// Look up a registered peer's public key from the database and verify the signature.
fn verify_registered_peer_signature(
    database: &RelayDatabase,
    peer_id: &str,
    signable: &impl Signable,
    signature_bytes: &[u8],
) -> Result<(), String> {
    let stored_public_key = registered_peer_public_key(database, peer_id)?;

    verify_signature(&stored_public_key, signable, signature_bytes)
}

fn registered_peer_public_key(database: &RelayDatabase, peer_id: &str) -> Result<Vec<u8>, String> {
    let stored_public_key = database
        .get_peer_public_key(peer_id)
        .map_err(|_| "RELAY_PEER_DATABASE".to_string())?
        .ok_or_else(|| "RELAY_PEER_NOT_REGISTERED".to_string())?;
    verify_peer_key_binding(peer_id, &stored_public_key)
        .map_err(|binding_error| binding_error.code().to_string())?;
    Ok(stored_public_key)
}

fn stable_verification_error(error: String, fallback: &str) -> String {
    if error.starts_with("RELAY_PEER_") {
        error
    } else {
        fallback.to_string()
    }
}

fn verify_registered_peer_raw_signature(
    database: &RelayDatabase,
    peer_id: &str,
    payload: &[u8],
    signature_bytes: &[u8],
) -> Result<(), String> {
    let stored_public_key = registered_peer_public_key(database, peer_id)?;
    let key_array: [u8; 32] = stored_public_key.as_slice().try_into().map_err(|_| {
        format!(
            "Invalid public key length: expected 32 bytes, got {}",
            stored_public_key.len()
        )
    })?;
    let verifying_key = VerifyingKey::from_bytes(&key_array)
        .map_err(|key_error| format!("Invalid Ed25519 public key: {}", key_error))?;
    let signature = Signature::from_slice(signature_bytes)
        .map_err(|sig_error| format!("Invalid signature format: {}", sig_error))?;
    verifying_key
        .verify(payload, &signature)
        .map_err(|_| "Signature verification failed".to_string())
}

fn validate_media_hash(media_hash: &str) -> Result<(), String> {
    if media_hash.len() != 64 || !media_hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!(
            "Invalid media hash '{}': expected 64 hex characters",
            media_hash
        ));
    }
    Ok(())
}

fn expected_media_type_for_mime(mime_type: &str) -> Option<&'static str> {
    match mime_type {
        "image/jpeg" | "image/png" | "image/gif" | "image/webp" => Some("image"),
        "video/mp4" | "video/webm" | "video/quicktime" => Some("video"),
        "audio/mpeg" | "audio/mp4" | "audio/wav" | "audio/ogg" | "audio/webm" => Some("audio"),
        _ => None,
    }
}

fn validate_media_item(item: &crate::WallPostMediaItemProto) -> Result<(), String> {
    validate_media_hash(&item.media_hash)?;
    let expected = expected_media_type_for_mime(&item.mime_type)
        .ok_or_else(|| format!("Unsupported media MIME type: {}", item.mime_type))?;
    if item.media_type != expected {
        return Err(format!(
            "Media type '{}' does not match MIME type '{}'",
            item.media_type, item.mime_type
        ));
    }
    if item.file_size <= 0 || item.file_size > MAX_POST_MEDIA_BYTES {
        return Err(format!(
            "Media '{}' is oversized or empty ({} bytes, max {} bytes)",
            item.file_name, item.file_size, MAX_POST_MEDIA_BYTES
        ));
    }
    if item.file_name.trim().is_empty() {
        return Err("Media file name is required".to_string());
    }
    if item.sort_order < 0 {
        return Err(format!("Invalid media sort_order {}", item.sort_order));
    }
    if matches!(item.width, Some(width) if width <= 0) {
        return Err("Media width must be positive when provided".to_string());
    }
    if matches!(item.height, Some(height) if height <= 0) {
        return Err("Media height must be positive when provided".to_string());
    }
    if matches!(item.duration_seconds, Some(duration) if duration <= 0) {
        return Err("Media duration_seconds must be positive when provided".to_string());
    }
    Ok(())
}

fn sorted_media_hashes(media_items: &[crate::WallPostMediaItemProto]) -> Vec<String> {
    let mut sorted = media_items.to_vec();
    sorted.sort_by_key(|item| item.sort_order);
    sorted.into_iter().map(|item| item.media_hash).collect()
}

fn signable_media_from_item(
    post_id: &str,
    author_peer_id: &str,
    item: &crate::WallPostMediaItemProto,
) -> SignablePostMedia {
    SignablePostMedia {
        post_id: post_id.to_string(),
        author_peer_id: author_peer_id.to_string(),
        media_hash: item.media_hash.clone(),
        media_type: item.media_type.clone(),
        mime_type: item.mime_type.clone(),
        file_name: item.file_name.clone(),
        file_size: item.file_size,
        width: item.width,
        height: item.height,
        duration_seconds: item.duration_seconds,
        sort_order: item.sort_order,
    }
}

// ============================================================
// Board service
// ============================================================

/// Service for processing board sync requests on the relay server
pub struct BoardService {
    db: RelayDatabase,
    community_name: String,
}

impl BoardService {
    pub fn new(db: RelayDatabase, community_name: String) -> Self {
        Self { db, community_name }
    }

    pub fn community_name(&self) -> &str {
        &self.community_name
    }

    /// Register a peer so they can post.
    ///
    /// For registration, the public key is provided in the request itself
    /// (this is the first time we see this peer), so we verify the signature
    /// against the supplied public key before storing it.
    #[allow(dead_code)] // Compatibility wrapper; the transport handler supplies explicit server time.
    pub fn process_register_peer(
        &self,
        peer_id: &str,
        public_key: &[u8],
        display_name: &str,
        timestamp: i64,
        signature: &[u8],
    ) -> Result<(), String> {
        self.process_register_peer_at(
            peer_id,
            public_key,
            display_name,
            timestamp,
            signature,
            chrono::Utc::now().timestamp(),
        )
        .map(|_| ())
    }

    pub fn process_register_peer_at(
        &self,
        peer_id: &str,
        public_key: &[u8],
        display_name: &str,
        timestamp: i64,
        signature: &[u8],
        server_now: i64,
    ) -> Result<String, String> {
        verify_registration_time(timestamp, server_now)
            .map_err(|binding_error| binding_error.code().to_string())?;
        verify_peer_key_binding(peer_id, public_key)
            .map_err(|binding_error| binding_error.code().to_string())?;
        if self
            .db
            .is_peer_banned(peer_id)
            .map_err(|_| RELAY_AUTH_DATABASE.to_string())?
        {
            return Err("Peer is banned".to_string());
        }

        // Verify the signature using the public key provided in the request.
        // This proves the registrant actually holds the corresponding private key.
        let signable_registration = SignablePeerRegistration {
            peer_id: peer_id.to_string(),
            display_name: display_name.to_string(),
            timestamp,
        };

        verify_signature(public_key, &signable_registration, signature).map_err(
            |verification_error| {
                warn!(
                    "RegisterPeer signature verification failed for {}: {}",
                    peer_id, verification_error
                );
                RELAY_PEER_SIGNATURE_INVALID.to_string()
            },
        )?;

        let identity_state = self
            .db
            .peer_identity_state(peer_id, server_now)
            .map_err(|_| "RELAY_PEER_DATABASE".to_string())?;
        self.db
            .register_peer(
                peer_id,
                public_key,
                display_name,
                timestamp,
                &identity_state,
                server_now,
            )
            .map_err(|store_error| store_error.code().to_string())?;

        info!(
            "Registered peer: {} ({}, identity_state={})",
            display_name, peer_id, identity_state
        );
        Ok(identity_state)
    }

    /// Submit a post to a board.
    ///
    /// Verifies the signature against the author's stored public key
    /// before accepting the post.
    #[allow(clippy::too_many_arguments)] // Relay protocol fields are passed through verbatim for signature verification.
    pub fn process_submit_post(
        &self,
        post_id: &str,
        board_id: &str,
        author_peer_id: &str,
        content_type: &str,
        content_text: Option<&str>,
        lamport_clock: u64,
        created_at: i64,
        signature: &[u8],
    ) -> Result<(), String> {
        if lamport_clock == 0 || lamport_clock > i64::MAX as u64 {
            return Err(RELAY_INTEGER_RANGE.to_string());
        }
        // Check peer is known
        if !self
            .db
            .is_peer_known(author_peer_id)
            .map_err(|_| RELAY_AUTH_DATABASE.to_string())?
        {
            return Err("Peer not registered. Call RegisterPeer first.".to_string());
        }

        // Check not banned
        if self
            .db
            .is_peer_banned(author_peer_id)
            .map_err(|_| RELAY_AUTH_DATABASE.to_string())?
        {
            return Err("Peer is banned".to_string());
        }

        // Check board exists
        if !self
            .db
            .board_exists(board_id)
            .map_err(|_| RELAY_AUTH_DATABASE.to_string())?
        {
            return Err(format!("Board {} does not exist", board_id));
        }

        // Verify signature against the author's stored public key.
        // This must happen before the database transaction so that we never
        // write a post whose signature is invalid.
        let signable_post = SignableBoardPost {
            post_id: post_id.to_string(),
            board_id: board_id.to_string(),
            author_peer_id: author_peer_id.to_string(),
            content_type: content_type.to_string(),
            content_text: content_text.map(|text| text.to_string()),
            lamport_clock,
            created_at,
        };

        verify_registered_peer_signature(&self.db, author_peer_id, &signable_post, signature)
            .map_err(|verification_error| {
                warn!(
                    "SubmitPost signature verification failed for post {} by {}: {}",
                    post_id, author_peer_id, verification_error
                );
                stable_verification_error(verification_error, "Signature verification failed")
            })?;

        // Atomically validate the lamport clock, insert the post, and advance
        // the clock high-water mark inside a single database transaction.
        // This eliminates TOCTOU races where two concurrent submissions from
        // the same author could both pass a non-atomic clock check.
        self.db
            .insert_post_with_clock_validation(
                post_id,
                board_id,
                author_peer_id,
                content_type,
                content_text,
                lamport_clock,
                created_at,
                signature,
            )
            .map_err(|validation_or_db_error| {
                warn!(
                    "Rejected post {} from {}: {}",
                    post_id, author_peer_id, validation_or_db_error
                );
                validation_or_db_error
            })?;

        info!(
            "Post {} accepted from {} on board {} (lamport_clock={})",
            post_id, author_peer_id, board_id, lamport_clock
        );
        Ok(())
    }

    /// List all boards.
    ///
    /// Verifies the requester's signature before returning data.
    /// The peer must be registered (so we have their public key on file).
    pub fn process_list_boards(
        &self,
        requester_peer_id: &str,
        timestamp: i64,
        signature: &[u8],
    ) -> Result<Vec<crate::db::BoardRow>, String> {
        // Verify signature for the requesting peer
        let signable_request = SignableBoardListRequest {
            requester_peer_id: requester_peer_id.to_string(),
            timestamp,
        };

        verify_registered_peer_signature(&self.db, requester_peer_id, &signable_request, signature)
            .map_err(|verification_error| {
                warn!(
                    "ListBoards signature verification failed for {}: {}",
                    requester_peer_id, verification_error
                );
                stable_verification_error(verification_error, RELAY_READ_SIGNATURE_INVALID)
            })?;

        self.db.list_boards().map_err(|db_error| {
            warn!("ListBoards database read failed: {}", db_error);
            RELAY_READ_DATABASE.to_string()
        })
    }

    /// Get paginated posts for a board.
    ///
    /// Verifies the requester's signature before returning data.
    pub fn process_get_board_posts(
        &self,
        requester_peer_id: &str,
        board_id: &str,
        after_timestamp: Option<i64>,
        limit: u32,
        timestamp: i64,
        signature: &[u8],
    ) -> Result<(Vec<crate::db::PostRow>, bool), String> {
        // Verify signature for the requesting peer
        let signable_request = SignableBoardPostsRequest {
            requester_peer_id: requester_peer_id.to_string(),
            board_id: board_id.to_string(),
            after_timestamp,
            limit,
            timestamp,
        };

        verify_registered_peer_signature(&self.db, requester_peer_id, &signable_request, signature)
            .map_err(|verification_error| {
                warn!(
                    "GetBoardPosts signature verification failed for {}: {}",
                    requester_peer_id, verification_error
                );
                stable_verification_error(verification_error, RELAY_READ_SIGNATURE_INVALID)
            })?;

        let clamped_limit = limit.min(100);
        let posts = self
            .db
            .get_board_posts(board_id, after_timestamp, clamped_limit + 1)
            .map_err(|db_error| {
                warn!("GetBoardPosts database read failed: {}", db_error);
                RELAY_READ_DATABASE.to_string()
            })?;

        let has_more = posts.len() > clamped_limit as usize;
        let posts = if has_more {
            posts[..clamped_limit as usize].to_vec()
        } else {
            posts
        };

        Ok((posts, has_more))
    }

    /// Get a stable newest-first page older than a compound cursor.
    pub fn process_get_older_board_posts(
        &self,
        requester_peer_id: &str,
        board_id: &str,
        before: Option<&BoardPostCursor>,
        limit: u32,
        timestamp: i64,
        signature: &[u8],
    ) -> Result<(Vec<crate::db::PostRow>, bool), String> {
        let signable_request = SignableOlderBoardPostsRequest {
            requester_peer_id: requester_peer_id.to_string(),
            board_id: board_id.to_string(),
            before: before.cloned(),
            limit,
            timestamp,
        };
        verify_registered_peer_signature(&self.db, requester_peer_id, &signable_request, signature)
            .map_err(|verification_error| {
                warn!(
                    "GetOlderBoardPosts signature verification failed for {}: {}",
                    requester_peer_id, verification_error
                );
                stable_verification_error(verification_error, RELAY_READ_SIGNATURE_INVALID)
            })?;

        let clamped_limit = limit.min(100);
        let posts = self
            .db
            .get_board_posts_older(board_id, before, clamped_limit + 1)
            .map_err(|db_error| {
                warn!("GetOlderBoardPosts database read failed: {}", db_error);
                RELAY_READ_DATABASE.to_string()
            })?;
        let has_more = posts.len() > clamped_limit as usize;
        let posts = if has_more {
            posts[..clamped_limit as usize].to_vec()
        } else {
            posts
        };
        Ok((posts, has_more))
    }

    /// Delete a post (author-only).
    ///
    /// Verifies the signature against the author's stored public key
    /// before deleting.
    pub fn process_delete_post(
        &self,
        post_id: &str,
        author_peer_id: &str,
        timestamp: i64,
        signature: &[u8],
    ) -> Result<(), String> {
        // Verify signature against the author's stored public key
        let signable_delete = SignableBoardPostDelete {
            post_id: post_id.to_string(),
            author_peer_id: author_peer_id.to_string(),
            timestamp,
        };

        verify_registered_peer_signature(&self.db, author_peer_id, &signable_delete, signature)
            .map_err(|verification_error| {
                warn!(
                    "DeletePost signature verification failed for post {} by {}: {}",
                    post_id, author_peer_id, verification_error
                );
                stable_verification_error(verification_error, "Signature verification failed")
            })?;

        let deleted = self
            .db
            .delete_post(post_id, author_peer_id)
            .map_err(|db_error| format!("Failed to delete post: {}", db_error))?;

        if !deleted {
            warn!(
                "Post {} not found or not owned by {}",
                post_id, author_peer_id
            );
            return Err("Post not found or not owned by you".to_string());
        }

        info!("Post {} deleted by {}", post_id, author_peer_id);
        Ok(())
    }

    // ============================================================
    // Wall post operations
    // ============================================================

    /// Submit a wall post for relay storage.
    ///
    /// Only the author can submit their own wall posts.  We verify the
    /// `request_signature` (which covers the entire request payload including
    /// the inner post `signature`) against the author's stored public key.
    #[allow(clippy::too_many_arguments)] // Mirrors the signed relay wire request without dropping fields.
    pub fn process_submit_wall_post(
        &self,
        author_peer_id: &str,
        post_id: &str,
        content_type: &str,
        content_text: Option<&str>,
        visibility: &str,
        lamport_clock: i64,
        created_at: i64,
        signature: &[u8],
        media_hashes: &[String],
        timestamp: i64,
        request_signature: &[u8],
        media_items: &[crate::WallPostMediaItemProto],
    ) -> Result<(), String> {
        // Check peer is known
        if !self
            .db
            .is_peer_known(author_peer_id)
            .map_err(|_| RELAY_AUTH_DATABASE.to_string())?
        {
            return Err("Peer not registered. Call RegisterPeer first.".to_string());
        }

        // Check not banned
        if self
            .db
            .is_peer_banned(author_peer_id)
            .map_err(|_| RELAY_AUTH_DATABASE.to_string())?
        {
            return Err("Peer is banned".to_string());
        }

        // Validate visibility
        if visibility != "public" && visibility != "contacts" {
            return Err(RELAY_POST_INVALID.to_string());
        }
        if lamport_clock <= 0 {
            return Err(RELAY_POST_INVALID.to_string());
        }

        // Verify request_signature against the author's stored public key.
        let signable_submit = SignableWallPostSubmit {
            author_peer_id: author_peer_id.to_string(),
            post_id: post_id.to_string(),
            content_type: content_type.to_string(),
            content_text: content_text.map(|t| t.to_string()),
            visibility: visibility.to_string(),
            lamport_clock,
            created_at,
            signature: signature.to_vec(),
            media_hashes: media_hashes.to_vec(),
            media_items: media_items.to_vec(),
            timestamp,
        };

        verify_registered_peer_signature(
            &self.db,
            author_peer_id,
            &signable_submit,
            request_signature,
        )
        .map_err(|verification_error| {
            warn!(
                "SubmitWallPost signature verification failed for post {} by {}: {}",
                post_id, author_peer_id, verification_error
            );
            stable_verification_error(verification_error, RELAY_POST_SIGNATURE_INVALID)
        })?;

        let lamport_clock_u64 =
            u64::try_from(lamport_clock).map_err(|_| RELAY_INTEGER_RANGE.to_string())?;
        let signable_post = SignablePost {
            post_id: post_id.to_string(),
            author_peer_id: author_peer_id.to_string(),
            content_type: content_type.to_string(),
            content_text: content_text.map(|t| t.to_string()),
            media_hashes: media_hashes.to_vec(),
            visibility: visibility.to_string(),
            lamport_clock: lamport_clock_u64,
            created_at,
        };
        verify_registered_peer_signature(&self.db, author_peer_id, &signable_post, signature)
            .map_err(|verification_error| {
                warn!(
                    "Inner wall post signature verification failed for post {} by {}: {}",
                    post_id, author_peer_id, verification_error
                );
                stable_verification_error(verification_error, RELAY_POST_SIGNATURE_INVALID)
            })?;

        let sorted_hashes = sorted_media_hashes(media_items);
        if sorted_hashes != media_hashes {
            return Err(RELAY_POST_INVALID.to_string());
        }

        let mut seen_orders = HashSet::new();
        let mut seen_hashes = HashSet::new();
        let mut total_media_bytes = 0i64;
        for item in media_items {
            validate_media_item(item).map_err(|error| {
                warn!("Invalid media metadata for post {}: {}", post_id, error);
                RELAY_POST_INVALID.to_string()
            })?;
            if !seen_orders.insert(item.sort_order) {
                return Err(RELAY_POST_INVALID.to_string());
            }
            if !seen_hashes.insert(item.media_hash.clone()) {
                return Err(RELAY_POST_INVALID.to_string());
            }
            total_media_bytes = total_media_bytes
                .checked_add(item.file_size)
                .filter(|total| *total <= MAX_POST_MEDIA_BYTES)
                .ok_or_else(|| RELAY_POST_INVALID.to_string())?;
            let signable_media = signable_media_from_item(post_id, author_peer_id, item);
            verify_registered_peer_signature(
                &self.db,
                author_peer_id,
                &signable_media,
                &item.signature,
            )
            .map_err(|verification_error| {
                warn!(
                    "Media metadata signature verification failed for post {} hash {}: {}",
                    post_id, item.media_hash, verification_error
                );
                stable_verification_error(verification_error, RELAY_POST_SIGNATURE_INVALID)
            })?;
        }

        let media_writes = media_items
            .iter()
            .map(|item| crate::db::WallPostMediaWrite {
                media_hash: &item.media_hash,
                media_type: &item.media_type,
                mime_type: &item.mime_type,
                file_name: &item.file_name,
                file_size: item.file_size,
                width: item.width,
                height: item.height,
                duration_seconds: item.duration_seconds,
                sort_order: item.sort_order,
                signature: &item.signature,
            })
            .collect::<Vec<_>>();
        self.db
            .write_wall_post_with_media(
                post_id,
                author_peer_id,
                content_type,
                content_text,
                visibility,
                lamport_clock,
                created_at,
                signature,
                &media_writes,
                chrono::Utc::now().timestamp(),
            )
            .map_err(|write_error| write_error.code().to_string())?;

        info!(
            "Wall post {} stored for {} (visibility={}, lamport_clock={}, media={})",
            post_id,
            author_peer_id,
            visibility,
            lamport_clock,
            media_items.len()
        );
        Ok(())
    }

    fn validate_wall_read_grant(
        &self,
        grant: &WallReadGrantProof,
        author_peer_id: &str,
        requester_peer_id: &str,
        now: i64,
    ) -> Result<(), String> {
        if grant.lamport_clock == 0 || grant.lamport_clock > i64::MAX as u64 {
            return Err(RELAY_INTEGER_RANGE.to_string());
        }
        if grant.issuer_peer_id != author_peer_id || grant.subject_peer_id != requester_peer_id {
            return Err("WallRead grant does not match requested author/requester".to_string());
        }
        if grant.capability != "wall_read" && grant.capability != "wall:read" {
            return Err(format!(
                "Invalid grant capability '{}': expected wall:read",
                grant.capability
            ));
        }
        if grant.scope.is_some() {
            return Err(RELAY_READ_SCOPE_UNSUPPORTED.to_string());
        }
        if let Some(expires_at) = grant.expires_at {
            if expires_at <= now {
                return Err("WallRead grant is expired".to_string());
            }
        }

        let signable = SignablePermissionGrant {
            grant_id: grant.grant_id.clone(),
            issuer_peer_id: grant.issuer_peer_id.clone(),
            subject_peer_id: grant.subject_peer_id.clone(),
            capability: grant.capability.clone(),
            scope: grant.scope.clone(),
            lamport_clock: grant.lamport_clock,
            issued_at: grant.issued_at,
            expires_at: grant.expires_at,
        };
        verify_registered_peer_signature(
            &self.db,
            &grant.issuer_peer_id,
            &signable,
            &grant.signature,
        )?;
        Ok(())
    }

    pub fn process_wall_read_grant(&self, grant: &WallReadGrantProof) -> Result<(), String> {
        self.validate_wall_read_grant(
            grant,
            &grant.issuer_peer_id,
            &grant.subject_peer_id,
            chrono::Utc::now().timestamp(),
        )?;
        let scope_json = grant.scope.as_ref().map(|scope| scope.to_string());
        self.db
            .upsert_wall_read_grant(
                &grant.grant_id,
                &grant.issuer_peer_id,
                &grant.subject_peer_id,
                &grant.capability,
                scope_json.as_deref(),
                grant.lamport_clock,
                grant.issued_at,
                grant.expires_at,
                &grant.signature,
            )
            .map_err(|db_error| format!("Failed to store WallRead grant: {}", db_error))?;
        Ok(())
    }

    pub fn process_wall_read_revoke(
        &self,
        grant_id: &str,
        issuer_peer_id: &str,
        lamport_clock: u64,
        revoked_at: i64,
        signature: &[u8],
    ) -> Result<(), String> {
        if lamport_clock == 0 || lamport_clock > i64::MAX as u64 {
            return Err(RELAY_INTEGER_RANGE.to_string());
        }
        let signable = SignablePermissionRevoke {
            grant_id: grant_id.to_string(),
            issuer_peer_id: issuer_peer_id.to_string(),
            lamport_clock,
            revoked_at,
        };
        verify_registered_peer_signature(&self.db, issuer_peer_id, &signable, signature)?;
        let revoked = self
            .db
            .revoke_wall_read_grant(grant_id, issuer_peer_id, lamport_clock, revoked_at)
            .map_err(|db_error| format!("Failed to revoke WallRead grant: {}", db_error))?;
        if !revoked {
            return Err("WallRead grant not found for issuer or already revoked".to_string());
        }
        Ok(())
    }

    /// Get wall posts for a specific author.
    ///
    /// Verifies the requester's signature before returning data.
    /// The requester must be a registered peer.
    #[allow(dead_code, clippy::type_complexity, clippy::too_many_arguments)] // Compatibility wrapper; production handlers supply explicit server time.
    pub fn process_get_wall_posts(
        &self,
        requester_peer_id: &str,
        author_peer_id: &str,
        since_lamport_clock: i64,
        limit: u32,
        timestamp: i64,
        signature: &[u8],
        grant_proof: Option<&WallReadGrantProof>,
    ) -> Result<
        (
            Vec<crate::db::WallPostRow>,
            bool,
            Vec<(String, Vec<crate::db::WallPostMediaRow>)>,
        ),
        String,
    > {
        self.process_get_wall_posts_at(
            requester_peer_id,
            author_peer_id,
            since_lamport_clock,
            limit,
            timestamp,
            signature,
            grant_proof,
            chrono::Utc::now().timestamp(),
        )
    }

    #[allow(clippy::type_complexity, clippy::too_many_arguments)]
    pub fn process_get_wall_posts_at(
        &self,
        requester_peer_id: &str,
        author_peer_id: &str,
        since_lamport_clock: i64,
        limit: u32,
        request_timestamp: i64,
        signature: &[u8],
        grant_proof: Option<&WallReadGrantProof>,
        server_now: i64,
    ) -> Result<
        (
            Vec<crate::db::WallPostRow>,
            bool,
            Vec<(String, Vec<crate::db::WallPostMediaRow>)>,
        ),
        String,
    > {
        // Verify the requester's signature
        let signable_request = SignableGetWallPosts {
            requester_peer_id: requester_peer_id.to_string(),
            author_peer_id: author_peer_id.to_string(),
            since_lamport_clock,
            limit,
            timestamp: request_timestamp,
        };

        verify_registered_peer_signature(&self.db, requester_peer_id, &signable_request, signature)
            .map_err(|verification_error| {
                warn!(
                    "GetWallPosts signature verification failed for {}: {}",
                    requester_peer_id, verification_error
                );
                stable_verification_error(verification_error, RELAY_READ_SIGNATURE_INVALID)
            })?;

        if let Some(grant) = grant_proof {
            self.validate_wall_read_grant(grant, author_peer_id, requester_peer_id, server_now)
                .map_err(|grant_error| {
                    if grant_error == RELAY_READ_SCOPE_UNSUPPORTED
                        || grant_error.starts_with("RELAY_PEER_")
                    {
                        grant_error
                    } else {
                        RELAY_READ_GRANT_INVALID.to_string()
                    }
                })?;
            let scope_json = grant.scope.as_ref().map(|scope| scope.to_string());
            self.db
                .upsert_wall_read_grant(
                    &grant.grant_id,
                    &grant.issuer_peer_id,
                    &grant.subject_peer_id,
                    &grant.capability,
                    scope_json.as_deref(),
                    grant.lamport_clock,
                    grant.issued_at,
                    grant.expires_at,
                    &grant.signature,
                )
                .map_err(|db_error| {
                    warn!("WallRead grant proof database write failed: {}", db_error);
                    RELAY_READ_DATABASE.to_string()
                })?;
        }

        let can_read_contacts = requester_peer_id == author_peer_id
            || self
                .db
                .has_active_wall_read_grant(author_peer_id, requester_peer_id, server_now)
                .map_err(|db_error| {
                    warn!("WallRead grant database lookup failed: {}", db_error);
                    RELAY_READ_DATABASE.to_string()
                })?;

        let clamped_limit = limit.min(100);
        let visible_posts = self
            .db
            .get_wall_posts(
                author_peer_id,
                since_lamport_clock,
                clamped_limit + 1,
                can_read_contacts,
            )
            .map_err(|db_error| {
                warn!("Wall post database read failed: {}", db_error);
                RELAY_READ_DATABASE.to_string()
            })?;

        let has_more = visible_posts.len() > clamped_limit as usize;
        let posts = if has_more {
            visible_posts[..clamped_limit as usize].to_vec()
        } else {
            visible_posts
        };

        let live_post_ids = posts
            .iter()
            .filter(|post| post.deleted_at.is_none())
            .map(|post| post.post_id.clone())
            .collect::<Vec<_>>();
        let media_map = self
            .db
            .get_wall_post_media_batch(&live_post_ids)
            .map_err(|db_error| {
                warn!("Wall post media batch read failed: {}", db_error);
                RELAY_READ_DATABASE.to_string()
            })?;

        Ok((posts, has_more, media_map))
    }

    pub fn process_submit_wall_social_event(
        &self,
        event: &crate::WallSocialEventItemProto,
        request_timestamp: i64,
        request_signature: &[u8],
    ) -> Result<(), String> {
        self.process_submit_wall_social_event_at(
            event,
            request_timestamp,
            request_signature,
            chrono::Utc::now().timestamp(),
        )
    }

    pub fn process_submit_wall_social_event_at(
        &self,
        event: &crate::WallSocialEventItemProto,
        request_timestamp: i64,
        request_signature: &[u8],
        server_now: i64,
    ) -> Result<(), String> {
        let (canonical_payload, expected_author_name) = canonical_social_payload(event)?;
        if request_timestamp < server_now.saturating_sub(300)
            || request_timestamp > server_now.saturating_add(30)
            || event.timestamp > server_now.saturating_add(30)
        {
            return Err("RELAY_SOCIAL_INVALID".into());
        }
        let signable = SignableWallSocialEventSubmit {
            event_id: event.event_id.clone(),
            event_type: event.event_type.clone(),
            post_id: event.post_id.clone(),
            actor_peer_id: event.actor_peer_id.clone(),
            author_name: event.author_name.clone(),
            comment_id: event.comment_id.clone(),
            content: event.content.clone(),
            reaction_type: event.reaction_type.clone(),
            timestamp: event.timestamp,
            payload_cbor: event.payload_cbor.clone(),
            signature: event.signature.clone(),
            request_timestamp,
        };
        verify_registered_peer_signature(
            &self.db,
            &event.actor_peer_id,
            &signable,
            request_signature,
        )
        .map_err(|error| stable_verification_error(error, "Signature verification failed"))?;
        verify_registered_peer_raw_signature(
            &self.db,
            &event.actor_peer_id,
            &canonical_payload,
            &event.signature,
        )
        .map_err(|error| stable_verification_error(error, "Event signature verification failed"))?;
        match self.db.insert_authorized_wall_social_event(
            &WallSocialEventRow {
                event_id: event.event_id.clone(),
                event_type: event.event_type.clone(),
                post_id: event.post_id.clone(),
                actor_peer_id: event.actor_peer_id.clone(),
                author_name: event.author_name.clone(),
                comment_id: event.comment_id.clone(),
                content: event.content.clone(),
                reaction_type: event.reaction_type.clone(),
                timestamp: event.timestamp,
                payload_cbor: event.payload_cbor.clone(),
                signature: event.signature.clone(),
            },
            expected_author_name,
            server_now,
        ) {
            Ok(WallSocialEventWriteOutcome::Inserted | WallSocialEventWriteOutcome::Duplicate) => {
                Ok(())
            }
            Err(error) => Err(error.code().to_string()),
        }
    }

    #[allow(dead_code, clippy::too_many_arguments)] // Compatibility wrapper; production handlers supply explicit server time.
    pub fn process_get_wall_social_events(
        &self,
        requester_peer_id: &str,
        author_peer_id: &str,
        post_ids: &[String],
        after_timestamp: i64,
        limit: u32,
        timestamp: i64,
        signature: &[u8],
    ) -> Result<(Vec<WallSocialEventRow>, bool, i64), String> {
        self.process_get_wall_social_events_at(
            requester_peer_id,
            author_peer_id,
            post_ids,
            after_timestamp,
            limit,
            timestamp,
            signature,
            chrono::Utc::now().timestamp(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn process_get_wall_social_events_at(
        &self,
        requester_peer_id: &str,
        author_peer_id: &str,
        post_ids: &[String],
        after_timestamp: i64,
        limit: u32,
        request_timestamp: i64,
        signature: &[u8],
        server_now: i64,
    ) -> Result<(Vec<WallSocialEventRow>, bool, i64), String> {
        let signable = SignableGetWallSocialEvents {
            requester_peer_id: requester_peer_id.to_string(),
            author_peer_id: author_peer_id.to_string(),
            post_ids: post_ids.to_vec(),
            after_timestamp,
            limit,
            timestamp: request_timestamp,
        };
        verify_registered_peer_signature(&self.db, requester_peer_id, &signable, signature)
            .map_err(|verification_error| {
                warn!(
                    "GetWallSocialEvents signature verification failed for {}: {}",
                    requester_peer_id, verification_error
                );
                stable_verification_error(verification_error, RELAY_READ_SIGNATURE_INVALID)
            })?;
        let can_read_contacts = requester_peer_id == author_peer_id
            || self
                .db
                .has_active_wall_read_grant(author_peer_id, requester_peer_id, server_now)
                .map_err(|db_error| {
                    warn!("Wall social grant database lookup failed: {}", db_error);
                    RELAY_READ_DATABASE.to_string()
                })?;
        let clamped = limit.min(500);
        let rows = self
            .db
            .get_wall_social_events(
                author_peer_id,
                post_ids,
                after_timestamp,
                clamped + 1,
                can_read_contacts,
            )
            .map_err(|db_error| {
                warn!("Wall social event database read failed: {}", db_error);
                RELAY_READ_DATABASE.to_string()
            })?;
        let has_more = rows.len() > clamped as usize;
        let events = if has_more {
            rows[..clamped as usize].to_vec()
        } else {
            rows
        };
        let next_timestamp = events
            .last()
            .map(|e| e.timestamp)
            .unwrap_or(after_timestamp);
        Ok((events, has_more, next_timestamp))
    }

    /// Delete a wall post (author-only).
    ///
    /// Verifies the signature against the author's stored public key
    /// before deleting.
    pub fn process_delete_wall_post(
        &self,
        author_peer_id: &str,
        post_id: &str,
        lamport_clock: u64,
        deleted_at: i64,
        signature: &[u8],
    ) -> Result<(), String> {
        // Verify signature against the author's stored public key
        let signable_delete = SignableWallPostDelete {
            post_id: post_id.to_string(),
            author_peer_id: author_peer_id.to_string(),
            lamport_clock,
            deleted_at,
        };

        verify_registered_peer_signature(&self.db, author_peer_id, &signable_delete, signature)
            .map_err(|verification_error| {
                warn!(
                    "DeleteWallPost signature verification failed for post {} by {}: {}",
                    post_id, author_peer_id, verification_error
                );
                stable_verification_error(verification_error, RELAY_POST_SIGNATURE_INVALID)
            })?;

        let lamport_clock =
            i64::try_from(lamport_clock).map_err(|_| RELAY_POST_INVALID.to_string())?;
        if lamport_clock <= 0 {
            return Err(RELAY_POST_INVALID.to_string());
        }
        self.db
            .tombstone_wall_post(
                post_id,
                author_peer_id,
                lamport_clock,
                deleted_at,
                signature,
                chrono::Utc::now().timestamp(),
            )
            .map_err(|write_error| write_error.code().to_string())?;

        info!(
            "Wall post {} tombstoned by {} (lamport_clock={}, deleted_at={})",
            post_id, author_peer_id, lamport_clock, deleted_at
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn signing_key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    fn author() -> &'static str {
        static PEER_ID: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        PEER_ID
            .get_or_init(|| {
                crate::peer_binding::peer_id_for_ed25519(&signing_key(1).verifying_key().to_bytes())
                    .unwrap()
                    .to_string()
            })
            .as_str()
    }

    fn requester() -> &'static str {
        static PEER_ID: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        PEER_ID
            .get_or_init(|| {
                crate::peer_binding::peer_id_for_ed25519(&signing_key(2).verifying_key().to_bytes())
                    .unwrap()
                    .to_string()
            })
            .as_str()
    }

    fn register(service: &BoardService, peer_id: &str, key: &SigningKey) {
        service
            .db
            .register_peer(
                peer_id,
                &key.verifying_key().to_bytes(),
                peer_id,
                1,
                "unverified",
                1,
            )
            .unwrap();
    }

    fn sign<T: Signable>(key: &SigningKey, value: &T) -> Vec<u8> {
        key.sign(&value.signable_bytes().unwrap())
            .to_bytes()
            .to_vec()
    }

    fn sign_social_request(
        key: &SigningKey,
        event: &crate::WallSocialEventItemProto,
        request_timestamp: i64,
    ) -> Vec<u8> {
        sign(
            key,
            &SignableWallSocialEventSubmit {
                event_id: event.event_id.clone(),
                event_type: event.event_type.clone(),
                post_id: event.post_id.clone(),
                actor_peer_id: event.actor_peer_id.clone(),
                author_name: event.author_name.clone(),
                comment_id: event.comment_id.clone(),
                content: event.content.clone(),
                reaction_type: event.reaction_type.clone(),
                timestamp: event.timestamp,
                payload_cbor: event.payload_cbor.clone(),
                signature: event.signature.clone(),
                request_timestamp,
            },
        )
    }

    fn comment_event(
        key: &SigningKey,
        actor_peer_id: &str,
        post_id: &str,
        author_name: &str,
        content: &str,
        timestamp: i64,
    ) -> crate::WallSocialEventItemProto {
        let event_id = uuid::Uuid::new_v4().to_string();
        let comment_id = uuid::Uuid::new_v4().to_string();
        let payload = SignableWallCommentCreate {
            event_id: event_id.clone(),
            post_id: post_id.into(),
            comment_id: comment_id.clone(),
            actor_peer_id: actor_peer_id.into(),
            author_name: author_name.into(),
            content: content.into(),
            timestamp,
        };
        crate::WallSocialEventItemProto {
            event_id,
            event_type: "comment_create".into(),
            post_id: post_id.into(),
            actor_peer_id: actor_peer_id.into(),
            author_name: Some(author_name.into()),
            comment_id: Some(comment_id),
            content: Some(content.into()),
            reaction_type: None,
            timestamp,
            payload_cbor: payload.signable_bytes().unwrap(),
            signature: sign(key, &payload),
        }
    }

    fn reaction_event(
        key: &SigningKey,
        actor_peer_id: &str,
        post_id: &str,
        event_type: &str,
        reaction_type: &str,
        timestamp: i64,
    ) -> crate::WallSocialEventItemProto {
        let event_id = uuid::Uuid::new_v4().to_string();
        let payload = SignableWallReaction {
            event_id: event_id.clone(),
            post_id: post_id.into(),
            actor_peer_id: actor_peer_id.into(),
            reaction_type: reaction_type.into(),
            timestamp,
        };
        crate::WallSocialEventItemProto {
            event_id,
            event_type: event_type.into(),
            post_id: post_id.into(),
            actor_peer_id: actor_peer_id.into(),
            author_name: None,
            comment_id: None,
            content: None,
            reaction_type: Some(reaction_type.into()),
            timestamp,
            payload_cbor: payload.signable_bytes().unwrap(),
            signature: sign(key, &payload),
        }
    }

    fn comment_delete_event(
        key: &SigningKey,
        actor_peer_id: &str,
        post_id: &str,
        comment_id: &str,
        timestamp: i64,
    ) -> crate::WallSocialEventItemProto {
        let event_id = uuid::Uuid::new_v4().to_string();
        let payload = SignableWallCommentDelete {
            event_id: event_id.clone(),
            post_id: post_id.into(),
            comment_id: comment_id.into(),
            actor_peer_id: actor_peer_id.into(),
            timestamp,
        };
        crate::WallSocialEventItemProto {
            event_id,
            event_type: "comment_delete".into(),
            post_id: post_id.into(),
            actor_peer_id: actor_peer_id.into(),
            author_name: None,
            comment_id: Some(comment_id.into()),
            content: None,
            reaction_type: None,
            timestamp,
            payload_cbor: payload.signable_bytes().unwrap(),
            signature: sign(key, &payload),
        }
    }

    fn signed_registration(
        key: &SigningKey,
        peer_id: &str,
        display_name: &str,
        timestamp: i64,
    ) -> Vec<u8> {
        sign(
            key,
            &SignablePeerRegistration {
                peer_id: peer_id.to_string(),
                display_name: display_name.to_string(),
                timestamp,
            },
        )
    }

    #[test]
    fn registration_binds_signing_key_to_peer_and_derives_identity_state() {
        let service =
            BoardService::new(RelayDatabase::open(":memory:").unwrap(), "test".to_string());
        let key = signing_key(7);
        let peer = crate::peer_binding::peer_id_for_ed25519(&key.verifying_key().to_bytes())
            .unwrap()
            .to_string();
        let timestamp = 1_000;
        let signature = signed_registration(&key, &peer, "Alice", timestamp);

        let state = service
            .process_register_peer_at(
                &peer,
                &key.verifying_key().to_bytes(),
                "Alice",
                timestamp,
                &signature,
                timestamp,
            )
            .unwrap();

        assert_eq!(state, "unverified");
        assert_eq!(
            service.db.get_peer_public_key(&peer).unwrap(),
            Some(key.verifying_key().to_bytes().to_vec())
        );

        let verified_key = signing_key(8);
        let verified_peer =
            crate::peer_binding::peer_id_for_ed25519(&verified_key.verifying_key().to_bytes())
                .unwrap()
                .to_string();
        service.db.with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO relay_name_claims(local_name, relay, peer_id, sequence, claim_cbor, not_before, not_after, relay_key_id, status, created_at, retired_at)
                     VALUES('alice', 'relay.test', ?, 1, X'', 900, 1100, 'key-1', 'active', 900, NULL)",
                    [&verified_peer],
                )
                .unwrap();
        });
        let verified_signature =
            signed_registration(&verified_key, &verified_peer, "Alice", timestamp);
        assert_eq!(
            service
                .process_register_peer_at(
                    &verified_peer,
                    &verified_key.verifying_key().to_bytes(),
                    "Alice",
                    timestamp,
                    &verified_signature,
                    timestamp,
                )
                .unwrap(),
            "verified"
        );
    }

    #[test]
    fn registration_rejects_cross_peer_and_rotation_key_substitution_before_storage() {
        let service =
            BoardService::new(RelayDatabase::open(":memory:").unwrap(), "test".to_string());
        let original = signing_key(9);
        let substitute = signing_key(10);
        let peer = crate::peer_binding::peer_id_for_ed25519(&original.verifying_key().to_bytes())
            .unwrap()
            .to_string();
        let timestamp = 1_000;
        let forged_signature = signed_registration(&substitute, &peer, "Mallory", timestamp);

        assert_eq!(
            service.process_register_peer_at(
                &peer,
                &substitute.verifying_key().to_bytes(),
                "Mallory",
                timestamp,
                &forged_signature,
                timestamp,
            ),
            Err("RELAY_PEER_KEY_MISMATCH".to_string())
        );
        assert_eq!(service.db.get_peer_public_key(&peer).unwrap(), None);

        let original_signature = signed_registration(&original, &peer, "Alice", timestamp);
        service
            .process_register_peer_at(
                &peer,
                &original.verifying_key().to_bytes(),
                "Alice",
                timestamp,
                &original_signature,
                timestamp,
            )
            .unwrap();
        let rotated_signature = signed_registration(&substitute, &peer, "Alice", timestamp + 1);
        assert_eq!(
            service.process_register_peer_at(
                &peer,
                &substitute.verifying_key().to_bytes(),
                "Alice",
                timestamp + 1,
                &rotated_signature,
                timestamp + 1,
            ),
            Err("RELAY_PEER_KEY_MISMATCH".to_string())
        );
        assert_eq!(
            service.db.get_peer_public_key(&peer).unwrap(),
            Some(original.verifying_key().to_bytes().to_vec())
        );
    }

    #[test]
    fn registration_sequence_and_key_binding_survive_restart() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("relay.db");
        let path_string = path.to_string_lossy().into_owned();
        let key = signing_key(11);
        let peer = crate::peer_binding::peer_id_for_ed25519(&key.verifying_key().to_bytes())
            .unwrap()
            .to_string();
        let timestamp = 1_000;
        {
            let service = BoardService::new(
                RelayDatabase::open(&path_string).unwrap(),
                "test".to_string(),
            );
            let signature = signed_registration(&key, &peer, "Alice", timestamp);
            service
                .process_register_peer_at(
                    &peer,
                    &key.verifying_key().to_bytes(),
                    "Alice",
                    timestamp,
                    &signature,
                    timestamp,
                )
                .unwrap();
        }

        let reopened = BoardService::new(
            RelayDatabase::open(&path_string).unwrap(),
            "test".to_string(),
        );
        let replay = signed_registration(&key, &peer, "Changed", timestamp);
        assert_eq!(
            reopened.process_register_peer_at(
                &peer,
                &key.verifying_key().to_bytes(),
                "Changed",
                timestamp,
                &replay,
                timestamp,
            ),
            Err("RELAY_PEER_REGISTRATION_STALE".to_string())
        );
        let fresh = signed_registration(&key, &peer, "Changed", timestamp + 1);
        assert!(reopened
            .process_register_peer_at(
                &peer,
                &key.verifying_key().to_bytes(),
                "Changed",
                timestamp + 1,
                &fresh,
                timestamp + 1,
            )
            .is_ok());
    }

    #[test]
    fn imported_key_mismatch_is_rejected_on_lookup_and_database_failure_denies_registration() {
        let service =
            BoardService::new(RelayDatabase::open(":memory:").unwrap(), "test".to_string());
        let key = signing_key(12);
        let substitute = signing_key(13);
        let peer = crate::peer_binding::peer_id_for_ed25519(&key.verifying_key().to_bytes())
            .unwrap()
            .to_string();
        register(&service, &peer, &key);
        service.db.with_connection(|connection| {
            connection
                .execute(
                    "UPDATE known_peers SET public_key = ? WHERE peer_id = ?",
                    rusqlite::params![substitute.verifying_key().to_bytes().to_vec(), peer],
                )
                .unwrap();
        });
        let timestamp = 2_000;
        let list_signature = sign(
            &key,
            &SignableBoardListRequest {
                requester_peer_id: peer.clone(),
                timestamp,
            },
        );
        assert!(matches!(
            service.process_list_boards(&peer, timestamp, &list_signature),
            Err(error) if error == "RELAY_PEER_KEY_MISMATCH"
        ));

        let failing =
            BoardService::new(RelayDatabase::open(":memory:").unwrap(), "test".to_string());
        failing.db.with_connection(|connection| {
            connection.execute("DROP TABLE known_peers", []).unwrap();
        });
        let signature = signed_registration(&key, &peer, "Alice", timestamp);
        assert_eq!(
            failing.process_register_peer_at(
                &peer,
                &key.verifying_key().to_bytes(),
                "Alice",
                timestamp,
                &signature,
                timestamp,
            ),
            Err("RELAY_PEER_DATABASE".to_string())
        );
    }

    fn signed_get(
        requester_key: &SigningKey,
        requester_peer_id: &str,
        author_peer_id: &str,
        timestamp: i64,
    ) -> Vec<u8> {
        signed_get_with(
            requester_key,
            requester_peer_id,
            author_peer_id,
            0,
            20,
            timestamp,
        )
    }

    fn signed_get_with(
        requester_key: &SigningKey,
        requester_peer_id: &str,
        author_peer_id: &str,
        since_lamport_clock: i64,
        limit: u32,
        timestamp: i64,
    ) -> Vec<u8> {
        sign(
            requester_key,
            &SignableGetWallPosts {
                requester_peer_id: requester_peer_id.to_string(),
                author_peer_id: author_peer_id.to_string(),
                since_lamport_clock,
                limit,
                timestamp,
            },
        )
    }

    fn signed_delete(
        author_key: &SigningKey,
        post_id: &str,
        author_peer_id: &str,
        lamport_clock: u64,
        deleted_at: i64,
    ) -> Vec<u8> {
        sign(
            author_key,
            &SignableWallPostDelete {
                post_id: post_id.to_string(),
                author_peer_id: author_peer_id.to_string(),
                lamport_clock,
                deleted_at,
            },
        )
    }

    fn signed_get_social(
        requester_key: &SigningKey,
        requester_peer_id: &str,
        author_peer_id: &str,
        post_ids: &[String],
        timestamp: i64,
    ) -> Vec<u8> {
        sign(
            requester_key,
            &SignableGetWallSocialEvents {
                requester_peer_id: requester_peer_id.to_string(),
                author_peer_id: author_peer_id.to_string(),
                post_ids: post_ids.to_vec(),
                after_timestamp: 0,
                limit: 20,
                timestamp,
            },
        )
    }

    fn wall_read_grant(
        author_key: &SigningKey,
        author_peer_id: &str,
        requester_peer_id: &str,
    ) -> WallReadGrantProof {
        let mut grant = WallReadGrantProof {
            grant_id: "grant-wall-read-1".to_string(),
            issuer_peer_id: author_peer_id.to_string(),
            subject_peer_id: requester_peer_id.to_string(),
            capability: "wall_read".to_string(),
            scope: None,
            lamport_clock: 3,
            issued_at: 1_000,
            expires_at: None,
            signature: Vec::new(),
        };
        grant.signature = sign(
            author_key,
            &SignablePermissionGrant {
                grant_id: grant.grant_id.clone(),
                issuer_peer_id: grant.issuer_peer_id.clone(),
                subject_peer_id: grant.subject_peer_id.clone(),
                capability: grant.capability.clone(),
                scope: grant.scope.clone(),
                lamport_clock: grant.lamport_clock,
                issued_at: grant.issued_at,
                expires_at: grant.expires_at,
            },
        );
        grant
    }

    fn signed_media_item(
        key: &SigningKey,
        post_id: &str,
        author_peer_id: &str,
        hash_byte: char,
        sort_order: i32,
    ) -> crate::WallPostMediaItemProto {
        let mut item = crate::WallPostMediaItemProto {
            media_hash: hash_byte.to_string().repeat(64),
            media_type: "image".into(),
            mime_type: "image/png".into(),
            file_name: format!("image-{sort_order}.png"),
            file_size: 128,
            width: Some(10),
            height: Some(10),
            duration_seconds: None,
            sort_order,
            signature: Vec::new(),
        };
        item.signature = sign(
            key,
            &signable_media_from_item(post_id, author_peer_id, &item),
        );
        item
    }

    fn submit_wall(
        service: &BoardService,
        key: &SigningKey,
        author_peer_id: &str,
        post_id: &str,
        lamport_clock: i64,
        content: &str,
        media_items: &[crate::WallPostMediaItemProto],
    ) -> Result<(), String> {
        let media_hashes = sorted_media_hashes(media_items);
        let created_at = 1_000 + lamport_clock;
        let timestamp = 2_000 + lamport_clock;
        let post_signature = sign(
            key,
            &SignablePost {
                post_id: post_id.to_string(),
                author_peer_id: author_peer_id.to_string(),
                content_type: "text".into(),
                content_text: Some(content.to_string()),
                media_hashes: media_hashes.clone(),
                visibility: "public".into(),
                lamport_clock: u64::try_from(lamport_clock).unwrap(),
                created_at,
            },
        );
        let request_signature = sign(
            key,
            &SignableWallPostSubmit {
                author_peer_id: author_peer_id.to_string(),
                post_id: post_id.to_string(),
                content_type: "text".into(),
                content_text: Some(content.to_string()),
                visibility: "public".into(),
                lamport_clock,
                created_at,
                signature: post_signature.clone(),
                media_hashes: media_hashes.clone(),
                media_items: media_items.to_vec(),
                timestamp,
            },
        );
        service.process_submit_wall_post(
            author_peer_id,
            post_id,
            "text",
            Some(content),
            "public",
            lamport_clock,
            created_at,
            &post_signature,
            &media_hashes,
            timestamp,
            &request_signature,
            media_items,
        )
    }

    #[test]
    fn wall_post_ids_are_author_bound_and_replays_are_typed_conflicts() {
        let (service, author_key, requester_key) = service_with_wall_posts();
        let attacker_media = [signed_media_item(
            &requester_key,
            "public-post",
            requester(),
            'c',
            0,
        )];
        assert_eq!(
            submit_wall(
                &service,
                &requester_key,
                requester(),
                "public-post",
                10,
                "stolen",
                &attacker_media,
            ),
            Err("RELAY_POST_OWNER_CONFLICT".to_string())
        );
        let original = service
            .db
            .get_wall_posts(author(), 0, 10, true)
            .unwrap()
            .into_iter()
            .find(|post| post.post_id == "public-post")
            .unwrap();
        assert_eq!(original.author_peer_id, author());
        assert_eq!(original.content_text.as_deref(), Some("Public"));

        let media = [signed_media_item(
            &author_key,
            "replay-post",
            author(),
            'd',
            0,
        )];
        submit_wall(
            &service,
            &author_key,
            author(),
            "replay-post",
            10,
            "once",
            &media,
        )
        .unwrap();
        assert_eq!(
            submit_wall(
                &service,
                &author_key,
                author(),
                "replay-post",
                10,
                "once",
                &media,
            ),
            Err("RELAY_POST_STALE_CLOCK".to_string())
        );
        let replacement_media = [signed_media_item(
            &author_key,
            "replay-post",
            author(),
            '9',
            0,
        )];
        submit_wall(
            &service,
            &author_key,
            author(),
            "replay-post",
            11,
            "updated",
            &replacement_media,
        )
        .unwrap();
        let updated = service
            .db
            .get_wall_posts(author(), 0, 20, true)
            .unwrap()
            .into_iter()
            .find(|post| post.post_id == "replay-post")
            .unwrap();
        assert_eq!(updated.lamport_clock, 11);
        assert_eq!(updated.content_text.as_deref(), Some("updated"));
        let updated_media = service
            .db
            .get_wall_post_media_batch(&["replay-post".into()])
            .unwrap();
        assert_eq!(updated_media[0].1[0].media_hash, "9".repeat(64));
    }

    #[test]
    fn media_failure_rolls_back_create_and_update_without_partial_rows() {
        let (service, author_key, _requester_key) = service_with_wall_posts();
        let initial_media = [signed_media_item(
            &author_key,
            "atomic-post",
            author(),
            'e',
            0,
        )];
        service.db.with_connection(|connection| {
            connection
                .execute_batch(
                    "CREATE TRIGGER fail_media BEFORE INSERT ON wall_post_media
                     BEGIN SELECT RAISE(ABORT, 'injected media failure'); END;",
                )
                .unwrap();
        });
        assert_eq!(
            submit_wall(
                &service,
                &author_key,
                author(),
                "atomic-post",
                10,
                "create",
                &initial_media,
            ),
            Err("RELAY_POST_DATABASE".to_string())
        );
        service.db.with_connection(|connection| {
            let posts: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM wall_posts WHERE post_id='atomic-post'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            let media: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM wall_post_media WHERE post_id='atomic-post'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!((posts, media), (0, 0));
            connection.execute("DROP TRIGGER fail_media", []).unwrap();
        });

        submit_wall(
            &service,
            &author_key,
            author(),
            "atomic-post",
            10,
            "before",
            &initial_media,
        )
        .unwrap();
        let updated_media = [signed_media_item(
            &author_key,
            "atomic-post",
            author(),
            'f',
            0,
        )];
        service.db.with_connection(|connection| {
            connection
                .execute_batch(
                    "CREATE TRIGGER fail_media BEFORE INSERT ON wall_post_media
                     BEGIN SELECT RAISE(ABORT, 'injected media failure'); END;",
                )
                .unwrap();
        });
        assert_eq!(
            submit_wall(
                &service,
                &author_key,
                author(),
                "atomic-post",
                11,
                "after",
                &updated_media,
            ),
            Err("RELAY_POST_DATABASE".to_string())
        );
        let stored = service
            .db
            .get_wall_posts(author(), 0, 20, true)
            .unwrap()
            .into_iter()
            .find(|post| post.post_id == "atomic-post")
            .unwrap();
        assert_eq!(stored.lamport_clock, 10);
        assert_eq!(stored.content_text.as_deref(), Some("before"));
        let stored_media = service
            .db
            .get_wall_post_media_batch(&["atomic-post".into()])
            .unwrap();
        assert_eq!(stored_media[0].1[0].media_hash, "e".repeat(64));
    }

    #[test]
    fn malformed_or_wrongly_signed_media_is_rejected_before_any_write() {
        let (service, author_key, requester_key) = service_with_wall_posts();
        let wrong_signature = [signed_media_item(
            &requester_key,
            "bad-signature-post",
            author(),
            '1',
            0,
        )];
        assert_eq!(
            submit_wall(
                &service,
                &author_key,
                author(),
                "bad-signature-post",
                10,
                "bad signature",
                &wrong_signature,
            ),
            Err(RELAY_POST_SIGNATURE_INVALID.to_string())
        );
        let mut malformed =
            signed_media_item(&author_key, "malformed-media-post", author(), '2', 0);
        malformed.media_hash = "not-a-sha256".into();
        assert_eq!(
            submit_wall(
                &service,
                &author_key,
                author(),
                "malformed-media-post",
                10,
                "bad metadata",
                &[malformed],
            ),
            Err(RELAY_POST_INVALID.to_string())
        );
        service.db.with_connection(|connection| {
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM wall_posts WHERE post_id IN ('bad-signature-post','malformed-media-post')",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 0);
        });
    }

    #[test]
    fn delete_is_author_bound_monotonic_atomic_and_idempotent() {
        let (service, author_key, requester_key) = service_with_wall_posts();
        let media = [signed_media_item(
            &author_key,
            "delete-post",
            author(),
            'a',
            0,
        )];
        submit_wall(
            &service,
            &author_key,
            author(),
            "delete-post",
            10,
            "delete me",
            &media,
        )
        .unwrap();

        let attacker_signature =
            signed_delete(&requester_key, "delete-post", requester(), 11, 3_000);
        assert_eq!(
            service.process_delete_wall_post(
                requester(),
                "delete-post",
                11,
                3_000,
                &attacker_signature,
            ),
            Err("RELAY_POST_OWNER_CONFLICT".to_string())
        );
        let stale_signature = signed_delete(&author_key, "delete-post", author(), 9, 3_000);
        assert_eq!(
            service.process_delete_wall_post(author(), "delete-post", 9, 3_000, &stale_signature,),
            Err("RELAY_POST_STALE_CLOCK".to_string())
        );

        let delete_signature = signed_delete(&author_key, "delete-post", author(), 11, 3_001);
        service.db.with_connection(|connection| {
            connection
                .execute_batch(
                    "CREATE TRIGGER fail_delete BEFORE UPDATE OF deleted_at ON wall_posts
                     WHEN NEW.post_id = 'delete-post'
                     BEGIN SELECT RAISE(ABORT, 'injected delete failure'); END;",
                )
                .unwrap();
        });
        assert_eq!(
            service
                .process_delete_wall_post(author(), "delete-post", 11, 3_001, &delete_signature,),
            Err("RELAY_POST_DATABASE".to_string())
        );
        let after_failed_delete = service
            .db
            .get_wall_posts(author(), 0, 20, true)
            .unwrap()
            .into_iter()
            .find(|post| post.post_id == "delete-post")
            .unwrap();
        assert!(after_failed_delete.deleted_at.is_none());
        assert_eq!(
            service
                .db
                .get_wall_post_media_batch(&["delete-post".into()])
                .unwrap()[0]
                .1
                .len(),
            1
        );
        service.db.with_connection(|connection| {
            connection.execute("DROP TRIGGER fail_delete", []).unwrap();
        });
        service
            .process_delete_wall_post(author(), "delete-post", 11, 3_001, &delete_signature)
            .unwrap();
        service
            .process_delete_wall_post(author(), "delete-post", 11, 3_001, &delete_signature)
            .unwrap();
        assert!(service
            .db
            .get_wall_post_media_batch(&["delete-post".into()])
            .unwrap()
            .is_empty());
        assert_eq!(
            submit_wall(
                &service,
                &author_key,
                author(),
                "delete-post",
                12,
                "resurrect",
                &[],
            ),
            Err("RELAY_POST_TOMBSTONED".to_string())
        );
    }

    #[test]
    fn wall_post_and_tombstone_integrity_survive_restart() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("relay.db");
        let path_string = path.to_string_lossy().into_owned();
        let key = signing_key(21);
        let peer = crate::peer_binding::peer_id_for_ed25519(&key.verifying_key().to_bytes())
            .unwrap()
            .to_string();
        let delete_signature = signed_delete(&key, "restart-post", &peer, 11, 3_001);
        {
            let service = BoardService::new(
                RelayDatabase::open(&path_string).unwrap(),
                "test".to_string(),
            );
            register(&service, &peer, &key);
            let media = [signed_media_item(&key, "restart-post", &peer, 'b', 0)];
            submit_wall(
                &service,
                &key,
                &peer,
                "restart-post",
                10,
                "before restart",
                &media,
            )
            .unwrap();
            service
                .process_delete_wall_post(&peer, "restart-post", 11, 3_001, &delete_signature)
                .unwrap();
        }
        let reopened = BoardService::new(
            RelayDatabase::open(&path_string).unwrap(),
            "test".to_string(),
        );
        let rows = reopened.db.get_wall_posts(&peer, 0, 10, true).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].deleted_at, Some(3_001));
        assert_eq!(rows[0].lamport_clock, 11);
        assert!(reopened
            .db
            .get_wall_post_media_batch(&["restart-post".into()])
            .unwrap()
            .is_empty());
        let stale = signed_delete(&key, "restart-post", &peer, 10, 3_002);
        assert_eq!(
            reopened.process_delete_wall_post(&peer, "restart-post", 10, 3_002, &stale),
            Err("RELAY_POST_STALE_CLOCK".to_string())
        );
    }

    fn service_with_wall_posts() -> (BoardService, SigningKey, SigningKey) {
        let service =
            BoardService::new(RelayDatabase::open(":memory:").unwrap(), "test".to_string());
        let author_key = signing_key(1);
        let requester_key = signing_key(2);
        register(&service, author(), &author_key);
        register(&service, requester(), &requester_key);
        service
            .db
            .write_wall_post_with_media(
                "public-post",
                author(),
                "text",
                Some("Public"),
                "public",
                1,
                1_000,
                &[7; 64],
                &[],
                1_000,
            )
            .unwrap();
        service
            .db
            .write_wall_post_with_media(
                "contacts-post",
                author(),
                "text",
                Some("Contacts"),
                "contacts",
                2,
                2_000,
                &[8; 64],
                &[],
                2_000,
            )
            .unwrap();
        (service, author_key, requester_key)
    }

    fn social_event_count(service: &BoardService) -> i64 {
        service.db.with_connection(|connection| {
            connection
                .query_row("SELECT COUNT(*) FROM wall_social_events", [], |row| {
                    row.get(0)
                })
                .unwrap()
        })
    }

    #[test]
    fn noncanonical_social_events_are_table_rejected_without_storage() {
        let (service, _author_key, requester_key) = service_with_wall_posts();
        let base = comment_event(
            &requester_key,
            requester(),
            "public-post",
            requester(),
            "hello",
            3_000,
        );
        let mut unknown_type = base.clone();
        unknown_type.event_type = "comment".into();
        let mut mismatched_payload = base.clone();
        mismatched_payload.payload_cbor.push(0);
        let mut missing_field = base.clone();
        missing_field.content = None;
        let mut mixed_fields = base.clone();
        mixed_fields.reaction_type = Some("like".into());
        let mut noncanonical_id = base.clone();
        noncanonical_id.event_id = base.event_id.to_uppercase();

        for (case, event) in [
            ("unknown type", unknown_type),
            ("payload mismatch", mismatched_payload),
            ("missing field", missing_field),
            ("mixed fields", mixed_fields),
            ("noncanonical id", noncanonical_id),
        ] {
            let request_signature = sign_social_request(&requester_key, &event, 3_000);
            assert!(
                service
                    .process_submit_wall_social_event_at(&event, 3_000, &request_signature, 3_000,)
                    .is_err(),
                "{case} was accepted"
            );
            assert_eq!(social_event_count(&service), 0, "{case} inserted a row");
        }
    }

    #[test]
    fn unauthorized_or_identity_mismatched_social_events_are_table_rejected() {
        for case in [
            "missing post",
            "private post",
            "forged author label",
            "orphan comment delete",
            "orphan reaction remove",
            "stale verified state",
            "future event",
        ] {
            let (service, _author_key, requester_key) = service_with_wall_posts();
            let event = match case {
                "missing post" => comment_event(
                    &requester_key,
                    requester(),
                    "missing-post",
                    requester(),
                    "hello",
                    3_000,
                ),
                "private post" => comment_event(
                    &requester_key,
                    requester(),
                    "contacts-post",
                    requester(),
                    "hello",
                    3_000,
                ),
                "forged author label" => comment_event(
                    &requester_key,
                    requester(),
                    "public-post",
                    "Mallory",
                    "hello",
                    3_000,
                ),
                "orphan comment delete" => comment_delete_event(
                    &requester_key,
                    requester(),
                    "public-post",
                    &uuid::Uuid::new_v4().to_string(),
                    3_000,
                ),
                "orphan reaction remove" => reaction_event(
                    &requester_key,
                    requester(),
                    "public-post",
                    "reaction_remove",
                    "like",
                    3_000,
                ),
                "stale verified state" => {
                    service.db.with_connection(|connection| {
                        connection
                            .execute(
                                "UPDATE known_peers SET identity_state='verified' WHERE peer_id=?",
                                [requester()],
                            )
                            .unwrap();
                    });
                    reaction_event(
                        &requester_key,
                        requester(),
                        "public-post",
                        "reaction_add",
                        "like",
                        3_000,
                    )
                }
                "future event" => reaction_event(
                    &requester_key,
                    requester(),
                    "public-post",
                    "reaction_add",
                    "like",
                    4_000,
                ),
                _ => unreachable!(),
            };
            let request_signature = sign_social_request(&requester_key, &event, 3_000);
            assert!(
                service
                    .process_submit_wall_social_event_at(&event, 3_000, &request_signature, 3_000,)
                    .is_err(),
                "{case} was accepted"
            );
            assert_eq!(social_event_count(&service), 0, "{case} inserted a row");
        }
    }

    #[test]
    fn valid_social_event_is_idempotent_but_same_id_conflict_is_rejected() {
        let (service, _author_key, requester_key) = service_with_wall_posts();
        let event = comment_event(
            &requester_key,
            requester(),
            "public-post",
            requester(),
            "hello",
            3_000,
        );
        let request_signature = sign_social_request(&requester_key, &event, 3_000);
        for _ in 0..2 {
            service
                .process_submit_wall_social_event_at(&event, 3_000, &request_signature, 3_000)
                .unwrap();
        }
        assert_eq!(social_event_count(&service), 1);

        let mut conflict = comment_event(
            &requester_key,
            requester(),
            "public-post",
            requester(),
            "changed",
            3_001,
        );
        conflict.event_id = event.event_id.clone();
        let payload = SignableWallCommentCreate {
            event_id: conflict.event_id.clone(),
            post_id: conflict.post_id.clone(),
            comment_id: conflict.comment_id.clone().unwrap(),
            actor_peer_id: conflict.actor_peer_id.clone(),
            author_name: conflict.author_name.clone().unwrap(),
            content: conflict.content.clone().unwrap(),
            timestamp: conflict.timestamp,
        };
        conflict.payload_cbor = payload.signable_bytes().unwrap();
        conflict.signature = sign(&requester_key, &payload);
        let conflict_request = sign_social_request(&requester_key, &conflict, 3_001);
        assert_eq!(
            service
                .process_submit_wall_social_event_at(&conflict, 3_001, &conflict_request, 3_001,),
            Err("RELAY_SOCIAL_EVENT_CONFLICT".into())
        );
        assert_eq!(social_event_count(&service), 1);
    }

    #[test]
    fn verified_and_explicitly_unverified_actors_follow_visibility_authorization() {
        let (service, author_key, requester_key) = service_with_wall_posts();
        let grant = wall_read_grant(&author_key, author(), requester());
        service.process_wall_read_grant(&grant).unwrap();
        let unverified = comment_event(
            &requester_key,
            requester(),
            "contacts-post",
            requester(),
            "authorized by grant",
            3_000,
        );
        let signature = sign_social_request(&requester_key, &unverified, 3_000);
        service
            .process_submit_wall_social_event_at(&unverified, 3_000, &signature, 3_000)
            .unwrap();

        service.db.with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO relay_name_claims(local_name,relay,peer_id,sequence,claim_cbor,not_before,not_after,relay_key_id,status,created_at,retired_at)
                     VALUES('alice','relay.test',?,1,X'',1,9999,'k1','active',1,NULL)",
                    [author()],
                )
                .unwrap();
            connection
                .execute(
                    "UPDATE known_peers SET identity_state='verified' WHERE peer_id=?",
                    [author()],
                )
                .unwrap();
        });
        let verified = comment_event(
            &author_key,
            author(),
            "contacts-post",
            "@alice@relay.test",
            "owner comment",
            3_001,
        );
        let signature = sign_social_request(&author_key, &verified, 3_001);
        service
            .process_submit_wall_social_event_at(&verified, 3_001, &signature, 3_001)
            .unwrap();
        assert_eq!(social_event_count(&service), 2);
    }

    #[test]
    fn social_event_integrity_capacity_and_retention_survive_restart() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("relay.db");
        let path = path.to_string_lossy().into_owned();
        let actor_key = signing_key(2);
        let event;
        let request_signature;
        {
            let database = RelayDatabase::open(&path).unwrap();
            database
                .configure_retention(
                    crate::db::RetentionLimits {
                        record_retention_secs: 10,
                        max_known_peers: 10,
                        max_posts: 10,
                        max_grants: 10,
                        max_introductions: 10,
                        max_social_events: 1,
                    },
                    3_000,
                )
                .unwrap();
            let service = BoardService::new(database, "test".into());
            register(&service, requester(), &actor_key);
            service
                .db
                .write_wall_post_with_media(
                    "restart-post",
                    requester(),
                    "text",
                    Some("post"),
                    "public",
                    1,
                    3_000,
                    &[1; 64],
                    &[],
                    3_000,
                )
                .unwrap();
            event = reaction_event(
                &actor_key,
                requester(),
                "restart-post",
                "reaction_add",
                "like",
                3_000,
            );
            request_signature = sign_social_request(&actor_key, &event, 3_000);
            service
                .process_submit_wall_social_event_at(&event, 3_000, &request_signature, 3_000)
                .unwrap();
        }

        let reopened_db = RelayDatabase::open(&path).unwrap();
        let reopened = BoardService::new(reopened_db.clone(), "test".into());
        reopened
            .process_submit_wall_social_event_at(&event, 3_000, &request_signature, 3_001)
            .unwrap();
        let second = reaction_event(
            &actor_key,
            requester(),
            "restart-post",
            "reaction_add",
            "heart",
            3_001,
        );
        let second_request = sign_social_request(&actor_key, &second, 3_001);
        assert!(reopened
            .process_submit_wall_social_event_at(&second, 3_001, &second_request, 3_001)
            .is_err());
        assert_eq!(social_event_count(&reopened), 1);

        reopened_db.enforce_retention(3_011).unwrap();
        assert_eq!(social_event_count(&reopened), 0);
    }

    #[test]
    fn get_wall_posts_returns_public_only_without_wall_read() {
        let (service, _author_key, requester_key) = service_with_wall_posts();
        let timestamp = 3_000;
        let signature = signed_get(&requester_key, requester(), author(), timestamp);

        let (posts, _has_more, _media) = service
            .process_get_wall_posts(requester(), author(), 0, 20, timestamp, &signature, None)
            .unwrap();

        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].post_id, "public-post");
    }

    #[test]
    fn unauthorized_wall_response_excludes_private_posts_media_and_social_events() {
        let (service, _author_key, requester_key) = service_with_wall_posts();
        for (post_id, hash, signature) in [
            ("public-post", "a".repeat(64), vec![10; 64]),
            ("contacts-post", "b".repeat(64), vec![11; 64]),
        ] {
            service.db.with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO wall_post_media(post_id, media_hash, media_type, mime_type, file_name, file_size, width, height, duration_seconds, sort_order, signature)
                         VALUES(?, ?, 'image', 'image/png', ?, 128, 10, 10, NULL, 0, ?)",
                        rusqlite::params![post_id, hash, format!("{post_id}.png"), signature],
                    )
                    .unwrap();
            });
            service
                .db
                .insert_wall_social_event(&WallSocialEventRow {
                    event_id: format!("event-{post_id}"),
                    event_type: "comment_create".into(),
                    post_id: post_id.into(),
                    actor_peer_id: requester().into(),
                    author_name: None,
                    comment_id: Some(format!("comment-{post_id}")),
                    content: Some(format!("comment on {post_id}")),
                    reaction_type: None,
                    timestamp: 2_500,
                    payload_cbor: vec![1, 2, 3],
                    signature,
                })
                .unwrap();
        }

        let timestamp = 3_000;
        let post_signature = signed_get(&requester_key, requester(), author(), timestamp);
        let (posts, _, media) = service
            .process_get_wall_posts(
                requester(),
                author(),
                0,
                20,
                timestamp,
                &post_signature,
                None,
            )
            .unwrap();
        assert_eq!(
            posts
                .iter()
                .map(|post| post.post_id.as_str())
                .collect::<Vec<_>>(),
            vec!["public-post"]
        );
        assert_eq!(media.len(), 1);
        assert_eq!(media[0].0, "public-post");
        assert!(media.iter().all(|(post_id, _)| post_id != "contacts-post"));

        let post_ids = vec!["public-post".into(), "contacts-post".into()];
        let social_signature =
            signed_get_social(&requester_key, requester(), author(), &post_ids, timestamp);
        let (events, _, _) = service
            .process_get_wall_social_events(
                requester(),
                author(),
                &post_ids,
                0,
                20,
                timestamp,
                &social_signature,
            )
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].post_id, "public-post");
        assert!(events.iter().all(|event| event.post_id != "contacts-post"));
    }

    #[test]
    fn get_wall_posts_returns_contacts_posts_with_valid_wall_read_grant() {
        let (service, author_key, requester_key) = service_with_wall_posts();
        let timestamp = 3_000;
        let signature = signed_get(&requester_key, requester(), author(), timestamp);
        let grant = wall_read_grant(&author_key, author(), requester());

        let (posts, _has_more, _media) = service
            .process_get_wall_posts(
                requester(),
                author(),
                0,
                20,
                timestamp,
                &signature,
                Some(&grant),
            )
            .unwrap();

        let post_ids: Vec<_> = posts.iter().map(|post| post.post_id.as_str()).collect();
        assert_eq!(post_ids, vec!["public-post", "contacts-post"]);
    }

    #[test]
    fn get_wall_posts_paginates_from_stored_lamport_without_duplicates() {
        let (service, _author_key, requester_key) = service_with_wall_posts();
        service
            .db
            .write_wall_post_with_media(
                "newer-public-post",
                author(),
                "text",
                Some("Newer"),
                "public",
                3,
                3_000,
                &[9; 64],
                &[],
                3_000,
            )
            .unwrap();

        let timestamp = 4_000;
        let first_signature =
            signed_get_with(&requester_key, requester(), author(), 0, 1, timestamp);
        let (first_page, has_more, _media) = service
            .process_get_wall_posts(
                requester(),
                author(),
                0,
                1,
                timestamp,
                &first_signature,
                None,
            )
            .unwrap();
        assert_eq!(first_page[0].post_id, "public-post");
        assert!(has_more);

        let next_cursor = first_page[0].lamport_clock;
        let second_signature = signed_get_with(
            &requester_key,
            requester(),
            author(),
            next_cursor,
            20,
            timestamp,
        );
        let (second_page, has_more, _media) = service
            .process_get_wall_posts(
                requester(),
                author(),
                next_cursor,
                20,
                timestamp,
                &second_signature,
                None,
            )
            .unwrap();

        let post_ids: Vec<_> = second_page
            .iter()
            .map(|post| post.post_id.as_str())
            .collect();
        assert_eq!(post_ids, vec!["newer-public-post"]);
        assert!(!has_more);
    }

    #[test]
    fn get_wall_posts_rejects_malformed_wall_read_grant() {
        let (service, author_key, requester_key) = service_with_wall_posts();
        let timestamp = 3_000;
        let signature = signed_get(&requester_key, requester(), author(), timestamp);
        let mut grant = wall_read_grant(&author_key, author(), requester());
        grant.capability = "chat".to_string();

        let result = service.process_get_wall_posts(
            requester(),
            author(),
            0,
            20,
            timestamp,
            &signature,
            Some(&grant),
        );

        assert!(matches!(result, Err(error) if error == RELAY_READ_GRANT_INVALID));
    }

    #[test]
    fn contact_card_wall_grant_is_enforced_then_revoked_across_profiles() {
        let (service, author_key, requester_key) = service_with_wall_posts();
        let mut grant = wall_read_grant(&author_key, author(), requester());
        grant.capability = "wall:read".to_string();
        grant.signature = sign(
            &author_key,
            &SignablePermissionGrant {
                grant_id: grant.grant_id.clone(),
                issuer_peer_id: grant.issuer_peer_id.clone(),
                subject_peer_id: grant.subject_peer_id.clone(),
                capability: grant.capability.clone(),
                scope: None,
                lamport_clock: grant.lamport_clock,
                issued_at: grant.issued_at,
                expires_at: None,
            },
        );
        service.process_wall_read_grant(&grant).unwrap();
        let timestamp = 3_000;
        let request_signature = signed_get(&requester_key, requester(), author(), timestamp);
        let (allowed, _, _) = service
            .process_get_wall_posts(
                requester(),
                author(),
                0,
                20,
                timestamp,
                &request_signature,
                None,
            )
            .unwrap();
        assert!(allowed.iter().any(|post| post.post_id == "contacts-post"));

        let revision = grant.lamport_clock + 1;
        let revoked_at = 2_500;
        let revoke_signature = sign(
            &author_key,
            &SignablePermissionRevoke {
                grant_id: grant.grant_id.clone(),
                issuer_peer_id: author().into(),
                lamport_clock: revision,
                revoked_at,
            },
        );
        service
            .process_wall_read_revoke(
                &grant.grant_id,
                author(),
                revision,
                revoked_at,
                &revoke_signature,
            )
            .unwrap();
        let (denied, _, _) = service
            .process_get_wall_posts(
                requester(),
                author(),
                0,
                20,
                timestamp,
                &request_signature,
                None,
            )
            .unwrap();
        assert_eq!(
            denied
                .iter()
                .map(|post| post.post_id.as_str())
                .collect::<Vec<_>>(),
            vec!["public-post"]
        );

        // A delayed copy of the older grant cannot resurrect access.
        service.process_wall_read_grant(&grant).unwrap();
        let (still_denied, _, _) = service
            .process_get_wall_posts(
                requester(),
                author(),
                0,
                20,
                timestamp,
                &request_signature,
                None,
            )
            .unwrap();
        assert_eq!(still_denied.len(), 1);
    }

    #[test]
    fn expired_contact_card_wall_grant_never_serves_private_rows() {
        let (service, author_key, requester_key) = service_with_wall_posts();
        let mut grant = wall_read_grant(&author_key, author(), requester());
        grant.capability = "wall:read".into();
        grant.expires_at = Some(2_000);
        grant.signature = sign(
            &author_key,
            &SignablePermissionGrant {
                grant_id: grant.grant_id.clone(),
                issuer_peer_id: grant.issuer_peer_id.clone(),
                subject_peer_id: grant.subject_peer_id.clone(),
                capability: grant.capability.clone(),
                scope: None,
                lamport_clock: grant.lamport_clock,
                issued_at: grant.issued_at,
                expires_at: grant.expires_at,
            },
        );
        // Propagation after expiry is rejected rather than persisted as authority.
        assert!(service
            .process_wall_read_grant(&grant)
            .unwrap_err()
            .contains("expired"));
        let timestamp = 3_000;
        let signature = signed_get(&requester_key, requester(), author(), timestamp);
        let (posts, _, _) = service
            .process_get_wall_posts(requester(), author(), 0, 20, timestamp, &signature, None)
            .unwrap();
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].post_id, "public-post");
    }

    #[test]
    fn client_timestamp_cannot_extend_an_expired_wall_read_grant() {
        let (service, author_key, requester_key) = service_with_wall_posts();
        let request_timestamp = 1_500;
        let server_now = 3_000;
        let signature = signed_get(&requester_key, requester(), author(), request_timestamp);
        let mut grant = wall_read_grant(&author_key, author(), requester());
        grant.expires_at = Some(2_000);
        grant.signature = sign(
            &author_key,
            &SignablePermissionGrant {
                grant_id: grant.grant_id.clone(),
                issuer_peer_id: grant.issuer_peer_id.clone(),
                subject_peer_id: grant.subject_peer_id.clone(),
                capability: grant.capability.clone(),
                scope: None,
                lamport_clock: grant.lamport_clock,
                issued_at: grant.issued_at,
                expires_at: grant.expires_at,
            },
        );

        let result = service.process_get_wall_posts_at(
            requester(),
            author(),
            0,
            20,
            request_timestamp,
            &signature,
            Some(&grant),
            server_now,
        );

        assert!(matches!(result, Err(error) if error == RELAY_READ_GRANT_INVALID));
    }

    #[test]
    fn scoped_wall_read_grants_are_rejected_until_scope_semantics_exist() {
        let (service, author_key, requester_key) = service_with_wall_posts();
        let timestamp = 3_000;
        let signature = signed_get(&requester_key, requester(), author(), timestamp);
        let mut grant = wall_read_grant(&author_key, author(), requester());
        grant.scope = Some(serde_json::json!({"post_ids": ["contacts-post"]}));
        grant.signature = sign(
            &author_key,
            &SignablePermissionGrant {
                grant_id: grant.grant_id.clone(),
                issuer_peer_id: grant.issuer_peer_id.clone(),
                subject_peer_id: grant.subject_peer_id.clone(),
                capability: grant.capability.clone(),
                scope: grant.scope.clone(),
                lamport_clock: grant.lamport_clock,
                issued_at: grant.issued_at,
                expires_at: grant.expires_at,
            },
        );

        let result = service.process_get_wall_posts_at(
            requester(),
            author(),
            0,
            20,
            timestamp,
            &signature,
            Some(&grant),
            timestamp,
        );

        assert!(matches!(result, Err(error) if error == RELAY_READ_SCOPE_UNSUPPORTED));
    }

    #[test]
    fn previously_stored_scoped_grant_cannot_authorize_an_unscoped_read() {
        let (service, author_key, requester_key) = service_with_wall_posts();
        let mut grant = wall_read_grant(&author_key, author(), requester());
        grant.scope = Some(serde_json::json!({"board_id": "different-scope"}));
        service
            .db
            .upsert_wall_read_grant(
                &grant.grant_id,
                &grant.issuer_peer_id,
                &grant.subject_peer_id,
                &grant.capability,
                Some(r#"{"board_id":"different-scope"}"#),
                grant.lamport_clock,
                grant.issued_at,
                grant.expires_at,
                &grant.signature,
            )
            .unwrap();
        let timestamp = 3_000;
        let signature = signed_get(&requester_key, requester(), author(), timestamp);

        let (posts, _, _) = service
            .process_get_wall_posts_at(
                requester(),
                author(),
                0,
                20,
                timestamp,
                &signature,
                None,
                timestamp,
            )
            .unwrap();

        assert_eq!(
            posts
                .iter()
                .map(|post| post.post_id.as_str())
                .collect::<Vec<_>>(),
            vec!["public-post"]
        );
    }

    #[test]
    fn wall_read_database_failure_denies_with_stable_error() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("relay.db");
        let path_string = path.to_string_lossy().into_owned();
        let service = BoardService::new(
            RelayDatabase::open(&path_string).unwrap(),
            "test".to_string(),
        );
        let requester_key = signing_key(2);
        register(&service, requester(), &requester_key);
        rusqlite::Connection::open(&path)
            .unwrap()
            .execute("DROP TABLE wall_read_grants", [])
            .unwrap();
        let timestamp = 3_000;
        let signature = signed_get(&requester_key, requester(), author(), timestamp);

        let result = service.process_get_wall_posts_at(
            requester(),
            author(),
            0,
            20,
            timestamp,
            &signature,
            None,
            timestamp,
        );

        assert!(matches!(result, Err(error) if error == RELAY_READ_DATABASE));
    }

    #[test]
    fn board_read_signature_binds_cursor_and_limit() {
        let service =
            BoardService::new(RelayDatabase::open(":memory:").unwrap(), "test".to_string());
        let requester_key = signing_key(2);
        register(&service, requester(), &requester_key);
        let timestamp = 3_000;
        let signature = sign(
            &requester_key,
            &SignableBoardPostsRequest {
                requester_peer_id: requester().into(),
                board_id: "general".into(),
                after_timestamp: None,
                limit: 20,
                timestamp,
            },
        );

        let cursor_result = service.process_get_board_posts(
            requester(),
            "general",
            Some(10),
            20,
            timestamp,
            &signature,
        );
        let limit_result = service.process_get_board_posts(
            requester(),
            "general",
            None,
            100,
            timestamp,
            &signature,
        );

        assert!(matches!(cursor_result, Err(error) if error == RELAY_READ_SIGNATURE_INVALID));
        assert!(matches!(limit_result, Err(error) if error == RELAY_READ_SIGNATURE_INVALID));
    }

    #[test]
    fn board_clock_protocol_boundary_rejects_unrepresentable_values() {
        let service =
            BoardService::new(RelayDatabase::open(":memory:").unwrap(), "test".to_string());
        for clock in [0, i64::MAX as u64 + 1, u64::MAX] {
            let result = service.process_submit_post(
                "post",
                "general",
                "author",
                "text",
                None,
                clock,
                100,
                &[],
            );
            assert!(matches!(result, Err(error) if error == RELAY_INTEGER_RANGE));
        }
        let representable = service.process_submit_post(
            "post",
            "general",
            "author",
            "text",
            None,
            i64::MAX as u64,
            100,
            &[],
        );
        assert!(!matches!(representable, Err(error) if error == RELAY_INTEGER_RANGE));
    }

    #[test]
    fn older_board_protocol_binds_and_advances_compound_cursor() {
        let service =
            BoardService::new(RelayDatabase::open(":memory:").unwrap(), "test".to_string());
        let requester_key = signing_key(2);
        register(&service, requester(), &requester_key);
        service.db.with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO boards(board_id,name,created_at,is_default) VALUES('paging','Paging',100,0)",
                    [],
                )
                .unwrap();
        });
        for (post_id, clock) in [("a", 1), ("b", 2), ("c", 3)] {
            service
                .db
                .insert_post(
                    post_id,
                    "paging",
                    requester(),
                    "text",
                    None,
                    clock,
                    100,
                    &[1],
                )
                .unwrap();
        }

        let timestamp = 3_000;
        let first_signature = sign(
            &requester_key,
            &SignableOlderBoardPostsRequest {
                requester_peer_id: requester().into(),
                board_id: "paging".into(),
                before: None,
                limit: 2,
                timestamp,
            },
        );
        let (first, has_more) = service
            .process_get_older_board_posts(
                requester(),
                "paging",
                None,
                2,
                timestamp,
                &first_signature,
            )
            .unwrap();
        assert!(has_more);
        assert_eq!(
            first
                .iter()
                .map(|post| post.post_id.as_str())
                .collect::<Vec<_>>(),
            ["c", "b"]
        );

        let cursor = BoardPostCursor {
            created_at: first.last().unwrap().created_at,
            post_id: first.last().unwrap().post_id.clone(),
        };
        let second_signature = sign(
            &requester_key,
            &SignableOlderBoardPostsRequest {
                requester_peer_id: requester().into(),
                board_id: "paging".into(),
                before: Some(cursor.clone()),
                limit: 2,
                timestamp,
            },
        );
        let (second, has_more) = service
            .process_get_older_board_posts(
                requester(),
                "paging",
                Some(&cursor),
                2,
                timestamp,
                &second_signature,
            )
            .unwrap();
        assert!(!has_more);
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].post_id, "a");
    }

    #[test]
    fn delete_wall_post_persists_signed_tombstone_and_blocks_stale_snapshot() {
        let (service, author_key, requester_key) = service_with_wall_posts();
        let delete_signature = signed_delete(&author_key, "public-post", author(), 5, 5_000);
        service
            .process_delete_wall_post(author(), "public-post", 5, 5_000, &delete_signature)
            .unwrap();

        let stale_result = service.db.write_wall_post_with_media(
            "public-post",
            author(),
            "text",
            Some("Stale relay snapshot"),
            "public",
            1,
            1_000,
            &[3; 64],
            &[],
            5_001,
        );
        assert_eq!(stale_result, Err(crate::db::WallPostWriteError::Tombstoned));

        let timestamp = 6_000;
        let signature = signed_get(&requester_key, requester(), author(), timestamp);
        let (posts, _has_more, media) = service
            .process_get_wall_posts(requester(), author(), 0, 20, timestamp, &signature, None)
            .unwrap();

        let tombstone = posts
            .iter()
            .find(|post| post.post_id == "public-post")
            .expect("delete tombstone should be returned");
        assert_eq!(tombstone.deleted_at, Some(5_000));
        assert_eq!(tombstone.lamport_clock, 5);
        assert_eq!(tombstone.signature, delete_signature);
        assert!(media.iter().all(|(post_id, _)| post_id != "public-post"));
    }

    #[test]
    fn get_wall_posts_hides_contacts_posts_after_grant_revoke() {
        let (service, author_key, requester_key) = service_with_wall_posts();
        let timestamp = 3_000;
        let grant = wall_read_grant(&author_key, author(), requester());
        service.process_wall_read_grant(&grant).unwrap();

        let revoke = SignablePermissionRevoke {
            grant_id: grant.grant_id.clone(),
            issuer_peer_id: author().to_string(),
            lamport_clock: 4,
            revoked_at: 3_500,
        };
        let revoke_signature = sign(&author_key, &revoke);
        service
            .process_wall_read_revoke(&grant.grant_id, author(), 4, 3_500, &revoke_signature)
            .unwrap();

        let signature = signed_get(&requester_key, requester(), author(), timestamp);
        let (posts, _has_more, _media) = service
            .process_get_wall_posts(requester(), author(), 0, 20, timestamp, &signature, None)
            .unwrap();

        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].post_id, "public-post");
    }
}
