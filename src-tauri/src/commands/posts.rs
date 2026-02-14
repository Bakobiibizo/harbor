//! Tauri commands for wall/blog posts

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;

use crate::db::repositories::{Post, PostMedia, PostVisibility};
use crate::error::AppError;
use crate::services::posts_service::AddMediaParams;
use crate::services::PostsService;

/// Post info for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostInfo {
    pub post_id: String,
    pub author_peer_id: String,
    pub content_type: String,
    pub content_text: Option<String>,
    pub visibility: String,
    pub lamport_clock: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
    pub is_local: bool,
}

impl From<Post> for PostInfo {
    fn from(post: Post) -> Self {
        Self {
            post_id: post.post_id,
            author_peer_id: post.author_peer_id,
            content_type: post.content_type,
            content_text: post.content_text,
            visibility: post.visibility.as_str().to_string(),
            lamport_clock: post.lamport_clock,
            created_at: post.created_at,
            updated_at: post.updated_at,
            deleted_at: post.deleted_at,
            is_local: post.is_local,
        }
    }
}

/// Post media info for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostMediaInfo {
    pub id: i64,
    pub post_id: String,
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

impl From<PostMedia> for PostMediaInfo {
    fn from(media: PostMedia) -> Self {
        Self {
            id: media.id,
            post_id: media.post_id,
            media_hash: media.media_hash,
            media_type: media.media_type,
            mime_type: media.mime_type,
            file_name: media.file_name,
            file_size: media.file_size,
            width: media.width,
            height: media.height,
            duration_seconds: media.duration_seconds,
            sort_order: media.sort_order,
        }
    }
}

/// Create post result for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePostResult {
    pub post_id: String,
    pub created_at: i64,
}

/// Create a new post
#[tauri::command]
pub async fn create_post(
    posts_service: State<'_, Arc<PostsService>>,
    network_state: State<'_, crate::commands::NetworkState>,
    content_type: String,
    content_text: Option<String>,
    visibility: Option<String>,
) -> Result<CreatePostResult, AppError> {
    let vis = match visibility.as_deref() {
        Some("public") => PostVisibility::Public,
        _ => PostVisibility::Contacts, // Default to contacts-only
    };

    let outgoing = posts_service.create_post(&content_type, content_text.as_deref(), vis)?;

    // Auto-sync: submit the new post to the relay in the background.
    // We don't fail the command if relay submission fails -- the user can
    // always manually sync later via sync_wall_to_relay.
    if let Ok(handle) = network_state.get_handle().await {
        if let Ok(stats) = handle.get_stats().await {
            if let Ok(relay_peer_id) =
                crate::commands::wall_sync::find_relay_peer_id(&stats.relay_addresses)
            {
                let post_id = outgoing.post_id.clone();
                let ct = outgoing.content_type.clone();
                let ct_text = outgoing.content_text.clone();
                let vis_str = outgoing.visibility.clone();
                let lc = outgoing.lamport_clock as i64;
                let ca = outgoing.created_at;
                let sig = outgoing.signature.clone();
                // Fire and forget -- don't block post creation on relay submission
                tokio::spawn(async move {
                    if let Err(e) = handle
                        .submit_wall_post_to_relay(
                            relay_peer_id,
                            post_id.clone(),
                            ct,
                            ct_text,
                            vis_str,
                            lc,
                            ca,
                            sig,
                        )
                        .await
                    {
                        tracing::warn!(
                            "Failed to auto-sync wall post {} to relay: {}",
                            post_id,
                            e
                        );
                    }
                });
            }
        }
    }

    Ok(CreatePostResult {
        post_id: outgoing.post_id,
        created_at: outgoing.created_at,
    })
}

/// Update a post
#[tauri::command]
pub async fn update_post(
    posts_service: State<'_, Arc<PostsService>>,
    post_id: String,
    content_text: Option<String>,
) -> Result<(), AppError> {
    posts_service.update_post(&post_id, content_text.as_deref())?;
    Ok(())
}

/// Delete a post
#[tauri::command]
pub async fn delete_post(
    posts_service: State<'_, Arc<PostsService>>,
    network_state: State<'_, crate::commands::NetworkState>,
    post_id: String,
) -> Result<(), AppError> {
    posts_service.delete_post(&post_id)?;

    // Auto-sync: delete the post on the relay in the background
    if let Ok(handle) = network_state.get_handle().await {
        if let Ok(stats) = handle.get_stats().await {
            if let Ok(relay_peer_id) =
                crate::commands::wall_sync::find_relay_peer_id(&stats.relay_addresses)
            {
                let pid = post_id.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle.delete_wall_post_on_relay(relay_peer_id, pid.clone()).await
                    {
                        tracing::warn!(
                            "Failed to auto-delete wall post {} on relay: {}",
                            pid,
                            e
                        );
                    }
                });
            }
        }
    }

    Ok(())
}

/// Get a single post by ID
#[tauri::command]
pub async fn get_post(
    posts_service: State<'_, Arc<PostsService>>,
    post_id: String,
) -> Result<Option<PostInfo>, AppError> {
    let post = posts_service.get_post(&post_id)?;
    Ok(post.map(PostInfo::from))
}

/// Get the local user's posts (their wall)
#[tauri::command]
pub async fn get_my_posts(
    posts_service: State<'_, Arc<PostsService>>,
    limit: Option<i64>,
    before_timestamp: Option<i64>,
) -> Result<Vec<PostInfo>, AppError> {
    let limit = limit.unwrap_or(50);
    let posts = posts_service.get_my_posts(limit, before_timestamp)?;
    Ok(posts.into_iter().map(PostInfo::from).collect())
}

/// Get posts by a specific author
#[tauri::command]
pub async fn get_posts_by_author(
    posts_service: State<'_, Arc<PostsService>>,
    author_peer_id: String,
    limit: Option<i64>,
    before_timestamp: Option<i64>,
) -> Result<Vec<PostInfo>, AppError> {
    let limit = limit.unwrap_or(50);
    let posts = posts_service.get_posts_by_author(&author_peer_id, limit, before_timestamp)?;
    Ok(posts.into_iter().map(PostInfo::from).collect())
}

/// Parameters for adding media to a post
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddPostMediaParams {
    pub post_id: String,
    pub media_hash: String,
    pub media_type: String,
    pub mime_type: String,
    pub file_name: String,
    pub file_size: i64,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub duration_seconds: Option<i32>,
    pub sort_order: Option<i32>,
}

/// Add media to a post
#[tauri::command]
pub async fn add_post_media(
    posts_service: State<'_, Arc<PostsService>>,
    params: AddPostMediaParams,
) -> Result<(), AppError> {
    posts_service.add_media_to_post(&AddMediaParams {
        post_id: &params.post_id,
        media_hash: &params.media_hash,
        media_type: &params.media_type,
        mime_type: &params.mime_type,
        file_name: &params.file_name,
        file_size: params.file_size,
        width: params.width,
        height: params.height,
        duration_seconds: params.duration_seconds,
        sort_order: params.sort_order.unwrap_or(0),
    })
}

/// Get media for a post
#[tauri::command]
pub async fn get_post_media(
    posts_service: State<'_, Arc<PostsService>>,
    post_id: String,
) -> Result<Vec<PostMediaInfo>, AppError> {
    let media = posts_service.get_post_media(&post_id)?;
    Ok(media.into_iter().map(PostMediaInfo::from).collect())
}
