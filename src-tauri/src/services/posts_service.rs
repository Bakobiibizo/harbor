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
    SignablePostDelete, SignablePostMedia, SignablePostUpdate, SignedPostMediaMetadata,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WallPostEventKind {
    Snapshot,
    Update,
    Delete,
}

impl WallPostEventKind {
    fn precedence(self) -> u8 {
        match self {
            Self::Snapshot => 0,
            Self::Update => 1,
            Self::Delete => 2,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Snapshot => "snapshot",
            Self::Update => "update",
            Self::Delete => "delete",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WallPostReconcileDecision {
    Apply,
    Ignore,
}

fn existing_post_event_kind(post: &Post) -> WallPostEventKind {
    if post.deleted_at.is_some() {
        WallPostEventKind::Delete
    } else if post.updated_at > post.created_at {
        WallPostEventKind::Update
    } else {
        WallPostEventKind::Snapshot
    }
}

fn reconcile_wall_post_event(
    existing: Option<&Post>,
    post_id: &str,
    author_peer_id: &str,
    incoming_kind: WallPostEventKind,
    incoming_lamport: u64,
    incoming_timestamp: i64,
) -> Result<WallPostReconcileDecision> {
    let Some(existing) = existing else {
        return Ok(WallPostReconcileDecision::Apply);
    };

    if existing.author_peer_id != author_peer_id {
        tracing::warn!(
            post_id = %post_id,
            existing_author = %existing.author_peer_id,
            incoming_author = %author_peer_id,
            event_type = incoming_kind.as_str(),
            "Rejected wall post event with mismatched author for existing object"
        );
        return Err(AppError::Validation(
            "Wall post event author does not match existing post author".to_string(),
        ));
    }

    let existing_kind = existing_post_event_kind(existing);
    let existing_lamport = existing.lamport_clock as u64;
    let existing_timestamp = existing.deleted_at.unwrap_or(existing.updated_at);

    // Tombstones are final for a post id: once a delete has been observed,
    // older or conflicting create/update snapshots must not resurrect the row.
    if existing_kind == WallPostEventKind::Delete && incoming_kind != WallPostEventKind::Delete {
        tracing::warn!(
            post_id = %post_id,
            author_peer_id = %author_peer_id,
            incoming_lamport,
            existing_lamport,
            incoming_timestamp,
            existing_timestamp,
            event_type = incoming_kind.as_str(),
            "Ignored wall post event because a tombstone is already stored"
        );
        return Ok(WallPostReconcileDecision::Ignore);
    }

    let should_apply = incoming_lamport > existing_lamport
        || (incoming_lamport == existing_lamport
            && (incoming_timestamp > existing_timestamp
                || (incoming_timestamp == existing_timestamp
                    && incoming_kind.precedence() > existing_kind.precedence())));

    if should_apply {
        Ok(WallPostReconcileDecision::Apply)
    } else {
        tracing::debug!(
            post_id = %post_id,
            author_peer_id = %author_peer_id,
            incoming_lamport,
            existing_lamport,
            incoming_timestamp,
            existing_timestamp,
            incoming_event_type = incoming_kind.as_str(),
            existing_event_type = existing_kind.as_str(),
            "Ignored stale or duplicate wall post event"
        );
        Ok(WallPostReconcileDecision::Ignore)
    }
}

/// Maximum size for a single wall media attachment (10 MiB).
pub const MAX_POST_MEDIA_BYTES: i64 = 10 * 1024 * 1024;

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
    pub media_items: Vec<SignedPostMediaMetadata>,
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

/// Parameters for media that is signed into a newly-created post.
pub struct CreatePostMediaParams<'a> {
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

fn validate_media_hash(media_hash: &str) -> Result<()> {
    if media_hash.len() != 64 || !media_hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(AppError::Validation(format!(
            "Invalid media hash '{}': expected 64 hex characters",
            media_hash
        )));
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

pub(crate) fn validate_signed_media_metadata(media: &SignedPostMediaMetadata) -> Result<()> {
    validate_media_hash(&media.media_hash)?;

    let expected_type = expected_media_type_for_mime(&media.mime_type).ok_or_else(|| {
        AppError::Validation(format!("Unsupported media MIME type: {}", media.mime_type))
    })?;

    if media.media_type != expected_type {
        return Err(AppError::Validation(format!(
            "Media type '{}' does not match MIME type '{}'",
            media.media_type, media.mime_type
        )));
    }

    if media.file_size <= 0 || media.file_size > MAX_POST_MEDIA_BYTES {
        return Err(AppError::Validation(format!(
            "Media '{}' is oversized or empty ({} bytes, max {} bytes)",
            media.file_name, media.file_size, MAX_POST_MEDIA_BYTES
        )));
    }

    if media.file_name.trim().is_empty() {
        return Err(AppError::Validation(
            "Media file name is required".to_string(),
        ));
    }

    if media.sort_order < 0 {
        return Err(AppError::Validation(format!(
            "Invalid media sort_order {}",
            media.sort_order
        )));
    }

    for (field, value) in [("width", media.width), ("height", media.height)] {
        if matches!(value, Some(v) if v <= 0) {
            return Err(AppError::Validation(format!(
                "Media {} must be positive when provided",
                field
            )));
        }
    }

    if matches!(media.duration_seconds, Some(duration) if duration <= 0) {
        return Err(AppError::Validation(
            "Media duration_seconds must be positive when provided".to_string(),
        ));
    }

    Ok(())
}

pub(crate) fn signable_media_from_signed(
    post_id: &str,
    author_peer_id: &str,
    media: &SignedPostMediaMetadata,
) -> SignablePostMedia {
    SignablePostMedia {
        post_id: post_id.to_string(),
        author_peer_id: author_peer_id.to_string(),
        media_hash: media.media_hash.clone(),
        media_type: media.media_type.clone(),
        mime_type: media.mime_type.clone(),
        file_name: media.file_name.clone(),
        file_size: media.file_size,
        width: media.width,
        height: media.height,
        duration_seconds: media.duration_seconds,
        sort_order: media.sort_order,
    }
}

pub(crate) fn sorted_media_hashes(media_items: &[SignedPostMediaMetadata]) -> Vec<String> {
    let mut sorted = media_items.to_vec();
    sorted.sort_by_key(|media| media.sort_order);
    sorted.into_iter().map(|media| media.media_hash).collect()
}

pub(crate) fn post_media_data_from_signed(
    post_id: &str,
    media: &SignedPostMediaMetadata,
) -> PostMediaData {
    PostMediaData {
        post_id: post_id.to_string(),
        media_hash: media.media_hash.clone(),
        media_type: media.media_type.clone(),
        mime_type: media.mime_type.clone(),
        file_name: media.file_name.clone(),
        file_size: media.file_size,
        width: media.width,
        height: media.height,
        duration_seconds: media.duration_seconds,
        sort_order: media.sort_order,
        signature: media.signature.clone(),
    }
}

impl PostsService {
    pub fn assert_can_publish(&self) -> Result<()> {
        crate::services::IdentityPublishingPolicy::enforce(&self.db, &self.identity_service)
    }
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

    /// Create a new post without media.
    pub fn create_post(
        &self,
        content_type: &str,
        content_text: Option<&str>,
        visibility: PostVisibility,
    ) -> Result<OutgoingPost> {
        self.create_post_with_media(content_type, content_text, visibility, &[])
    }

    /// Create a new post and bind ordered media hashes plus signed metadata to it.
    pub fn create_post_with_media(
        &self,
        content_type: &str,
        content_text: Option<&str>,
        visibility: PostVisibility,
        media: &[CreatePostMediaParams<'_>],
    ) -> Result<OutgoingPost> {
        crate::services::IdentityPublishingPolicy::enforce(&self.db, &self.identity_service)?;
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

        let mut signed_media_items = Vec::with_capacity(media.len());
        for item in media {
            let unsigned = SignedPostMediaMetadata {
                media_hash: item.media_hash.to_string(),
                media_type: item.media_type.to_string(),
                mime_type: item.mime_type.to_string(),
                file_name: item.file_name.to_string(),
                file_size: item.file_size,
                width: item.width,
                height: item.height,
                duration_seconds: item.duration_seconds,
                sort_order: item.sort_order,
                signature: Vec::new(),
            };
            validate_signed_media_metadata(&unsigned)?;

            let signable_media = signable_media_from_signed(&post_id, &identity.peer_id, &unsigned);
            let media_signature = self.identity_service.sign(&signable_media)?;
            signed_media_items.push(SignedPostMediaMetadata {
                signature: media_signature,
                ..unsigned
            });
        }

        let media_hashes = sorted_media_hashes(&signed_media_items);

        // Create signable. If media is part of the post, media_hashes is non-empty
        // before the post signature is produced.
        let signable = SignablePost {
            post_id: post_id.clone(),
            author_peer_id: identity.peer_id.clone(),
            content_type: content_type.to_string(),
            content_text: content_text.map(String::from),
            media_hashes: media_hashes.clone(),
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

        for media_item in &signed_media_items {
            let media_data = post_media_data_from_signed(&post_id, media_item);
            PostsRepository::add_media(&self.db, &media_data)
                .map_err(|e| AppError::DatabaseString(e.to_string()))?;
        }

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
            media_hashes,
            media_items: signed_media_items,
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
        crate::services::IdentityPublishingPolicy::enforce(&self.db, &self.identity_service)?;
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

        // Create and sign the update event.
        let signable = SignablePostUpdate {
            post_id: post_id.to_string(),
            author_peer_id: identity.peer_id.clone(),
            content_text: content_text.map(String::from),
            lamport_clock,
            updated_at,
        };

        let signature = self.identity_service.sign(&signable)?;

        // Also materialize a current-state post signature so relay snapshots and
        // direct fetches can be verified by consumers that did not see the
        // original create event in this session.
        let media_hashes = PostsRepository::get_media_hashes(&self.db, post_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;
        let current_post_signable = SignablePost {
            post_id: post_id.to_string(),
            author_peer_id: identity.peer_id.clone(),
            content_type: post.content_type.clone(),
            content_text: content_text.map(String::from),
            media_hashes,
            visibility: post.visibility.to_string(),
            lamport_clock,
            created_at: post.created_at,
        };
        let current_post_signature = self.identity_service.sign(&current_post_signable)?;

        // Update locally with the current-state signature.
        PostsRepository::update_post_with_signature(
            &self.db,
            post_id,
            content_text,
            updated_at,
            lamport_clock as i64,
            &current_post_signature,
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
        crate::services::IdentityPublishingPolicy::enforce(&self.db, &self.identity_service)?;
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

        // Delete locally and retain the tombstone lamport/signature so stale
        // creates or updates cannot resurrect the post.
        PostsRepository::delete_post_with_tombstone(
            &self.db,
            post_id,
            deleted_at,
            lamport_clock as i64,
            &signature,
        )
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

        let unsigned = SignedPostMediaMetadata {
            media_hash: params.media_hash.to_string(),
            media_type: params.media_type.to_string(),
            mime_type: params.mime_type.to_string(),
            file_name: params.file_name.to_string(),
            file_size: params.file_size,
            width: params.width,
            height: params.height,
            duration_seconds: params.duration_seconds,
            sort_order: params.sort_order,
            signature: Vec::new(),
        };
        validate_signed_media_metadata(&unsigned)?;

        let signable_media =
            signable_media_from_signed(params.post_id, &identity.peer_id, &unsigned);
        let signed_media = SignedPostMediaMetadata {
            signature: self.identity_service.sign(&signable_media)?,
            ..unsigned
        };

        let existing_media = PostsRepository::get_post_media(&self.db, params.post_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;
        if existing_media
            .iter()
            .any(|media| media.media_hash == signed_media.media_hash)
        {
            return Ok(());
        }

        let mut media_for_signature: Vec<SignedPostMediaMetadata> = existing_media
            .into_iter()
            .map(|media| SignedPostMediaMetadata {
                media_hash: media.media_hash,
                media_type: media.media_type,
                mime_type: media.mime_type,
                file_name: media.file_name,
                file_size: media.file_size,
                width: media.width,
                height: media.height,
                duration_seconds: media.duration_seconds,
                sort_order: media.sort_order,
                signature: media.signature,
            })
            .collect();
        media_for_signature.push(signed_media.clone());
        let media_hashes = sorted_media_hashes(&media_for_signature);

        let signable_post = SignablePost {
            post_id: post.post_id.clone(),
            author_peer_id: post.author_peer_id.clone(),
            content_type: post.content_type.clone(),
            content_text: post.content_text.clone(),
            media_hashes,
            visibility: post.visibility.to_string(),
            lamport_clock: post.lamport_clock as u64,
            created_at: post.created_at,
        };
        let verifying_key = VerifyingKey::from_bytes(
            identity
                .public_key
                .as_slice()
                .try_into()
                .map_err(|_| AppError::Crypto("Invalid public key length".to_string()))?,
        )
        .map_err(|e| AppError::Crypto(format!("Invalid public key: {}", e)))?;
        if !verify(&verifying_key, &signable_post, &post.signature)? {
            return Err(AppError::Validation(
                "Cannot attach media that was not included in the post signature".to_string(),
            ));
        }

        let media_data = post_media_data_from_signed(params.post_id, &signed_media);

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

        let visibility_filter = if author_peer_id == identity.peer_id
            || self
                .permissions_service
                .we_have_capability(author_peer_id, Capability::WallRead)?
        {
            None
        } else {
            Some(PostVisibility::Public)
        };

        PostsRepository::get_by_author_with_visibility(
            &self.db,
            author_peer_id,
            visibility_filter,
            limit,
            before_timestamp,
        )
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

        let existing = PostsRepository::get_by_post_id(&self.db, post_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;
        if reconcile_wall_post_event(
            existing.as_ref(),
            post_id,
            author_peer_id,
            WallPostEventKind::Snapshot,
            lamport_clock,
            created_at,
        )? == WallPostReconcileDecision::Ignore
        {
            return Ok(());
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
        if existing.is_some() {
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

        // Check we have the post and reconcile against the materialized state.
        let existing = PostsRepository::get_by_post_id(&self.db, post_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?
            .ok_or_else(|| AppError::NotFound("Post not found".to_string()))?;

        if reconcile_wall_post_event(
            Some(&existing),
            post_id,
            author_peer_id,
            WallPostEventKind::Update,
            lamport_clock,
            updated_at,
        )? == WallPostReconcileDecision::Ignore
        {
            return Ok(());
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

        // Reconcile against an existing post or tombstone.
        let existing = PostsRepository::get_by_post_id(&self.db, post_id)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        if reconcile_wall_post_event(
            existing.as_ref(),
            post_id,
            author_peer_id,
            WallPostEventKind::Delete,
            lamport_clock,
            deleted_at,
        )? == WallPostReconcileDecision::Ignore
        {
            return Ok(());
        }

        // Update lamport clock
        self.db
            .update_lamport_clock(author_peer_id, lamport_clock as i64)
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        // Delete post or persist a tombstone if this peer learned about the
        // delete before it ever saw the create/update snapshot.
        if PostsRepository::delete_post_with_tombstone(
            &self.db,
            post_id,
            deleted_at,
            lamport_clock as i64,
            signature,
        )
        .map_err(|e| AppError::DatabaseString(e.to_string()))?
        {
            // existing row tombstoned
        } else {
            PostsRepository::insert_remote_tombstone(
                &self.db,
                post_id,
                author_peer_id,
                lamport_clock as i64,
                deleted_at,
                signature,
            )
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;
        }

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
    use crate::services::{sign, ContactsService, PermissionsService};
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;
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
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO identity_migration_state(peer_id, mode, updated_at) VALUES(?, 'compatibility', 1)",
                [&peer_id],
            )
            .map(|_| ())
        })
        .unwrap();

        (
            db,
            identity_service,
            contacts_service,
            permissions_service,
            posts_service,
            peer_id,
        )
    }

    fn add_remote_author(contacts: &ContactsService, peer_id: &str) -> SigningKey {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        contacts
            .add_contact(
                peer_id,
                signing_key.verifying_key().as_bytes(),
                &[7u8; 32],
                "Remote Author",
                None,
                None,
            )
            .unwrap();
        signing_key
    }

    fn signed_remote_post(
        signing_key: &SigningKey,
        post_id: &str,
        author_peer_id: &str,
        content_text: &str,
        lamport_clock: u64,
        created_at: i64,
    ) -> (IncomingPostParams<'static>, Vec<String>, Vec<u8>) {
        let media_hashes = Vec::new();
        let signable = SignablePost {
            post_id: post_id.to_string(),
            author_peer_id: author_peer_id.to_string(),
            content_type: "text".to_string(),
            content_text: Some(content_text.to_string()),
            media_hashes: media_hashes.clone(),
            visibility: "public".to_string(),
            lamport_clock,
            created_at,
        };
        let signature = sign(signing_key, &signable).unwrap();
        let params = IncomingPostParams {
            post_id: Box::leak(post_id.to_string().into_boxed_str()),
            author_peer_id: Box::leak(author_peer_id.to_string().into_boxed_str()),
            content_type: "text",
            content_text: Some(Box::leak(content_text.to_string().into_boxed_str())),
            media_hashes: Box::leak(Box::new(media_hashes.clone())),
            visibility: "public",
            lamport_clock,
            created_at,
            signature: Box::leak(signature.clone().into_boxed_slice()),
        };
        (params, media_hashes, signature)
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
    fn test_get_posts_by_author_without_wall_read_returns_public_only() {
        let (db, _identity, _contacts, _perms, service, _peer_id) = create_test_env();
        let remote_peer = "12D3KooWRemoteWall";

        PostsRepository::insert_remote_post(
            &db,
            &PostData {
                post_id: "remote-public".to_string(),
                author_peer_id: remote_peer.to_string(),
                content_type: "text".to_string(),
                content_text: Some("Public".to_string()),
                visibility: PostVisibility::Public,
                lamport_clock: 1,
                created_at: 1000,
                signature: vec![0u8; 64],
            },
        )
        .unwrap();
        PostsRepository::insert_remote_post(
            &db,
            &PostData {
                post_id: "remote-contacts".to_string(),
                author_peer_id: remote_peer.to_string(),
                content_type: "text".to_string(),
                content_text: Some("Contacts".to_string()),
                visibility: PostVisibility::Contacts,
                lamport_clock: 2,
                created_at: 2000,
                signature: vec![0u8; 64],
            },
        )
        .unwrap();

        let posts = service.get_posts_by_author(remote_peer, 10, None).unwrap();
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].post_id, "remote-public");
    }

    #[test]
    fn test_update_post() {
        let (_db, identity, _contacts, _perms, service, _peer_id) = create_test_env();

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
        assert_eq!(stored.lamport_clock, updated.lamport_clock as i64);

        // The materialized post signature is a current-state signature for
        // relay/direct-sync snapshots, not the update-event signature.
        let identity_info = identity.get_identity().unwrap().unwrap();
        let verifying_key =
            VerifyingKey::from_bytes(identity_info.public_key.as_slice().try_into().unwrap())
                .unwrap();
        let signable = SignablePost {
            post_id: stored.post_id.clone(),
            author_peer_id: stored.author_peer_id.clone(),
            content_type: stored.content_type.clone(),
            content_text: stored.content_text.clone(),
            media_hashes: Vec::new(),
            visibility: stored.visibility.to_string(),
            lamport_clock: stored.lamport_clock as u64,
            created_at: stored.created_at,
        };
        assert!(verify(&verifying_key, &signable, &stored.signature).unwrap());
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

        // Post should still exist but be soft-deleted with tombstone state.
        let stored = service.get_post(&created.post_id).unwrap().unwrap();
        assert!(stored.deleted_at.is_some());
        assert_eq!(stored.lamport_clock, deleted.lamport_clock as i64);
        assert_eq!(stored.signature, deleted.signature);

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
    fn test_incoming_delete_tombstone_blocks_stale_and_newer_snapshots() {
        let (db, _identity, contacts, _perms, service, _peer_id) = create_test_env();
        let author = "remote-author-delete";
        let signing_key = add_remote_author(&contacts, author);
        let post_id = "remote-post-delete";

        let create = signed_remote_post(&signing_key, post_id, author, "original", 1, 1000).0;
        service.process_incoming_post(&create).unwrap();

        let delete_signable = SignablePostDelete {
            post_id: post_id.to_string(),
            author_peer_id: author.to_string(),
            lamport_clock: 2,
            deleted_at: 2000,
        };
        let delete_signature = sign(&signing_key, &delete_signable).unwrap();
        service
            .process_incoming_post_delete(post_id, author, 2, 2000, &delete_signature)
            .unwrap();

        let stale = signed_remote_post(&signing_key, post_id, author, "stale", 1, 1000).0;
        service.process_incoming_post(&stale).unwrap();
        let newer_snapshot =
            signed_remote_post(&signing_key, post_id, author, "resurrect", 3, 3000).0;
        service.process_incoming_post(&newer_snapshot).unwrap();

        let stored = PostsRepository::get_by_post_id(&db, post_id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.deleted_at, Some(2000));
        assert_eq!(stored.lamport_clock, 2);
        assert_eq!(stored.content_text, Some("original".to_string()));
    }

    #[test]
    fn test_incoming_delete_same_lamport_wins_by_event_precedence() {
        let (db, _identity, contacts, _perms, service, _peer_id) = create_test_env();
        let author = "remote-author-same-lamport";
        let signing_key = add_remote_author(&contacts, author);
        let post_id = "remote-post-same-lamport";

        let create = signed_remote_post(&signing_key, post_id, author, "original", 1, 1000).0;
        service.process_incoming_post(&create).unwrap();

        let update_signable = SignablePostUpdate {
            post_id: post_id.to_string(),
            author_peer_id: author.to_string(),
            content_text: Some("updated".to_string()),
            lamport_clock: 2,
            updated_at: 2000,
        };
        let update_signature = sign(&signing_key, &update_signable).unwrap();
        service
            .process_incoming_post_update(
                post_id,
                author,
                Some("updated"),
                2,
                2000,
                &update_signature,
            )
            .unwrap();

        let delete_signable = SignablePostDelete {
            post_id: post_id.to_string(),
            author_peer_id: author.to_string(),
            lamport_clock: 2,
            deleted_at: 2000,
        };
        let delete_signature = sign(&signing_key, &delete_signable).unwrap();
        service
            .process_incoming_post_delete(post_id, author, 2, 2000, &delete_signature)
            .unwrap();

        let stored = PostsRepository::get_by_post_id(&db, post_id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.deleted_at, Some(2000));
        assert_eq!(stored.lamport_clock, 2);
    }

    #[test]
    fn test_incoming_delete_before_create_preserves_remote_tombstone() {
        let (db, _identity, contacts, _perms, service, _peer_id) = create_test_env();
        let author = "remote-author-offline-delete";
        let signing_key = add_remote_author(&contacts, author);
        let post_id = "remote-post-offline-delete";

        let delete_signable = SignablePostDelete {
            post_id: post_id.to_string(),
            author_peer_id: author.to_string(),
            lamport_clock: 5,
            deleted_at: 5000,
        };
        let delete_signature = sign(&signing_key, &delete_signable).unwrap();
        service
            .process_incoming_post_delete(post_id, author, 5, 5000, &delete_signature)
            .unwrap();

        let older_create = signed_remote_post(&signing_key, post_id, author, "old", 1, 1000).0;
        service.process_incoming_post(&older_create).unwrap();

        let stored = PostsRepository::get_by_post_id(&db, post_id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.deleted_at, Some(5000));
        assert_eq!(stored.lamport_clock, 5);
        assert_eq!(stored.content_type, "deleted");
    }

    #[test]
    fn test_incoming_duplicate_and_forged_author_are_not_applied() {
        let (db, _identity, contacts, _perms, service, _peer_id) = create_test_env();
        let author = "remote-author-forgery";
        let signing_key = add_remote_author(&contacts, author);
        let post_id = "remote-post-forgery";

        let create = signed_remote_post(&signing_key, post_id, author, "original", 1, 1000).0;
        service.process_incoming_post(&create).unwrap();
        let duplicate = signed_remote_post(&signing_key, post_id, author, "duplicate", 1, 1000).0;
        service.process_incoming_post(&duplicate).unwrap();
        assert_eq!(
            PostsRepository::get_by_post_id(&db, post_id)
                .unwrap()
                .unwrap()
                .content_text,
            Some("original".to_string())
        );

        let forged_author = "remote-author-forged";
        let forged_key = add_remote_author(&contacts, forged_author);
        let forged = signed_remote_post(&forged_key, post_id, forged_author, "forged", 2, 2000).0;
        let result = service.process_incoming_post(&forged);
        assert!(result.is_err());
        assert_eq!(
            PostsRepository::get_by_post_id(&db, post_id)
                .unwrap()
                .unwrap()
                .content_text,
            Some("original".to_string())
        );
    }

    #[test]
    fn test_incoming_invalid_signature_is_rejected() {
        let (db, _identity, contacts, _perms, service, _peer_id) = create_test_env();
        let author = "remote-author-invalid-signature";
        let signing_key = add_remote_author(&contacts, author);
        let post_id = "remote-post-invalid-signature";
        let mut create = signed_remote_post(&signing_key, post_id, author, "original", 1, 1000).0;
        create.signature = &[0u8; 64];

        let result = service.process_incoming_post(&create);
        assert!(result.is_err());
        assert!(PostsRepository::get_by_post_id(&db, post_id)
            .unwrap()
            .is_none());
    }

    #[test]
    fn test_create_post_with_signed_media() {
        let (_db, _identity, _contacts, _perms, service, _peer_id) = create_test_env();
        let media_hash = "a".repeat(64);

        let created = service
            .create_post_with_media(
                "image",
                Some("Post with media"),
                PostVisibility::Public,
                &[CreatePostMediaParams {
                    media_hash: &media_hash,
                    media_type: "image",
                    mime_type: "image/jpeg",
                    file_name: "photo.jpg",
                    file_size: 12345,
                    width: Some(800),
                    height: Some(600),
                    duration_seconds: None,
                    sort_order: 0,
                }],
            )
            .unwrap();

        assert_eq!(created.media_hashes, vec![media_hash.clone()]);
        assert_eq!(created.media_items.len(), 1);
        assert!(!created.media_items[0].signature.is_empty());

        let media = service.get_post_media(&created.post_id).unwrap();
        assert_eq!(media.len(), 1);
        assert_eq!(media[0].media_hash, media_hash);
        assert_eq!(media[0].file_name, "photo.jpg");
        assert_eq!(media[0].width, Some(800));
        assert!(!media[0].signature.is_empty());
    }

    #[test]
    fn test_create_post_with_media_rejects_unsupported_and_oversized_media() {
        let (_db, _identity, _contacts, _perms, service, _peer_id) = create_test_env();
        let media_hash = "c".repeat(64);

        let unsupported = service.create_post_with_media(
            "image",
            Some("bad mime"),
            PostVisibility::Public,
            &[CreatePostMediaParams {
                media_hash: &media_hash,
                media_type: "image",
                mime_type: "application/octet-stream",
                file_name: "bad.bin",
                file_size: 123,
                width: None,
                height: None,
                duration_seconds: None,
                sort_order: 0,
            }],
        );
        assert!(unsupported.is_err());

        let oversized = service.create_post_with_media(
            "video",
            Some("too large"),
            PostVisibility::Public,
            &[CreatePostMediaParams {
                media_hash: &media_hash,
                media_type: "video",
                mime_type: "video/mp4",
                file_name: "large.mp4",
                file_size: MAX_POST_MEDIA_BYTES + 1,
                width: Some(640),
                height: Some(480),
                duration_seconds: Some(5),
                sort_order: 0,
            }],
        );
        assert!(oversized.is_err());
    }

    #[test]
    fn test_add_media_rejects_when_post_signature_did_not_include_hash() {
        let (_db, _identity, _contacts, _perms, service, _peer_id) = create_test_env();
        let created = service
            .create_post("text", Some("Unsigned media later"), PostVisibility::Public)
            .unwrap();
        let media_hash = "b".repeat(64);

        let result = service.add_media_to_post(&AddMediaParams {
            post_id: &created.post_id,
            media_hash: &media_hash,
            media_type: "image",
            mime_type: "image/jpeg",
            file_name: "photo.jpg",
            file_size: 12345,
            width: Some(800),
            height: Some(600),
            duration_seconds: None,
            sort_order: 0,
        });

        assert!(result.is_err());
        assert!(service.get_post_media(&created.post_id).unwrap().is_empty());
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
