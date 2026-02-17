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

/// A request to fetch a specific post
#[derive(Debug, Clone)]
pub struct OutgoingFetchRequest {
    pub requester_peer_id: String,
    pub post_id: String,
    pub include_media: bool,
    pub timestamp: i64,
    pub signature: Vec<u8>,
}

/// A response with full post content
#[derive(Debug, Clone)]
pub struct OutgoingFetchResponse {
    pub post_id: String,
    pub author_peer_id: String,
    pub content_type: String,
    pub content_text: Option<String>,
    pub visibility: String,
    pub lamport_clock: u64,
    pub created_at: i64,
    pub signature: Vec<u8>,
}

/// Parameters for storing a remote post received from a peer
pub struct RemotePostParams<'a> {
    pub post_id: &'a str,
    pub author_peer_id: &'a str,
    pub content_type: &'a str,
    pub content_text: Option<&'a str>,
    pub visibility: &'a str,
    pub lamport_clock: u64,
    pub created_at: i64,
    pub signature: &'a [u8],
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

    /// Get a reference to the underlying database (used for direct post_media writes)
    pub fn db(&self) -> &Database {
        &self.db
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
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

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

    /// Create a fetch request to send to a peer
    pub fn create_fetch_request(
        &self,
        post_id: String,
        include_media: bool,
    ) -> Result<OutgoingFetchRequest> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        let timestamp = chrono::Utc::now().timestamp();

        // Sign the fetch request parameters
        let sign_data = format!(
            "fetch:{}:{}:{}:{}",
            identity.peer_id, post_id, include_media, timestamp
        );
        let signature = self.identity_service.sign_raw(sign_data.as_bytes())?;

        Ok(OutgoingFetchRequest {
            requester_peer_id: identity.peer_id,
            post_id,
            include_media,
            timestamp,
            signature,
        })
    }

    /// Process an incoming fetch request and return the post if authorized
    pub fn process_fetch_request(
        &self,
        requester_peer_id: &str,
        post_id: &str,
        _include_media: bool,
        timestamp: i64,
        signature: &[u8],
    ) -> Result<OutgoingFetchResponse> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        // Validate timestamp is within acceptable window (5 minutes)
        let now = chrono::Utc::now().timestamp();
        let time_diff = (now - timestamp).abs();
        if time_diff > 300 {
            return Err(AppError::Crypto(format!(
                "Request timestamp too old or in future: {} seconds difference",
                time_diff
            )));
        }

        // Verify the requester's signature
        let requester_public_key = self
            .contacts_service
            .get_public_key(requester_peer_id)?
            .ok_or_else(|| AppError::NotFound("Requester not in contacts".to_string()))?;

        // Reconstruct the signed data (must match create_fetch_request format)
        let sign_data = format!(
            "fetch:{}:{}:{}:{}",
            requester_peer_id, post_id, _include_media, timestamp
        );

        let verifying_key = VerifyingKey::from_bytes(
            requester_public_key
                .as_slice()
                .try_into()
                .map_err(|_| AppError::Crypto("Invalid public key length".to_string()))?,
        )
        .map_err(|e| AppError::Crypto(format!("Invalid public key: {}", e)))?;

        // Verify signature using raw verification
        use ed25519_dalek::Verifier;
        let sig = ed25519_dalek::Signature::from_slice(signature)
            .map_err(|e| AppError::Crypto(format!("Invalid signature format: {}", e)))?;
        verifying_key
            .verify(sign_data.as_bytes(), &sig)
            .map_err(|_| AppError::Crypto("Invalid fetch request signature".to_string()))?;

        // Check if the requester has WallRead permission from us
        if !self
            .permissions_service
            .peer_has_capability(requester_peer_id, Capability::WallRead)?
        {
            return Err(AppError::PermissionDenied(
                "Requester doesn't have WallRead permission".to_string(),
            ));
        }

        // Get the post
        let post = PostsRepository::get_by_post_id(&self.db, post_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?
            .ok_or_else(|| AppError::NotFound(format!("Post {} not found", post_id)))?;

        // Verify this is our post (we can only serve our own posts)
        if post.author_peer_id != identity.peer_id {
            return Err(AppError::PermissionDenied(
                "Can only serve own posts".to_string(),
            ));
        }

        // Check visibility - for Contacts visibility, requester must be in contacts
        // (which we already verified above via WallRead permission check)
        // For Public, anyone with WallRead can access
        // Note: We don't serve posts with other visibility levels

        Ok(OutgoingFetchResponse {
            post_id: post.post_id,
            author_peer_id: post.author_peer_id,
            content_type: post.content_type,
            content_text: post.content_text,
            visibility: post.visibility.to_string(),
            lamport_clock: post.lamport_clock as u64,
            created_at: post.created_at,
            signature: post.signature,
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
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

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
    pub fn store_remote_post(&self, params: &RemotePostParams<'_>) -> Result<()> {
        let post_id = params.post_id;
        let author_peer_id = params.author_peer_id;
        let content_type = params.content_type;
        let content_text = params.content_text;
        let visibility = params.visibility;
        let lamport_clock = params.lamport_clock;
        let created_at = params.created_at;
        let signature = params.signature;
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
        let posts = PostsRepository::get_by_author_after_cursor(
            &self.db,
            author_peer_id,
            cursor as i64,
            limit as i64,
        )
        .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        Ok(posts)
    }

    /// Store sync cursor for a peer
    fn store_sync_cursor(&self, peer_id: &str, cursor: &HashMap<String, u64>) -> Result<()> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        // We are syncing *from* peer_id, so the cursor is keyed by (source_peer_id=peer_id)
        // for our local identity.
        self.db
            .update_sync_cursors_batch(peer_id, "posts", cursor)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        // Also update last_sync_at even if cursor is empty.
        // We do this by recording our own peer as author with existing clock 0 if needed.
        if cursor.is_empty() {
            self.db
                .update_sync_cursor(peer_id, "posts", &identity.peer_id, 0)
                .map_err(|e| AppError::DatabaseString(e.to_string()))?;
        }

        Ok(())
    }

    /// Get stored sync cursor for a peer
    pub fn get_sync_cursor(&self, peer_id: &str) -> Result<HashMap<String, u64>> {
        self.db
            .get_sync_cursor(peer_id, "posts")
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{ContactData, ContactsRepository};
    use crate::models::CreateIdentityRequest;
    use crate::services::{ContactsService, IdentityService, PermissionsService};
    use std::sync::Arc;

    fn create_test_env() -> (
        ContentSyncService,
        Arc<Database>,
        Arc<IdentityService>,
        String, // our peer_id
    ) {
        let db = Arc::new(Database::in_memory().unwrap());
        let identity_service = Arc::new(IdentityService::new(db.clone()));
        let contacts_service = Arc::new(ContactsService::new(db.clone(), identity_service.clone()));
        let permissions_service = Arc::new(PermissionsService::new(
            db.clone(),
            identity_service.clone(),
        ));

        let info = identity_service
            .create_identity(CreateIdentityRequest {
                display_name: "Sync User".to_string(),
                passphrase: "test-pass".to_string(),
                bio: None,
                passphrase_hint: None,
            })
            .unwrap();

        let service = ContentSyncService::new(
            db.clone(),
            identity_service.clone(),
            contacts_service,
            permissions_service,
        );

        (service, db, identity_service, info.peer_id)
    }

    #[test]
    fn test_create_manifest_request() {
        let (service, _db, _identity, peer_id) = create_test_env();

        let mut cursor = HashMap::new();
        cursor.insert("12D3KooWPeer1".to_string(), 5u64);

        let request = service.create_manifest_request(cursor.clone(), 50).unwrap();

        assert_eq!(request.requester_peer_id, peer_id);
        assert_eq!(request.cursor, cursor);
        assert_eq!(request.limit, 50);
        assert!(!request.signature.is_empty());
    }

    #[test]
    fn test_create_manifest_request_empty_cursor() {
        let (service, _db, _identity, peer_id) = create_test_env();

        let request = service
            .create_manifest_request(HashMap::new(), 100)
            .unwrap();

        assert_eq!(request.requester_peer_id, peer_id);
        assert!(request.cursor.is_empty());
    }

    #[test]
    fn test_create_manifest_request_requires_identity() {
        let db = Arc::new(Database::in_memory().unwrap());
        let identity_service = Arc::new(IdentityService::new(db.clone()));
        let contacts_service = Arc::new(ContactsService::new(db.clone(), identity_service.clone()));
        let permissions_service = Arc::new(PermissionsService::new(
            db.clone(),
            identity_service.clone(),
        ));

        let service =
            ContentSyncService::new(db, identity_service, contacts_service, permissions_service);

        let result = service.create_manifest_request(HashMap::new(), 50);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_fetch_request() {
        let (service, _db, _identity, peer_id) = create_test_env();

        let request = service
            .create_fetch_request("post-123".to_string(), true)
            .unwrap();

        assert_eq!(request.requester_peer_id, peer_id);
        assert_eq!(request.post_id, "post-123");
        assert!(request.include_media);
        assert!(!request.signature.is_empty());
    }

    #[test]
    fn test_create_fetch_request_no_media() {
        let (service, _db, _identity, _peer_id) = create_test_env();

        let request = service
            .create_fetch_request("post-456".to_string(), false)
            .unwrap();

        assert!(!request.include_media);
    }

    #[test]
    fn test_get_sync_cursor_empty() {
        let (service, _db, _identity, _peer_id) = create_test_env();

        let cursor = service.get_sync_cursor("12D3KooWPeer1").unwrap();
        assert!(cursor.is_empty());
    }

    #[test]
    fn test_store_remote_post_new() {
        let (service, db, _identity_service, _peer_id) = create_test_env();

        // Create a peer with a real signing key so we can create a valid signature
        let (peer_signing, peer_verifying) =
            crate::services::CryptoService::generate_ed25519_keypair();
        let peer_peer_id = "12D3KooWRemotePeer".to_string();

        // Add the peer as a contact with their real public key
        let contact_data = ContactData {
            peer_id: peer_peer_id.clone(),
            public_key: peer_verifying.to_bytes().to_vec(),
            x25519_public: vec![0u8; 32],
            display_name: "Remote Peer".to_string(),
            avatar_hash: None,
            bio: None,
        };
        ContactsRepository::add_contact(&db, &contact_data).unwrap();

        // Create a properly signed post
        let signable = crate::services::SignablePost {
            post_id: "remote-post-1".to_string(),
            author_peer_id: peer_peer_id.clone(),
            content_type: "text".to_string(),
            content_text: Some("Remote post content".to_string()),
            media_hashes: vec![],
            visibility: "public".to_string(),
            lamport_clock: 1,
            created_at: 1000,
        };
        let signature = crate::services::sign(&peer_signing, &signable).unwrap();

        service
            .store_remote_post(&RemotePostParams {
                post_id: "remote-post-1",
                author_peer_id: &peer_peer_id,
                content_type: "text",
                content_text: Some("Remote post content"),
                visibility: "public",
                lamport_clock: 1,
                created_at: 1000,
                signature: &signature,
            })
            .unwrap();

        // Verify post was stored
        let post = PostsRepository::get_by_post_id(&db, "remote-post-1")
            .unwrap()
            .unwrap();
        assert_eq!(post.content_text, Some("Remote post content".to_string()));
    }

    #[test]
    fn test_store_remote_post_invalid_signature() {
        let (service, db, _identity, _peer_id) = create_test_env();

        // Add a contact
        let (_peer_signing, peer_verifying) =
            crate::services::CryptoService::generate_ed25519_keypair();
        let peer_peer_id = "12D3KooWRemotePeer".to_string();
        let contact_data = ContactData {
            peer_id: peer_peer_id.clone(),
            public_key: peer_verifying.to_bytes().to_vec(),
            x25519_public: vec![0u8; 32],
            display_name: "Remote Peer".to_string(),
            avatar_hash: None,
            bio: None,
        };
        ContactsRepository::add_contact(&db, &contact_data).unwrap();

        // Try to store with an invalid signature
        let result = service.store_remote_post(&RemotePostParams {
            post_id: "remote-post-bad",
            author_peer_id: &peer_peer_id,
            content_type: "text",
            content_text: Some("Bad post"),
            visibility: "public",
            lamport_clock: 1,
            created_at: 1000,
            signature: &vec![0u8; 64], // Invalid signature
        });

        assert!(result.is_err());
    }

    #[test]
    fn test_store_remote_post_unknown_contact() {
        let (service, _db, _identity, _peer_id) = create_test_env();

        let result = service.store_remote_post(&RemotePostParams {
            post_id: "remote-post",
            author_peer_id: "12D3KooWUnknownPeer",
            content_type: "text",
            content_text: Some("Post"),
            visibility: "public",
            lamport_clock: 1,
            created_at: 1000,
            signature: &vec![0u8; 64],
        });

        assert!(result.is_err());
    }

    #[test]
    fn test_store_remote_post_update_existing() {
        let (service, db, _identity, _peer_id) = create_test_env();

        let (peer_signing, peer_verifying) =
            crate::services::CryptoService::generate_ed25519_keypair();
        let peer_peer_id = "12D3KooWRemotePeer".to_string();

        let contact_data = ContactData {
            peer_id: peer_peer_id.clone(),
            public_key: peer_verifying.to_bytes().to_vec(),
            x25519_public: vec![0u8; 32],
            display_name: "Remote Peer".to_string(),
            avatar_hash: None,
            bio: None,
        };
        ContactsRepository::add_contact(&db, &contact_data).unwrap();

        // Store first version
        let signable1 = crate::services::SignablePost {
            post_id: "remote-post-1".to_string(),
            author_peer_id: peer_peer_id.clone(),
            content_type: "text".to_string(),
            content_text: Some("Version 1".to_string()),
            media_hashes: vec![],
            visibility: "public".to_string(),
            lamport_clock: 1,
            created_at: 1000,
        };
        let sig1 = crate::services::sign(&peer_signing, &signable1).unwrap();

        service
            .store_remote_post(&RemotePostParams {
                post_id: "remote-post-1",
                author_peer_id: &peer_peer_id,
                content_type: "text",
                content_text: Some("Version 1"),
                visibility: "public",
                lamport_clock: 1,
                created_at: 1000,
                signature: &sig1,
            })
            .unwrap();

        // Store updated version with higher lamport clock
        let signable2 = crate::services::SignablePost {
            post_id: "remote-post-1".to_string(),
            author_peer_id: peer_peer_id.clone(),
            content_type: "text".to_string(),
            content_text: Some("Version 2".to_string()),
            media_hashes: vec![],
            visibility: "public".to_string(),
            lamport_clock: 2,
            created_at: 1000,
        };
        let sig2 = crate::services::sign(&peer_signing, &signable2).unwrap();

        service
            .store_remote_post(&RemotePostParams {
                post_id: "remote-post-1",
                author_peer_id: &peer_peer_id,
                content_type: "text",
                content_text: Some("Version 2"),
                visibility: "public",
                lamport_clock: 2,
                created_at: 1000,
                signature: &sig2,
            })
            .unwrap();

        // Should have the updated content
        let post = PostsRepository::get_by_post_id(&db, "remote-post-1")
            .unwrap()
            .unwrap();
        assert_eq!(post.content_text, Some("Version 2".to_string()));
        assert_eq!(post.lamport_clock, 2);
    }

    #[test]
    fn test_store_remote_post_skip_older_version() {
        let (service, db, _identity, _peer_id) = create_test_env();

        let (peer_signing, peer_verifying) =
            crate::services::CryptoService::generate_ed25519_keypair();
        let peer_peer_id = "12D3KooWRemotePeer".to_string();

        let contact_data = ContactData {
            peer_id: peer_peer_id.clone(),
            public_key: peer_verifying.to_bytes().to_vec(),
            x25519_public: vec![0u8; 32],
            display_name: "Remote Peer".to_string(),
            avatar_hash: None,
            bio: None,
        };
        ContactsRepository::add_contact(&db, &contact_data).unwrap();

        // Store version with lamport_clock=5
        let signable1 = crate::services::SignablePost {
            post_id: "remote-post-1".to_string(),
            author_peer_id: peer_peer_id.clone(),
            content_type: "text".to_string(),
            content_text: Some("Newer version".to_string()),
            media_hashes: vec![],
            visibility: "public".to_string(),
            lamport_clock: 5,
            created_at: 1000,
        };
        let sig1 = crate::services::sign(&peer_signing, &signable1).unwrap();

        service
            .store_remote_post(&RemotePostParams {
                post_id: "remote-post-1",
                author_peer_id: &peer_peer_id,
                content_type: "text",
                content_text: Some("Newer version"),
                visibility: "public",
                lamport_clock: 5,
                created_at: 1000,
                signature: &sig1,
            })
            .unwrap();

        // Try to store older version with lamport_clock=3
        let signable2 = crate::services::SignablePost {
            post_id: "remote-post-1".to_string(),
            author_peer_id: peer_peer_id.clone(),
            content_type: "text".to_string(),
            content_text: Some("Older version".to_string()),
            media_hashes: vec![],
            visibility: "public".to_string(),
            lamport_clock: 3,
            created_at: 1000,
        };
        let sig2 = crate::services::sign(&peer_signing, &signable2).unwrap();

        // This should succeed but not update (older version is skipped)
        service
            .store_remote_post(&RemotePostParams {
                post_id: "remote-post-1",
                author_peer_id: &peer_peer_id,
                content_type: "text",
                content_text: Some("Older version"),
                visibility: "public",
                lamport_clock: 3,
                created_at: 1000,
                signature: &sig2,
            })
            .unwrap();

        // Content should still be the newer version
        let post = PostsRepository::get_by_post_id(&db, "remote-post-1")
            .unwrap()
            .unwrap();
        assert_eq!(post.content_text, Some("Newer version".to_string()));
        assert_eq!(post.lamport_clock, 5);
    }
}
