//! Server-side board logic for the relay server

use crate::db::RelayDatabase;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::Serialize;
use tracing::{info, warn};

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
    pub timestamp: i64,
}

impl Signable for SignableWallPostSubmit {}

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

/// Signable version of a wall post delete (excludes signature).
/// Must match `SignableWallPostDelete` on the client side.
#[derive(Debug, Clone, Serialize)]
struct SignableWallPostDelete {
    pub author_peer_id: String,
    pub post_id: String,
    pub timestamp: i64,
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

        verify_registered_peer_signature(
            &self.db,
            requester_peer_id,
            &signable_request,
            signature,
        )
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

        verify_registered_peer_signature(
            &self.db,
            requester_peer_id,
            &signable_request,
            signature,
        )
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
                item.sort_order,
            ) {
                warn!(
                    "Failed to store media metadata for post {}: {}",
                    post_id, e
                );
            }
        }

        info!(
            "Wall post {} stored for {} (visibility={}, lamport_clock={}, media={})",
            post_id, author_peer_id, visibility, lamport_clock, media_items.len()
        );
        Ok(())
    }

    /// Get wall posts for a specific author.
    ///
    /// Verifies the requester's signature before returning data.
    /// The requester must be a registered peer.
    pub fn process_get_wall_posts(
        &self,
        requester_peer_id: &str,
        author_peer_id: &str,
        since_lamport_clock: i64,
        limit: u32,
        timestamp: i64,
        signature: &[u8],
    ) -> Result<(Vec<crate::db::WallPostRow>, bool, Vec<(String, Vec<crate::db::WallPostMediaRow>)>), String> {
        // Verify the requester's signature
        let signable_request = SignableGetWallPosts {
            requester_peer_id: requester_peer_id.to_string(),
            author_peer_id: author_peer_id.to_string(),
            since_lamport_clock,
            limit,
            timestamp,
        };

        verify_registered_peer_signature(
            &self.db,
            requester_peer_id,
            &signable_request,
            signature,
        )
        .map_err(|verification_error| {
            warn!(
                "GetWallPosts signature verification failed for {}: {}",
                requester_peer_id, verification_error
            );
            format!("Signature verification failed: {}", verification_error)
        })?;

        let clamped_limit = limit.min(100);
        let posts = self
            .db
            .get_wall_posts(author_peer_id, since_lamport_clock, clamped_limit + 1)
            .map_err(|db_error| format!("Failed to get wall posts: {}", db_error))?;

        let has_more = posts.len() > clamped_limit as usize;
        let posts = if has_more {
            posts[..clamped_limit as usize].to_vec()
        } else {
            posts
        };

        // Fetch media metadata for each post
        let mut media_map = Vec::new();
        for post in &posts {
            match self.db.get_wall_post_media(&post.post_id) {
                Ok(media_items) if !media_items.is_empty() => {
                    media_map.push((post.post_id.clone(), media_items));
                }
                _ => {}
            }
        }

        Ok((posts, has_more, media_map))
    }

    /// Delete a wall post (author-only).
    ///
    /// Verifies the signature against the author's stored public key
    /// before deleting.
    pub fn process_delete_wall_post(
        &self,
        author_peer_id: &str,
        post_id: &str,
        timestamp: i64,
        signature: &[u8],
    ) -> Result<(), String> {
        // Verify signature against the author's stored public key
        let signable_delete = SignableWallPostDelete {
            author_peer_id: author_peer_id.to_string(),
            post_id: post_id.to_string(),
            timestamp,
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
            .delete_wall_post(post_id, author_peer_id)
            .map_err(|db_error| format!("Failed to delete wall post: {}", db_error))?;

        if !deleted {
            warn!(
                "Wall post {} not found or not owned by {}",
                post_id, author_peer_id
            );
            return Err("Wall post not found or not owned by you".to_string());
        }

        info!("Wall post {} deleted by {}", post_id, author_peer_id);
        Ok(())
    }
}
