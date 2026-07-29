//! Tauri commands for wall/blog posts

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;

use crate::db::repositories::{Post, PostMedia, PostVisibility};
use crate::error::AppError;
use crate::services::posts_service::{AddMediaParams, CreatePostMediaParams};
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
    pub relay_status: String,
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
            relay_status: post.relay_status,
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
    pub signature: Vec<u8>,
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
            signature: media.signature,
        }
    }
}

/// Create post result for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePostResult {
    pub post_id: String,
    pub created_at: i64,
    pub relay_status: String,
}

/// State of a committed local update/delete while durable relay delivery proceeds.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostMutationResult {
    pub post_id: String,
    pub relay_status: String,
}

/// Media metadata supplied while creating a post.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePostMediaInput {
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

pub(crate) fn parse_visibility_input(visibility: Option<&str>) -> Result<PostVisibility, AppError> {
    match visibility {
        None => Ok(PostVisibility::Contacts),
        Some("contacts") => Ok(PostVisibility::Contacts),
        Some("public") => Ok(PostVisibility::Public),
        Some(other) => Err(AppError::Validation(format!(
            "Invalid visibility '{}': must be 'public' or 'contacts'",
            other
        ))),
    }
}

/// Create a new post
#[tauri::command]
pub async fn create_post(
    posts_service: State<'_, Arc<PostsService>>,
    content_type: String,
    content_text: Option<String>,
    visibility: Option<String>,
    media: Option<Vec<CreatePostMediaInput>>,
) -> Result<CreatePostResult, AppError> {
    let vis = parse_visibility_input(visibility.as_deref())?;

    let media = media.unwrap_or_default();
    let media_params: Vec<CreatePostMediaParams<'_>> = media
        .iter()
        .map(|item| CreatePostMediaParams {
            media_hash: &item.media_hash,
            media_type: &item.media_type,
            mime_type: &item.mime_type,
            file_name: &item.file_name,
            file_size: item.file_size,
            width: item.width,
            height: item.height,
            duration_seconds: item.duration_seconds,
            sort_order: item.sort_order,
        })
        .collect();

    let outgoing = posts_service.create_post_with_media(
        &content_type,
        content_text.as_deref(),
        vis,
        &media_params,
    )?;

    Ok(CreatePostResult {
        post_id: outgoing.post_id.clone(),
        created_at: outgoing.created_at,
        relay_status: posts_service
            .get_post(&outgoing.post_id)?
            .map(|post| post.relay_status)
            .unwrap_or_else(|| "local_pending".to_string()),
    })
}

/// Update a post
#[tauri::command]
pub async fn update_post(
    posts_service: State<'_, Arc<PostsService>>,
    post_id: String,
    content_text: Option<String>,
) -> Result<PostMutationResult, AppError> {
    posts_service.update_post(&post_id, content_text.as_deref())?;
    let relay_status = posts_service
        .get_post(&post_id)?
        .map(|post| post.relay_status)
        .unwrap_or_else(|| "local_pending".to_string());
    Ok(PostMutationResult {
        post_id,
        relay_status,
    })
}

/// Delete a post
#[tauri::command]
pub async fn delete_post(
    posts_service: State<'_, Arc<PostsService>>,
    post_id: String,
) -> Result<PostMutationResult, AppError> {
    posts_service.delete_post(&post_id)?;
    let relay_status = posts_service
        .get_post(&post_id)?
        .map(|post| post.relay_status)
        .unwrap_or_else(|| "local_pending".to_string());
    Ok(PostMutationResult {
        post_id,
        relay_status,
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_visibility_input_accepts_supported_values_and_default() {
        assert_eq!(
            parse_visibility_input(None).unwrap(),
            PostVisibility::Contacts
        );
        assert_eq!(
            parse_visibility_input(Some("contacts")).unwrap(),
            PostVisibility::Contacts
        );
        assert_eq!(
            parse_visibility_input(Some("public")).unwrap(),
            PostVisibility::Public
        );
    }

    #[test]
    fn parse_visibility_input_rejects_unknown_values() {
        let result = parse_visibility_input(Some("friends"));
        assert!(
            matches!(result, Err(AppError::Validation(message)) if message.contains("Invalid visibility"))
        );
    }
}
