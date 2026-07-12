//! Server-side board logic for the relay server

use crate::db::{RelayDatabase, WallSocialEventRow};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tracing::{info, warn};

const MAX_POST_MEDIA_BYTES: i64 = 10 * 1024 * 1024;

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
    pub timestamp: i64,
}

impl Signable for SignableBoardPostsRequest {}

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
struct SignableGetWallSocialEvents {
    pub requester_peer_id: String,
    pub author_peer_id: String,
    pub post_ids: Vec<String>,
    pub after_timestamp: i64,
    pub limit: u32,
    pub timestamp: i64,
}
impl Signable for SignableGetWallSocialEvents {}

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
    let stored_public_key = database
        .get_peer_public_key(peer_id)
        .map_err(|db_error| format!("Database error looking up peer key: {}", db_error))?
        .ok_or_else(|| format!("No public key found for peer: {}", peer_id))?;

    verify_signature(&stored_public_key, signable, signature_bytes)
}

fn verify_registered_peer_raw_signature(
    database: &RelayDatabase,
    peer_id: &str,
    payload: &[u8],
    signature_bytes: &[u8],
) -> Result<(), String> {
    let stored_public_key = database
        .get_peer_public_key(peer_id)
        .map_err(|db_error| format!("Database error looking up peer key: {}", db_error))?
        .ok_or_else(|| format!("No public key found for peer: {}", peer_id))?;
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
    pub fn process_register_peer(
        &self,
        peer_id: &str,
        public_key: &[u8],
        display_name: &str,
        timestamp: i64,
        signature: &[u8],
    ) -> Result<(), String> {
        if self.db.is_peer_banned(peer_id).unwrap_or(false) {
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
                format!("Signature verification failed: {}", verification_error)
            },
        )?;

        self.db
            .register_peer(peer_id, public_key, display_name)
            .map_err(|db_error| format!("Failed to register peer: {}", db_error))?;

        info!("Registered peer: {} ({})", display_name, peer_id);
        Ok(())
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
        // Check peer is known
        if !self.db.is_peer_known(author_peer_id).unwrap_or(false) {
            return Err("Peer not registered. Call RegisterPeer first.".to_string());
        }

        // Check not banned
        if self.db.is_peer_banned(author_peer_id).unwrap_or(false) {
            return Err("Peer is banned".to_string());
        }

        // Check board exists
        if !self.db.board_exists(board_id).unwrap_or(false) {
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
                format!("Signature verification failed: {}", verification_error)
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
                format!("Signature verification failed: {}", verification_error)
            })?;

        self.db
            .list_boards()
            .map_err(|db_error| format!("Failed to list boards: {}", db_error))
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
            timestamp,
        };

        verify_registered_peer_signature(&self.db, requester_peer_id, &signable_request, signature)
            .map_err(|verification_error| {
                warn!(
                    "GetBoardPosts signature verification failed for {}: {}",
                    requester_peer_id, verification_error
                );
                format!("Signature verification failed: {}", verification_error)
            })?;

        let clamped_limit = limit.min(100);
        let posts = self
            .db
            .get_board_posts(board_id, after_timestamp, clamped_limit + 1)
            .map_err(|db_error| format!("Failed to get board posts: {}", db_error))?;

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
                format!("Signature verification failed: {}", verification_error)
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
        if !self.db.is_peer_known(author_peer_id).unwrap_or(false) {
            return Err("Peer not registered. Call RegisterPeer first.".to_string());
        }

        // Check not banned
        if self.db.is_peer_banned(author_peer_id).unwrap_or(false) {
            return Err("Peer is banned".to_string());
        }

        // Validate visibility
        if visibility != "public" && visibility != "contacts" {
            return Err(format!(
                "Invalid visibility '{}': must be 'public' or 'contacts'",
                visibility
            ));
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
            format!("Signature verification failed: {}", verification_error)
        })?;

        let signable_post = SignablePost {
            post_id: post_id.to_string(),
            author_peer_id: author_peer_id.to_string(),
            content_type: content_type.to_string(),
            content_text: content_text.map(|t| t.to_string()),
            media_hashes: media_hashes.to_vec(),
            visibility: visibility.to_string(),
            lamport_clock: lamport_clock as u64,
            created_at,
        };
        verify_registered_peer_signature(&self.db, author_peer_id, &signable_post, signature)
            .map_err(|verification_error| {
                warn!(
                    "Inner wall post signature verification failed for post {} by {}: {}",
                    post_id, author_peer_id, verification_error
                );
                format!("Post signature verification failed: {}", verification_error)
            })?;

        let sorted_hashes = sorted_media_hashes(media_items);
        if sorted_hashes != media_hashes {
            return Err(
                "Media metadata hashes do not match the hashes signed by the post".to_string(),
            );
        }

        let mut seen_orders = HashSet::new();
        let mut seen_hashes = HashSet::new();
        for item in media_items {
            validate_media_item(item)
                .map_err(|error| format!("Invalid media metadata: {}", error))?;
            if !seen_orders.insert(item.sort_order) {
                return Err(format!("Duplicate media sort_order {}", item.sort_order));
            }
            if !seen_hashes.insert(item.media_hash.clone()) {
                return Err(format!("Duplicate media hash {}", item.media_hash));
            }
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
                format!(
                    "Media signature verification failed: {}",
                    verification_error
                )
            })?;
        }

        // Store the wall post
        self.db
            .insert_wall_post(
                post_id,
                author_peer_id,
                content_type,
                content_text,
                visibility,
                lamport_clock,
                created_at,
                signature,
            )
            .map_err(|db_error| format!("Failed to store wall post: {}", db_error))?;

        // Store media metadata alongside the wall post
        for item in media_items {
            if let Err(e) = self.db.insert_wall_post_media(
                post_id,
                &item.media_hash,
                &item.media_type,
                &item.mime_type,
                &item.file_name,
                item.file_size,
                item.width,
                item.height,
                item.duration_seconds,
                item.sort_order,
                &item.signature,
            ) {
                warn!("Failed to store media metadata for post {}: {}", post_id, e);
            }
        }

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
        if grant.issuer_peer_id != author_peer_id || grant.subject_peer_id != requester_peer_id {
            return Err("WallRead grant does not match requested author/requester".to_string());
        }
        if grant.capability != "wall_read" && grant.capability != "wall:read" {
            return Err(format!(
                "Invalid grant capability '{}': expected wall:read",
                grant.capability
            ));
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
    #[allow(clippy::type_complexity, clippy::too_many_arguments)] // Return type and arguments mirror the relay wire request/response.
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
        // Verify the requester's signature
        let signable_request = SignableGetWallPosts {
            requester_peer_id: requester_peer_id.to_string(),
            author_peer_id: author_peer_id.to_string(),
            since_lamport_clock,
            limit,
            timestamp,
        };

        verify_registered_peer_signature(&self.db, requester_peer_id, &signable_request, signature)
            .map_err(|verification_error| {
                warn!(
                    "GetWallPosts signature verification failed for {}: {}",
                    requester_peer_id, verification_error
                );
                format!("Signature verification failed: {}", verification_error)
            })?;

        if let Some(grant) = grant_proof {
            self.validate_wall_read_grant(grant, author_peer_id, requester_peer_id, timestamp)
                .map_err(|grant_error| format!("Invalid WallRead grant proof: {}", grant_error))?;
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
                    format!("Failed to store WallRead grant proof: {}", db_error)
                })?;
        }

        let can_read_contacts = requester_peer_id == author_peer_id
            || self
                .db
                .has_active_wall_read_grant(author_peer_id, requester_peer_id, timestamp)
                .map_err(|db_error| format!("Failed to check WallRead grant: {}", db_error))?;

        let clamped_limit = limit.min(100);
        let visible_posts = self
            .db
            .get_wall_posts(
                author_peer_id,
                since_lamport_clock,
                clamped_limit + 1,
                can_read_contacts,
            )
            .map_err(|db_error| format!("Failed to get wall posts: {}", db_error))?;

        let has_more = visible_posts.len() > clamped_limit as usize;
        let posts = if has_more {
            visible_posts[..clamped_limit as usize].to_vec()
        } else {
            visible_posts
        };

        // Fetch media metadata for each live post; tombstones carry the delete signature only.
        let mut media_map = Vec::new();
        for post in &posts {
            if post.deleted_at.is_some() {
                continue;
            }
            match self.db.get_wall_post_media(&post.post_id) {
                Ok(media_items) if !media_items.is_empty() => {
                    media_map.push((post.post_id.clone(), media_items));
                }
                _ => {}
            }
        }

        Ok((posts, has_more, media_map))
    }

    pub fn process_submit_wall_social_event(
        &self,
        event: &crate::WallSocialEventItemProto,
        request_timestamp: i64,
        request_signature: &[u8],
    ) -> Result<(), String> {
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
        .map_err(|e| format!("Signature verification failed: {}", e))?;
        verify_registered_peer_raw_signature(
            &self.db,
            &event.actor_peer_id,
            &event.payload_cbor,
            &event.signature,
        )
        .map_err(|e| format!("Event signature verification failed: {}", e))?;
        self.db
            .insert_wall_social_event(&WallSocialEventRow {
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
            })
            .map_err(|e| format!("Failed to store wall social event: {}", e))?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)] // Mirrors the relay wire request shape.
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
        let signable = SignableGetWallSocialEvents {
            requester_peer_id: requester_peer_id.to_string(),
            author_peer_id: author_peer_id.to_string(),
            post_ids: post_ids.to_vec(),
            after_timestamp,
            limit,
            timestamp,
        };
        verify_registered_peer_signature(&self.db, requester_peer_id, &signable, signature)
            .map_err(|e| format!("Signature verification failed: {}", e))?;
        let can_read_contacts = requester_peer_id == author_peer_id
            || self
                .db
                .has_active_wall_read_grant(author_peer_id, requester_peer_id, timestamp)
                .map_err(|e| format!("Failed to check WallRead grant: {}", e))?;
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
            .map_err(|e| format!("Failed to get wall social events: {}", e))?;
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
                format!("Signature verification failed: {}", verification_error)
            })?;

        let deleted = self
            .db
            .tombstone_wall_post(
                post_id,
                author_peer_id,
                lamport_clock,
                deleted_at,
                signature,
            )
            .map_err(|db_error| format!("Failed to tombstone wall post: {}", db_error))?;

        if !deleted {
            warn!(
                "Wall post {} not found or not owned by {}",
                post_id, author_peer_id
            );
            return Err("Wall post not found or not owned by you".to_string());
        }

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

    fn register(service: &BoardService, peer_id: &str, key: &SigningKey) {
        service
            .db
            .register_peer(peer_id, &key.verifying_key().to_bytes(), peer_id)
            .unwrap();
    }

    fn sign<T: Signable>(key: &SigningKey, value: &T) -> Vec<u8> {
        key.sign(&value.signable_bytes().unwrap())
            .to_bytes()
            .to_vec()
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

    fn service_with_wall_posts() -> (BoardService, SigningKey, SigningKey) {
        let service =
            BoardService::new(RelayDatabase::open(":memory:").unwrap(), "test".to_string());
        let author_key = signing_key(1);
        let requester_key = signing_key(2);
        register(&service, "author", &author_key);
        register(&service, "requester", &requester_key);
        service
            .db
            .insert_wall_post(
                "public-post",
                "author",
                "text",
                Some("Public"),
                "public",
                1,
                1_000,
                &[7; 64],
            )
            .unwrap();
        service
            .db
            .insert_wall_post(
                "contacts-post",
                "author",
                "text",
                Some("Contacts"),
                "contacts",
                2,
                2_000,
                &[8; 64],
            )
            .unwrap();
        (service, author_key, requester_key)
    }

    #[test]
    fn get_wall_posts_returns_public_only_without_wall_read() {
        let (service, _author_key, requester_key) = service_with_wall_posts();
        let timestamp = 3_000;
        let signature = signed_get(&requester_key, "requester", "author", timestamp);

        let (posts, _has_more, _media) = service
            .process_get_wall_posts("requester", "author", 0, 20, timestamp, &signature, None)
            .unwrap();

        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].post_id, "public-post");
    }

    #[test]
    fn get_wall_posts_returns_contacts_posts_with_valid_wall_read_grant() {
        let (service, author_key, requester_key) = service_with_wall_posts();
        let timestamp = 3_000;
        let signature = signed_get(&requester_key, "requester", "author", timestamp);
        let grant = wall_read_grant(&author_key, "author", "requester");

        let (posts, _has_more, _media) = service
            .process_get_wall_posts(
                "requester",
                "author",
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
            .insert_wall_post(
                "newer-public-post",
                "author",
                "text",
                Some("Newer"),
                "public",
                3,
                3_000,
                &[9; 64],
            )
            .unwrap();

        let timestamp = 4_000;
        let first_signature =
            signed_get_with(&requester_key, "requester", "author", 0, 1, timestamp);
        let (first_page, has_more, _media) = service
            .process_get_wall_posts(
                "requester",
                "author",
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
            "requester",
            "author",
            next_cursor,
            20,
            timestamp,
        );
        let (second_page, has_more, _media) = service
            .process_get_wall_posts(
                "requester",
                "author",
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
        let signature = signed_get(&requester_key, "requester", "author", timestamp);
        let mut grant = wall_read_grant(&author_key, "author", "requester");
        grant.capability = "chat".to_string();

        let result = service.process_get_wall_posts(
            "requester",
            "author",
            0,
            20,
            timestamp,
            &signature,
            Some(&grant),
        );

        assert!(matches!(result, Err(message) if message.contains("Invalid WallRead grant proof")));
    }

    #[test]
    fn contact_card_wall_grant_is_enforced_then_revoked_across_profiles() {
        let (service, author_key, requester_key) = service_with_wall_posts();
        let mut grant = wall_read_grant(&author_key, "author", "requester");
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
        let request_signature = signed_get(&requester_key, "requester", "author", timestamp);
        let (allowed, _, _) = service
            .process_get_wall_posts(
                "requester",
                "author",
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
                issuer_peer_id: "author".into(),
                lamport_clock: revision,
                revoked_at,
            },
        );
        service
            .process_wall_read_revoke(
                &grant.grant_id,
                "author",
                revision,
                revoked_at,
                &revoke_signature,
            )
            .unwrap();
        let (denied, _, _) = service
            .process_get_wall_posts(
                "requester",
                "author",
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
                "requester",
                "author",
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
        let mut grant = wall_read_grant(&author_key, "author", "requester");
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
        let signature = signed_get(&requester_key, "requester", "author", timestamp);
        let (posts, _, _) = service
            .process_get_wall_posts("requester", "author", 0, 20, timestamp, &signature, None)
            .unwrap();
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].post_id, "public-post");
    }

    #[test]
    fn delete_wall_post_persists_signed_tombstone_and_blocks_stale_snapshot() {
        let (service, author_key, requester_key) = service_with_wall_posts();
        let delete_signature = signed_delete(&author_key, "public-post", "author", 5, 5_000);
        service
            .process_delete_wall_post("author", "public-post", 5, 5_000, &delete_signature)
            .unwrap();

        service
            .db
            .insert_wall_post(
                "public-post",
                "author",
                "text",
                Some("Stale relay snapshot"),
                "public",
                1,
                1_000,
                &[3; 64],
            )
            .unwrap();

        let timestamp = 6_000;
        let signature = signed_get(&requester_key, "requester", "author", timestamp);
        let (posts, _has_more, media) = service
            .process_get_wall_posts("requester", "author", 0, 20, timestamp, &signature, None)
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
        let grant = wall_read_grant(&author_key, "author", "requester");
        service.process_wall_read_grant(&grant).unwrap();

        let revoke = SignablePermissionRevoke {
            grant_id: grant.grant_id.clone(),
            issuer_peer_id: "author".to_string(),
            lamport_clock: 4,
            revoked_at: 3_500,
        };
        let revoke_signature = sign(&author_key, &revoke);
        service
            .process_wall_read_revoke(&grant.grant_id, "author", 4, 3_500, &revoke_signature)
            .unwrap();

        let signature = signed_get(&requester_key, "requester", "author", timestamp);
        let (posts, _has_more, _media) = service
            .process_get_wall_posts("requester", "author", 0, 20, timestamp, &signature, None)
            .unwrap();

        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].post_id, "public-post");
    }
}
