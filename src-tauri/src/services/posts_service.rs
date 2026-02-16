//! Posts service for managing wall/blog posts

use ed25519_dalek::VerifyingKey;
use std::sync::Arc;
use uuid::Uuid;

use crate::db::{
    Capability, Database, Post, PostData, PostMedia, PostMediaData, PostVisibility,
    PostsRepository, RecordPostEventParams,
};
use crate::error::{AppError, Result};
use crate::services::{
    verify, ContactsService, IdentityService, PermissionsService, Signable, SignablePost,
    SignablePostDelete, SignablePostUpdate,
};

/// Service for managing wall/blog posts
pub struct PostsService {
    db: Arc<Database>,
    identity_service: Arc<IdentityService>,
    contacts_service: Arc<ContactsService>,
    permissions_service: Arc<PermissionsService>,
}

/// A post ready to be synced over the network
#[derive(Debug, Clone)]
pub struct OutgoingPost {
    pub post_id: String,
    pub author_peer_id: String,
    pub content_type: String,
    pub content_text: Option<String>,
    pub media_hashes: Vec<String>,
    pub visibility: String,
    pub lamport_clock: u64,
    pub created_at: i64,
    pub signature: Vec<u8>,
}

/// A post update ready to be synced
#[derive(Debug, Clone)]
pub struct OutgoingPostUpdate {
    pub post_id: String,
    pub author_peer_id: String,
    pub content_text: Option<String>,
    pub lamport_clock: u64,
    pub updated_at: i64,
    pub signature: Vec<u8>,
}

/// A post delete ready to be synced
#[derive(Debug, Clone)]
pub struct OutgoingPostDelete {
    pub post_id: String,
    pub author_peer_id: String,
    pub lamport_clock: u64,
    pub deleted_at: i64,
    pub signature: Vec<u8>,
}

/// Parameters for adding media to a post
pub struct AddMediaParams<'a> {
    pub post_id: &'a str,
    pub media_hash: &'a str,
    pub media_type: &'a str,
    pub mime_type: &'a str,
    pub file_name: &'a str,
    pub file_size: i64,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub duration_seconds: Option<i32>,
    pub sort_order: i32,
}

/// Parameters for processing an incoming post from the network
pub struct IncomingPostParams<'a> {
    pub post_id: &'a str,
    pub author_peer_id: &'a str,
    pub content_type: &'a str,
    pub content_text: Option<&'a str>,
    pub media_hashes: &'a [String],
    pub visibility: &'a str,
    pub lamport_clock: u64,
    pub created_at: i64,
    pub signature: &'a [u8],
}

impl PostsService {
    /// Create a new posts service
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

    /// Create a new post
    pub fn create_post(
        &self,
        content_type: &str,
        content_text: Option<&str>,
        visibility: PostVisibility,
    ) -> Result<OutgoingPost> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        let post_id = Uuid::new_v4().to_string();
        let lamport_clock =
            self.db
                .next_lamport_clock(&identity.peer_id)
                .map_err(|e| AppError::DatabaseString(e.to_string()))? as u64;
        let created_at = chrono::Utc::now().timestamp();

        // Create signable
        let signable = SignablePost {
            post_id: post_id.clone(),
            author_peer_id: identity.peer_id.clone(),
            content_type: content_type.to_string(),
            content_text: content_text.map(String::from),
            media_hashes: Vec::new(), // Media added separately
            visibility: visibility.to_string(),
            lamport_clock,
            created_at,
        };

        let signature = self.identity_service.sign(&signable)?;

        // Store locally
        let post_data = PostData {
            post_id: post_id.clone(),
            author_peer_id: identity.peer_id.clone(),
            content_type: content_type.to_string(),
            content_text: content_text.map(String::from),
            visibility,
            lamport_clock: lamport_clock as i64,
            created_at,
            signature: signature.clone(),
        };

        PostsRepository::insert_post(&self.db, &post_data)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        // Record event
        let event_id = format!("created:{}", post_id);
        let payload_cbor = signable.signable_bytes()?;
        PostsRepository::record_post_event(
            &self.db,
            &RecordPostEventParams {
                event_id: &event_id,
                event_type: "created",
                post_id: &post_id,
                author_peer_id: &identity.peer_id,
                lamport_clock: lamport_clock as i64,
                timestamp: created_at,
                payload_cbor: &payload_cbor,
                signature: &signature,
            },
        )
        .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        Ok(OutgoingPost {
            post_id,
            author_peer_id: identity.peer_id,
            content_type: content_type.to_string(),
            content_text: content_text.map(String::from),
            media_hashes: Vec::new(),
            visibility: visibility.to_string(),
            lamport_clock,
            created_at,
            signature,
        })
    }

    /// Update a post's content
    pub fn update_post(
        &self,
        post_id: &str,
        content_text: Option<&str>,
    ) -> Result<OutgoingPostUpdate> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        // Verify we own the post
        let post = PostsRepository::get_by_post_id(&self.db, post_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?
            .ok_or_else(|| AppError::NotFound("Post not found".to_string()))?;

        if post.author_peer_id != identity.peer_id {
            return Err(AppError::PermissionDenied(
                "Cannot update another user's post".to_string(),
            ));
        }

        let lamport_clock =
            self.db
                .next_lamport_clock(&identity.peer_id)
                .map_err(|e| AppError::DatabaseString(e.to_string()))? as u64;
        let updated_at = chrono::Utc::now().timestamp();

        // Create signable
        let signable = SignablePostUpdate {
            post_id: post_id.to_string(),
            author_peer_id: identity.peer_id.clone(),
            content_text: content_text.map(String::from),
            lamport_clock,
            updated_at,
        };

        let signature = self.identity_service.sign(&signable)?;

        // Update locally
        PostsRepository::update_post(
            &self.db,
            post_id,
            content_text,
            updated_at,
            lamport_clock as i64,
        )
        .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        // Record event
        let event_id = format!("updated:{}:{}", post_id, lamport_clock);
        let payload_cbor = signable.signable_bytes()?;
        PostsRepository::record_post_event(
            &self.db,
            &RecordPostEventParams {
                event_id: &event_id,
                event_type: "updated",
                post_id,
                author_peer_id: &identity.peer_id,
                lamport_clock: lamport_clock as i64,
                timestamp: updated_at,
                payload_cbor: &payload_cbor,
                signature: &signature,
            },
        )
        .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        Ok(OutgoingPostUpdate {
            post_id: post_id.to_string(),
            author_peer_id: identity.peer_id,
            content_text: content_text.map(String::from),
            lamport_clock,
            updated_at,
            signature,
        })
    }

    /// Delete a post (soft delete)
    pub fn delete_post(&self, post_id: &str) -> Result<OutgoingPostDelete> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        // Verify we own the post
        let post = PostsRepository::get_by_post_id(&self.db, post_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?
            .ok_or_else(|| AppError::NotFound("Post not found".to_string()))?;

        if post.author_peer_id != identity.peer_id {
            return Err(AppError::PermissionDenied(
                "Cannot delete another user's post".to_string(),
            ));
        }

        let lamport_clock =
            self.db
                .next_lamport_clock(&identity.peer_id)
                .map_err(|e| AppError::DatabaseString(e.to_string()))? as u64;
        let deleted_at = chrono::Utc::now().timestamp();

        // Create signable
        let signable = SignablePostDelete {
            post_id: post_id.to_string(),
            author_peer_id: identity.peer_id.clone(),
            lamport_clock,
            deleted_at,
        };

        let signature = self.identity_service.sign(&signable)?;

        // Delete locally
        PostsRepository::delete_post(&self.db, post_id, deleted_at)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        // Record event
        let event_id = format!("deleted:{}", post_id);
        let payload_cbor = signable.signable_bytes()?;
        PostsRepository::record_post_event(
            &self.db,
            &RecordPostEventParams {
                event_id: &event_id,
                event_type: "deleted",
                post_id,
                author_peer_id: &identity.peer_id,
                lamport_clock: lamport_clock as i64,
                timestamp: deleted_at,
                payload_cbor: &payload_cbor,
                signature: &signature,
            },
        )
        .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        Ok(OutgoingPostDelete {
            post_id: post_id.to_string(),
            author_peer_id: identity.peer_id,
            lamport_clock,
            deleted_at,
            signature,
        })
    }

    /// Add media to a post
    pub fn add_media_to_post(&self, params: &AddMediaParams<'_>) -> Result<()> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        // Verify we own the post
        let post = PostsRepository::get_by_post_id(&self.db, params.post_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?
            .ok_or_else(|| AppError::NotFound("Post not found".to_string()))?;

        if post.author_peer_id != identity.peer_id {
            return Err(AppError::PermissionDenied(
                "Cannot add media to another user's post".to_string(),
            ));
        }

        let media_data = PostMediaData {
            post_id: params.post_id.to_string(),
            media_hash: params.media_hash.to_string(),
            media_type: params.media_type.to_string(),
            mime_type: params.mime_type.to_string(),
            file_name: params.file_name.to_string(),
            file_size: params.file_size,
            width: params.width,
            height: params.height,
            duration_seconds: params.duration_seconds,
            sort_order: params.sort_order,
        };

        PostsRepository::add_media(&self.db, &media_data)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Get media for a post
    pub fn get_post_media(&self, post_id: &str) -> Result<Vec<PostMedia>> {
        PostsRepository::get_post_media(&self.db, post_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Get a post by ID
    pub fn get_post(&self, post_id: &str) -> Result<Option<Post>> {
        PostsRepository::get_by_post_id(&self.db, post_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Get local user's posts (their wall)
    pub fn get_my_posts(&self, limit: i64, before_timestamp: Option<i64>) -> Result<Vec<Post>> {
        // Verify identity exists
        let _identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        PostsRepository::get_local_posts(&self.db, limit, before_timestamp)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Get posts by a specific author (for viewing their wall)
    pub fn get_posts_by_author(
        &self,
        author_peer_id: &str,
        limit: i64,
        before_timestamp: Option<i64>,
    ) -> Result<Vec<Post>> {
        // If not our posts, check we have WallRead permission
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        if author_peer_id != identity.peer_id {
            // Check if they've granted us WallRead permission
            if !self
                .permissions_service
                .we_have_capability(author_peer_id, Capability::WallRead)?
            {
                return Err(AppError::PermissionDenied(
                    "No permission to view this user's wall".to_string(),
                ));
            }
        }

        PostsRepository::get_by_author(&self.db, author_peer_id, limit, before_timestamp)
            .map_err(|e| AppError::DatabaseString(e.to_string()))
    }

    /// Process an incoming post from the network
    pub fn process_incoming_post(&self, params: &IncomingPostParams<'_>) -> Result<()> {
        let post_id = params.post_id;
        let author_peer_id = params.author_peer_id;
        let content_type = params.content_type;
        let content_text = params.content_text;
        let media_hashes = params.media_hashes;
        let visibility = params.visibility;
        let lamport_clock = params.lamport_clock;
        let created_at = params.created_at;
        let signature = params.signature;
        // Get author's public key for verification
        let author_public_key = self
            .contacts_service
            .get_public_key(author_peer_id)?
            .ok_or_else(|| AppError::NotFound("Author not in contacts".to_string()))?;

        // Verify signature
        let signable = SignablePost {
            post_id: post_id.to_string(),
            author_peer_id: author_peer_id.to_string(),
            content_type: content_type.to_string(),
            content_text: content_text.map(String::from),
            media_hashes: media_hashes.to_vec(),
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

        // Check if we already have this post
        if let Some(existing) = PostsRepository::get_by_post_id(&self.db, post_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?
        {
            // Accept if higher lamport clock
            if lamport_clock <= existing.lamport_clock as u64 {
                return Ok(()); // Already have newer or same version
            }
        }

        // Update lamport clock
        self.db
            .update_lamport_clock(author_peer_id, lamport_clock as i64)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        // Parse visibility
        let vis = match visibility {
            "contacts" => PostVisibility::Contacts,
            "public" => PostVisibility::Public,
            _ => {
                return Err(AppError::Validation(format!(
                    "Invalid visibility: {}",
                    visibility
                )))
            }
        };

        // Store post
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

        // Use upsert behavior
        if PostsRepository::get_by_post_id(&self.db, post_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?
            .is_some()
        {
            // Update existing - use update_post but with full content
            PostsRepository::update_post(
                &self.db,
                post_id,
                content_text,
                created_at,
                lamport_clock as i64,
            )
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;
        } else {
            PostsRepository::insert_remote_post(&self.db, &post_data)
                .map_err(|e| AppError::DatabaseString(e.to_string()))?;
        }

        // Record event
        let event_id = format!("received:{}:{}", post_id, lamport_clock);
        let payload_cbor = signable.signable_bytes()?;
        PostsRepository::record_post_event(
            &self.db,
            &RecordPostEventParams {
                event_id: &event_id,
                event_type: "received",
                post_id,
                author_peer_id,
                lamport_clock: lamport_clock as i64,
                timestamp: created_at,
                payload_cbor: &payload_cbor,
                signature,
            },
        )
        .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        Ok(())
    }

    /// Process an incoming post update
    pub fn process_incoming_post_update(
        &self,
        post_id: &str,
        author_peer_id: &str,
        content_text: Option<&str>,
        lamport_clock: u64,
        updated_at: i64,
        signature: &[u8],
    ) -> Result<()> {
        // Get author's public key
        let author_public_key = self
            .contacts_service
            .get_public_key(author_peer_id)?
            .ok_or_else(|| AppError::NotFound("Author not in contacts".to_string()))?;

        // Verify signature
        let signable = SignablePostUpdate {
            post_id: post_id.to_string(),
            author_peer_id: author_peer_id.to_string(),
            content_text: content_text.map(String::from),
            lamport_clock,
            updated_at,
        };

        let verifying_key = VerifyingKey::from_bytes(
            author_public_key
                .as_slice()
                .try_into()
                .map_err(|_| AppError::Crypto("Invalid public key length".to_string()))?,
        )
        .map_err(|e| AppError::Crypto(format!("Invalid public key: {}", e)))?;

        if !verify(&verifying_key, &signable, signature)? {
            return Err(AppError::Crypto(
                "Invalid post update signature".to_string(),
            ));
        }

        // Check we have the post and it's older
        let existing = PostsRepository::get_by_post_id(&self.db, post_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?
            .ok_or_else(|| AppError::NotFound("Post not found".to_string()))?;

        if lamport_clock <= existing.lamport_clock as u64 {
            return Ok(()); // Already have newer or same version
        }

        // Update lamport clock
        self.db
            .update_lamport_clock(author_peer_id, lamport_clock as i64)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        // Update post
        PostsRepository::update_post(
            &self.db,
            post_id,
            content_text,
            updated_at,
            lamport_clock as i64,
        )
        .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        // Record event
        let event_id = format!("updated:{}:{}", post_id, lamport_clock);
        let payload_cbor = signable.signable_bytes()?;
        PostsRepository::record_post_event(
            &self.db,
            &RecordPostEventParams {
                event_id: &event_id,
                event_type: "updated",
                post_id,
                author_peer_id,
                lamport_clock: lamport_clock as i64,
                timestamp: updated_at,
                payload_cbor: &payload_cbor,
                signature,
            },
        )
        .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        Ok(())
    }

    /// Get the database reference (for testing)
    #[cfg(test)]
    pub fn db(&self) -> &Database {
        &self.db
    }

    /// Process an incoming post delete
    pub fn process_incoming_post_delete(
        &self,
        post_id: &str,
        author_peer_id: &str,
        lamport_clock: u64,
        deleted_at: i64,
        signature: &[u8],
    ) -> Result<()> {
        // Get author's public key
        let author_public_key = self
            .contacts_service
            .get_public_key(author_peer_id)?
            .ok_or_else(|| AppError::NotFound("Author not in contacts".to_string()))?;

        // Verify signature
        let signable = SignablePostDelete {
            post_id: post_id.to_string(),
            author_peer_id: author_peer_id.to_string(),
            lamport_clock,
            deleted_at,
        };

        let verifying_key = VerifyingKey::from_bytes(
            author_public_key
                .as_slice()
                .try_into()
                .map_err(|_| AppError::Crypto("Invalid public key length".to_string()))?,
        )
        .map_err(|e| AppError::Crypto(format!("Invalid public key: {}", e)))?;

        if !verify(&verifying_key, &signable, signature)? {
            return Err(AppError::Crypto(
                "Invalid post delete signature".to_string(),
            ));
        }

        // Check we have the post
        let existing = PostsRepository::get_by_post_id(&self.db, post_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        if let Some(post) = existing {
            if lamport_clock <= post.lamport_clock as u64 {
                return Ok(()); // Already have newer or same version
            }
        }

        // Update lamport clock
        self.db
            .update_lamport_clock(author_peer_id, lamport_clock as i64)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        // Delete post
        PostsRepository::delete_post(&self.db, post_id, deleted_at)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        // Record event
        let event_id = format!("deleted:{}", post_id);
        let payload_cbor = signable.signable_bytes()?;
        PostsRepository::record_post_event(
            &self.db,
            &RecordPostEventParams {
                event_id: &event_id,
                event_type: "deleted",
                post_id,
                author_peer_id,
                lamport_clock: lamport_clock as i64,
                timestamp: deleted_at,
                payload_cbor: &payload_cbor,
                signature,
            },
        )
        .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CreateIdentityRequest;
    use crate::services::{ContactsService, PermissionsService};
    use std::sync::Arc;

    /// Create a full test environment with identity service that has a created+unlocked identity.
    fn create_test_env() -> (
        Arc<Database>,
        Arc<IdentityService>,
        Arc<ContactsService>,
        Arc<PermissionsService>,
        PostsService,
        String, // peer_id of the created identity
    ) {
        let db = Arc::new(Database::in_memory().unwrap());
        let identity_service = Arc::new(IdentityService::new(db.clone()));
        let contacts_service = Arc::new(ContactsService::new(db.clone(), identity_service.clone()));
        let permissions_service = Arc::new(PermissionsService::new(
            db.clone(),
            identity_service.clone(),
        ));
        let posts_service = PostsService::new(
            db.clone(),
            identity_service.clone(),
            contacts_service.clone(),
            permissions_service.clone(),
        );

        // Create and unlock identity
        let info = identity_service
            .create_identity(CreateIdentityRequest {
                display_name: "Test User".to_string(),
                passphrase: "test-pass".to_string(),
                bio: None,
                passphrase_hint: None,
            })
            .unwrap();

        let peer_id = info.peer_id;

        (
            db,
            identity_service,
            contacts_service,
            permissions_service,
            posts_service,
            peer_id,
        )
    }

    #[test]
    fn test_create_post_success() {
        let (_db, _identity, _contacts, _perms, service, peer_id) = create_test_env();

        let post = service
            .create_post("text", Some("Hello, world!"), PostVisibility::Public)
            .unwrap();

        assert!(!post.post_id.is_empty());
        assert_eq!(post.author_peer_id, peer_id);
        assert_eq!(post.content_type, "text");
        assert_eq!(post.content_text, Some("Hello, world!".to_string()));
        assert_eq!(post.visibility, "public");
        assert!(!post.signature.is_empty());
    }

    #[test]
    fn test_create_post_contacts_visibility() {
        let (_db, _identity, _contacts, _perms, service, _peer_id) = create_test_env();

        let post = service
            .create_post("text", Some("Private post"), PostVisibility::Contacts)
            .unwrap();

        assert_eq!(post.visibility, "contacts");
    }

    #[test]
    fn test_create_post_none_content() {
        let (_db, _identity, _contacts, _perms, service, _peer_id) = create_test_env();

        let post = service
            .create_post("text", None, PostVisibility::Public)
            .unwrap();

        assert_eq!(post.content_text, None);
    }

    #[test]
    fn test_create_post_increments_lamport_clock() {
        let (_db, _identity, _contacts, _perms, service, _peer_id) = create_test_env();

        let post1 = service
            .create_post("text", Some("Post 1"), PostVisibility::Public)
            .unwrap();
        let post2 = service
            .create_post("text", Some("Post 2"), PostVisibility::Public)
            .unwrap();

        assert!(post2.lamport_clock > post1.lamport_clock);
    }

    #[test]
    fn test_create_post_requires_identity() {
        let db = Arc::new(Database::in_memory().unwrap());
        let identity_service = Arc::new(IdentityService::new(db.clone()));
        let contacts_service = Arc::new(ContactsService::new(db.clone(), identity_service.clone()));
        let permissions_service = Arc::new(PermissionsService::new(
            db.clone(),
            identity_service.clone(),
        ));
        let posts_service =
            PostsService::new(db, identity_service, contacts_service, permissions_service);

        let result = posts_service.create_post("text", Some("Hello"), PostVisibility::Public);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_post() {
        let (_db, _identity, _contacts, _perms, service, _peer_id) = create_test_env();

        let created = service
            .create_post("text", Some("Test post"), PostVisibility::Public)
            .unwrap();

        let retrieved = service.get_post(&created.post_id).unwrap();
        assert!(retrieved.is_some());

        let post = retrieved.unwrap();
        assert_eq!(post.post_id, created.post_id);
        assert_eq!(post.content_text, Some("Test post".to_string()));
    }

    #[test]
    fn test_get_post_nonexistent() {
        let (_db, _identity, _contacts, _perms, service, _peer_id) = create_test_env();

        let result = service.get_post("nonexistent-post-id").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_get_my_posts() {
        let (_db, _identity, _contacts, _perms, service, _peer_id) = create_test_env();

        // Create multiple posts
        service
            .create_post("text", Some("Post 1"), PostVisibility::Public)
            .unwrap();
        service
            .create_post("text", Some("Post 2"), PostVisibility::Contacts)
            .unwrap();
        service
            .create_post("text", Some("Post 3"), PostVisibility::Public)
            .unwrap();

        let posts = service.get_my_posts(10, None).unwrap();
        assert_eq!(posts.len(), 3);

        // Should be ordered by created_at DESC
        assert!(posts[0].created_at >= posts[1].created_at);
    }

    #[test]
    fn test_get_my_posts_with_limit() {
        let (_db, _identity, _contacts, _perms, service, _peer_id) = create_test_env();

        for i in 0..5 {
            service
                .create_post("text", Some(&format!("Post {}", i)), PostVisibility::Public)
                .unwrap();
        }

        let posts = service.get_my_posts(3, None).unwrap();
        assert_eq!(posts.len(), 3);
    }

    #[test]
    fn test_update_post() {
        let (_db, _identity, _contacts, _perms, service, _peer_id) = create_test_env();

        let created = service
            .create_post("text", Some("Original"), PostVisibility::Public)
            .unwrap();

        let updated = service
            .update_post(&created.post_id, Some("Updated content"))
            .unwrap();

        assert_eq!(updated.post_id, created.post_id);
        assert_eq!(updated.content_text, Some("Updated content".to_string()));
        assert!(updated.lamport_clock > created.lamport_clock);

        // Verify in DB
        let stored = service.get_post(&created.post_id).unwrap().unwrap();
        assert_eq!(stored.content_text, Some("Updated content".to_string()));
    }

    #[test]
    fn test_update_nonexistent_post() {
        let (_db, _identity, _contacts, _perms, service, _peer_id) = create_test_env();

        let result = service.update_post("nonexistent", Some("Updated"));
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_post() {
        let (_db, _identity, _contacts, _perms, service, _peer_id) = create_test_env();

        let created = service
            .create_post("text", Some("To delete"), PostVisibility::Public)
            .unwrap();

        let deleted = service.delete_post(&created.post_id).unwrap();

        assert_eq!(deleted.post_id, created.post_id);
        assert!(deleted.lamport_clock > created.lamport_clock);

        // Post should still exist but be soft-deleted
        let stored = service.get_post(&created.post_id).unwrap().unwrap();
        assert!(stored.deleted_at.is_some());

        // Should not appear in my posts list
        let my_posts = service.get_my_posts(10, None).unwrap();
        assert!(my_posts.is_empty());
    }

    #[test]
    fn test_delete_nonexistent_post() {
        let (_db, _identity, _contacts, _perms, service, _peer_id) = create_test_env();

        let result = service.delete_post("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_add_and_get_media() {
        let (_db, _identity, _contacts, _perms, service, _peer_id) = create_test_env();

        let created = service
            .create_post("text", Some("Post with media"), PostVisibility::Public)
            .unwrap();

        service
            .add_media_to_post(&AddMediaParams {
                post_id: &created.post_id,
                media_hash: "hash123",
                media_type: "image",
                mime_type: "image/jpeg",
                file_name: "photo.jpg",
                file_size: 12345,
                width: Some(800),
                height: Some(600),
                duration_seconds: None,
                sort_order: 0,
            })
            .unwrap();

        let media = service.get_post_media(&created.post_id).unwrap();
        assert_eq!(media.len(), 1);
        assert_eq!(media[0].media_hash, "hash123");
        assert_eq!(media[0].file_name, "photo.jpg");
        assert_eq!(media[0].width, Some(800));
    }

    #[test]
    fn test_add_media_to_nonexistent_post() {
        let (_db, _identity, _contacts, _perms, service, _peer_id) = create_test_env();

        let result = service.add_media_to_post(&AddMediaParams {
            post_id: "nonexistent",
            media_hash: "hash123",
            media_type: "image",
            mime_type: "image/jpeg",
            file_name: "photo.jpg",
            file_size: 12345,
            width: None,
            height: None,
            duration_seconds: None,
            sort_order: 0,
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_post_event_recorded() {
        let (_db, _identity, _contacts, _perms, service, _peer_id) = create_test_env();

        let created = service
            .create_post("text", Some("Event post"), PostVisibility::Public)
            .unwrap();

        // Verify the event was recorded by checking event_exists
        let event_id = format!("created:{}", created.post_id);
        let exists = PostsRepository::event_exists(service.db(), &event_id).unwrap();
        assert!(exists);
    }

    #[test]
    fn test_create_post_locked_identity_fails() {
        let (_db, identity_service, _contacts, _perms, service, _peer_id) = create_test_env();

        // Lock the identity
        identity_service.lock();

        let result = service.create_post("text", Some("Should fail"), PostVisibility::Public);
        assert!(result.is_err());
    }
}
