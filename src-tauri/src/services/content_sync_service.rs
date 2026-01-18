//! Content sync service for synchronizing posts between peers

use std::collections::HashMap;
use std::sync::Arc;

use ed25519_dalek::VerifyingKey;

use crate::db::{Capability, Database, PostData, PostVisibility, PostsRepository};
use crate::error::{AppError, Result};
use crate::services::{
    verify, ContactsService, IdentityService, PermissionsService, PostSummary,
    SignableContentManifestRequest, SignableContentManifestResponse, SignablePost,
};

/// Service for syncing content between peers
pub struct ContentSyncService {
    db: Arc<Database>,
    identity_service: Arc<IdentityService>,
    contacts_service: Arc<ContactsService>,
    permissions_service: Arc<PermissionsService>,
}

/// A request for content manifest
#[derive(Debug, Clone)]
pub struct OutgoingManifestRequest {
    pub requester_peer_id: String,
    pub cursor: HashMap<String, u64>,
    pub limit: u32,
    pub timestamp: i64,
    pub signature: Vec<u8>,
}

/// A response with content manifest
#[derive(Debug, Clone)]
pub struct OutgoingManifestResponse {
    pub responder_peer_id: String,
    pub posts: Vec<PostSummary>,
    pub has_more: bool,
    pub next_cursor: HashMap<String, u64>,
    pub timestamp: i64,
    pub signature: Vec<u8>,
}

impl ContentSyncService {
    /// Create a new content sync service
    pub fn new(
        db: Arc<Database>,
        identity_service: Arc<IdentityService>,
        contacts_service: Arc<ContactsService>,
        permissions_service: Arc<PermissionsService>,
    ) -> Self {
        Self {
            db,
            identity_service,
            contacts_service,
            permissions_service,
        }
    }

    /// Create a manifest request to send to a peer
    pub fn create_manifest_request(
        &self,
        cursor: HashMap<String, u64>,
        limit: u32,
    ) -> Result<OutgoingManifestRequest> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::NotFound("No identity".to_string()))?;

        let timestamp = chrono::Utc::now().timestamp();

        let signable = SignableContentManifestRequest {
            requester_peer_id: identity.peer_id.clone(),
            cursor: cursor.clone(),
            limit,
            timestamp,
        };

        let signature = self.identity_service.sign(&signable)?;

        Ok(OutgoingManifestRequest {
            requester_peer_id: identity.peer_id,
            cursor,
            limit,
            timestamp,
            signature,
        })
    }

    /// Process an incoming manifest request and create a response
    pub fn process_manifest_request(
        &self,
        requester_peer_id: &str,
        cursor: &HashMap<String, u64>,
        limit: u32,
        timestamp: i64,
        signature: &[u8],
    ) -> Result<OutgoingManifestResponse> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::NotFound("No identity".to_string()))?;

        // Verify the requester's signature
        let requester_public_key = self
            .contacts_service
            .get_public_key(requester_peer_id)?
            .ok_or_else(|| AppError::NotFound("Requester not in contacts".to_string()))?;

        let signable = SignableContentManifestRequest {
            requester_peer_id: requester_peer_id.to_string(),
            cursor: cursor.clone(),
            limit,
            timestamp,
        };

        let verifying_key = VerifyingKey::from_bytes(
            requester_public_key
                .as_slice()
                .try_into()
                .map_err(|_| AppError::Crypto("Invalid public key length".to_string()))?,
        )
        .map_err(|e| AppError::Crypto(format!("Invalid public key: {}", e)))?;

        if !verify(&verifying_key, &signable, signature)? {
            return Err(AppError::Crypto(
                "Invalid manifest request signature".to_string(),
            ));
        }

        // Check if the requester has WallRead permission from us
        if !self
            .permissions_service
            .peer_has_capability(requester_peer_id, Capability::WallRead)?
        {
            return Err(AppError::PermissionDenied(
                "Requester doesn't have WallRead permission".to_string(),
            ));
        }

        // Get our posts that the requester hasn't seen yet
        // The cursor maps our peer_id to the highest lamport clock they've seen
        let our_cursor = cursor.get(&identity.peer_id).copied().unwrap_or(0);

        // Get posts newer than the cursor
        let posts = self.get_posts_after_cursor(&identity.peer_id, our_cursor, limit)?;

        // Build post summaries
        let post_summaries: Vec<PostSummary> = posts
            .iter()
            .map(|post| {
                let media_hashes =
                    PostsRepository::get_media_hashes(&self.db, &post.post_id).unwrap_or_default();

                PostSummary {
                    post_id: post.post_id.clone(),
                    author_peer_id: post.author_peer_id.clone(),
                    lamport_clock: post.lamport_clock as u64,
                    content_type: post.content_type.clone(),
                    has_media: !media_hashes.is_empty(),
                    media_hashes,
                    created_at: post.created_at,
                }
            })
            .collect();

        // Calculate next cursor
        let mut next_cursor = cursor.clone();
        if let Some(last_post) = posts.last() {
            next_cursor.insert(identity.peer_id.clone(), last_post.lamport_clock as u64);
        }

        let has_more = posts.len() as u32 >= limit;

        let response_timestamp = chrono::Utc::now().timestamp();

        let response_signable = SignableContentManifestResponse {
            responder_peer_id: identity.peer_id.clone(),
            posts: post_summaries.clone(),
            has_more,
            next_cursor: next_cursor.clone(),
            timestamp: response_timestamp,
        };

        let response_signature = self.identity_service.sign(&response_signable)?;

        Ok(OutgoingManifestResponse {
            responder_peer_id: identity.peer_id,
            posts: post_summaries,
            has_more,
            next_cursor,
            timestamp: response_timestamp,
            signature: response_signature,
        })
    }

    /// Process an incoming manifest response
    pub fn process_manifest_response(
        &self,
        responder_peer_id: &str,
        posts: &[PostSummary],
        has_more: bool,
        next_cursor: &HashMap<String, u64>,
        timestamp: i64,
        signature: &[u8],
    ) -> Result<Vec<String>> {
        // Verify the responder's signature
        let responder_public_key = self
            .contacts_service
            .get_public_key(responder_peer_id)?
            .ok_or_else(|| AppError::NotFound("Responder not in contacts".to_string()))?;

        let signable = SignableContentManifestResponse {
            responder_peer_id: responder_peer_id.to_string(),
            posts: posts.to_vec(),
            has_more,
            next_cursor: next_cursor.clone(),
            timestamp,
        };

        let verifying_key = VerifyingKey::from_bytes(
            responder_public_key
                .as_slice()
                .try_into()
                .map_err(|_| AppError::Crypto("Invalid public key length".to_string()))?,
        )
        .map_err(|e| AppError::Crypto(format!("Invalid public key: {}", e)))?;

        if !verify(&verifying_key, &signable, signature)? {
            return Err(AppError::Crypto(
                "Invalid manifest response signature".to_string(),
            ));
        }

        // Return list of post IDs we need to fetch
        let mut posts_to_fetch = Vec::new();

        for summary in posts {
            // Check if we already have this post with the same or newer lamport clock
            if let Some(existing) = PostsRepository::get_by_post_id(&self.db, &summary.post_id)
                .map_err(|e| AppError::DatabaseString(e.to_string()))?
            {
                if existing.lamport_clock as u64 >= summary.lamport_clock {
                    continue; // We have a newer or same version
                }
            }
            posts_to_fetch.push(summary.post_id.clone());
        }

        // Store the cursor for future requests
        self.store_sync_cursor(responder_peer_id, next_cursor)?;

        Ok(posts_to_fetch)
    }

    /// Store a post received from a peer
    pub fn store_remote_post(
        &self,
        post_id: &str,
        author_peer_id: &str,
        content_type: &str,
        content_text: Option<&str>,
        visibility: &str,
        lamport_clock: u64,
        created_at: i64,
        signature: &[u8],
    ) -> Result<()> {
        // Verify the signature
        let author_public_key = self
            .contacts_service
            .get_public_key(author_peer_id)?
            .ok_or_else(|| AppError::NotFound("Author not in contacts".to_string()))?;

        let signable = SignablePost {
            post_id: post_id.to_string(),
            author_peer_id: author_peer_id.to_string(),
            content_type: content_type.to_string(),
            content_text: content_text.map(String::from),
            media_hashes: Vec::new(), // Will be added separately
            visibility: visibility.to_string(),
            lamport_clock,
            created_at,
        };

        let verifying_key = VerifyingKey::from_bytes(
            author_public_key
                .as_slice()
                .try_into()
                .map_err(|_| AppError::Crypto("Invalid public key length".to_string()))?,
        )
        .map_err(|e| AppError::Crypto(format!("Invalid public key: {}", e)))?;

        if !verify(&verifying_key, &signable, signature)? {
            return Err(AppError::Crypto("Invalid post signature".to_string()));
        }

        // Check for existing post
        if let Some(existing) = PostsRepository::get_by_post_id(&self.db, post_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?
        {
            if existing.lamport_clock as u64 >= lamport_clock {
                return Ok(()); // We have a newer or same version
            }
            // Update existing post
            PostsRepository::update_post(
                &self.db,
                post_id,
                content_text,
                created_at,
                lamport_clock as i64,
            )
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;
        } else {
            // Insert new post
            let vis = PostVisibility::from_str(visibility).unwrap_or(PostVisibility::Contacts);

            let post_data = PostData {
                post_id: post_id.to_string(),
                author_peer_id: author_peer_id.to_string(),
                content_type: content_type.to_string(),
                content_text: content_text.map(String::from),
                visibility: vis,
                lamport_clock: lamport_clock as i64,
                created_at,
                signature: signature.to_vec(),
            };

            PostsRepository::insert_remote_post(&self.db, &post_data)
                .map_err(|e| AppError::DatabaseString(e.to_string()))?;
        }

        // Update lamport clock
        self.db
            .update_lamport_clock(author_peer_id, lamport_clock as i64)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        Ok(())
    }

    /// Get posts after a certain lamport clock cursor
    fn get_posts_after_cursor(
        &self,
        author_peer_id: &str,
        cursor: u64,
        limit: u32,
    ) -> Result<Vec<crate::db::Post>> {
        // For now, just get posts and filter by lamport clock
        // TODO: Add a more efficient query
        let posts = PostsRepository::get_by_author(&self.db, author_peer_id, 1000, None)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        let filtered: Vec<_> = posts
            .into_iter()
            .filter(|p| p.lamport_clock as u64 > cursor)
            .take(limit as usize)
            .collect();

        Ok(filtered)
    }

    /// Store sync cursor for a peer
    fn store_sync_cursor(&self, _peer_id: &str, _cursor: &HashMap<String, u64>) -> Result<()> {
        // For now, we don't persist the cursor - it's ephemeral
        // TODO: Persist cursor to database for resumable syncs
        Ok(())
    }

    /// Get stored sync cursor for a peer
    pub fn get_sync_cursor(&self, _peer_id: &str) -> Result<HashMap<String, u64>> {
        // For now, return empty cursor
        // TODO: Load from database
        Ok(HashMap::new())
    }
}
