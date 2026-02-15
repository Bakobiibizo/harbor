//! Tauri commands for post comments

use crate::db::repositories::{CommentCount, CommentData, CommentsRepository, PostComment};
use crate::db::Database;
use crate::error::{AppError, Result};
use crate::services::IdentityService;
use std::sync::Arc;
use tauri::State;
use uuid::Uuid;

/// Add a comment to a post
#[tauri::command]
pub async fn add_comment(
    db: State<'_, Arc<Database>>,
    identity_service: State<'_, Arc<IdentityService>>,
    post_id: String,
    content: String,
) -> Result<PostComment> {
    // Validate content
    let content = content.trim().to_string();
    if content.is_empty() {
        return Err(AppError::Validation("Comment content cannot be empty".to_string()));
    }

    // Get current identity for author info
    let identity = identity_service
        .get_identity()?
        .ok_or_else(|| AppError::NotFound("No identity found".to_string()))?;

    let comment_id = Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().timestamp();

    let data = CommentData {
        comment_id: comment_id.clone(),
        post_id: post_id.clone(),
        author_peer_id: identity.peer_id.clone(),
        author_name: identity.display_name.clone(),
        content,
        created_at,
    };

    CommentsRepository::add_comment(&db, &data)
        .map_err(|e| AppError::DatabaseString(e.to_string()))?;

    // Return the created comment
    CommentsRepository::get_by_comment_id(&db, &comment_id)
        .map_err(|e| AppError::DatabaseString(e.to_string()))?
        .ok_or_else(|| AppError::Internal("Failed to retrieve created comment".to_string()))
}

/// Get comments for a post
#[tauri::command]
pub async fn get_comments(
    db: State<'_, Arc<Database>>,
    post_id: String,
) -> Result<Vec<PostComment>> {
    CommentsRepository::get_comments(&db, &post_id)
        .map_err(|e| AppError::DatabaseString(e.to_string()))
}

/// Delete a comment (only the author can delete their own comments)
#[tauri::command]
pub async fn delete_comment(
    db: State<'_, Arc<Database>>,
    identity_service: State<'_, Arc<IdentityService>>,
    comment_id: String,
) -> Result<bool> {
    // Get current identity
    let identity = identity_service
        .get_identity()?
        .ok_or_else(|| AppError::NotFound("No identity found".to_string()))?;

    // Check that the comment exists and belongs to the current user
    let comment = CommentsRepository::get_by_comment_id(&db, &comment_id)
        .map_err(|e| AppError::DatabaseString(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Comment not found".to_string()))?;

    if comment.author_peer_id != identity.peer_id {
        return Err(AppError::PermissionDenied(
            "You can only delete your own comments".to_string(),
        ));
    }

    CommentsRepository::delete_comment(&db, &comment_id)
        .map_err(|e| AppError::DatabaseString(e.to_string()))
}

/// Get comment counts for multiple posts (efficient batch query)
#[tauri::command]
pub async fn get_comment_counts(
    db: State<'_, Arc<Database>>,
    post_ids: Vec<String>,
) -> Result<Vec<CommentCount>> {
    CommentsRepository::get_comment_counts_batch(&db, &post_ids)
        .map_err(|e| AppError::DatabaseString(e.to_string()))
}
