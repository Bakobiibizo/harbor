//! Posts service for managing wall/blog posts

use ed25519_dalek::VerifyingKey;
use std::sync::Arc;
use uuid::Uuid;

use crate::db::{
    Capability, Database, Post, PostData, PostMedia, PostMediaData, PostVisibility, PostsRepository,
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
            .ok_or_else(|| AppError::NotFound("No identity".to_string()))?;

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
            visibility: visibility,
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
            &event_id,
            "created",
            &post_id,
            &identity.peer_id,
            lamport_clock as i64,
            created_at,
            &payload_cbor,
            &signature,
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
            .ok_or_else(|| AppError::NotFound("No identity".to_string()))?;

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
            &event_id,
            "updated",
            post_id,
            &identity.peer_id,
            lamport_clock as i64,
            updated_at,
            &payload_cbor,
            &signature,
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
            .ok_or_else(|| AppError::NotFound("No identity".to_string()))?;

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
            &event_id,
            "deleted",
            post_id,
            &identity.peer_id,
            lamport_clock as i64,
            deleted_at,
            &payload_cbor,
            &signature,
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
    pub fn add_media_to_post(
        &self,
        post_id: &str,
        media_hash: &str,
        media_type: &str,
        mime_type: &str,
        file_name: &str,
        file_size: i64,
        width: Option<i32>,
        height: Option<i32>,
        duration_seconds: Option<i32>,
        sort_order: i32,
    ) -> Result<()> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::NotFound("No identity".to_string()))?;

        // Verify we own the post
        let post = PostsRepository::get_by_post_id(&self.db, post_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?
            .ok_or_else(|| AppError::NotFound("Post not found".to_string()))?;

        if post.author_peer_id != identity.peer_id {
            return Err(AppError::PermissionDenied(
                "Cannot add media to another user's post".to_string(),
            ));
        }

        let media_data = PostMediaData {
            post_id: post_id.to_string(),
            media_hash: media_hash.to_string(),
            media_type: media_type.to_string(),
            mime_type: mime_type.to_string(),
            file_name: file_name.to_string(),
            file_size,
            width,
            height,
            duration_seconds,
            sort_order,
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
            .ok_or_else(|| AppError::NotFound("No identity".to_string()))?;

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
            .ok_or_else(|| AppError::NotFound("No identity".to_string()))?;

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
    pub fn process_incoming_post(
        &self,
        post_id: &str,
        author_peer_id: &str,
        content_type: &str,
        content_text: Option<&str>,
        media_hashes: &[String],
        visibility: &str,
        lamport_clock: u64,
        created_at: i64,
        signature: &[u8],
    ) -> Result<()> {
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
            PostsRepository::insert_post(&self.db, &post_data)
                .map_err(|e| AppError::DatabaseString(e.to_string()))?;
        }

        // Record event
        let event_id = format!("received:{}:{}", post_id, lamport_clock);
        let payload_cbor = signable.signable_bytes()?;
        PostsRepository::record_post_event(
            &self.db,
            &event_id,
            "received",
            post_id,
            author_peer_id,
            lamport_clock as i64,
            created_at,
            &payload_cbor,
            signature,
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
            &event_id,
            "updated",
            post_id,
            author_peer_id,
            lamport_clock as i64,
            updated_at,
            &payload_cbor,
            signature,
        )
        .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        Ok(())
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
            &event_id,
            "deleted",
            post_id,
            author_peer_id,
            lamport_clock as i64,
            deleted_at,
            &payload_cbor,
            signature,
        )
        .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        Ok(())
    }
}
