//! Tauri commands for post comments

use crate::db::repositories::{CommentCount, CommentsRepository, PostComment, WallSocialEvent};
use crate::db::Database;
use crate::error::{AppError, Result};
use crate::services::WallSocialService;
use std::sync::Arc;
use tauri::State;

/// Add a signed syncable comment to a post.
#[tauri::command]
pub async fn add_comment(
    wall_social_service: State<'_, Arc<WallSocialService>>,
    post_id: String,
    content: String,
) -> Result<PostComment> {
    wall_social_service.add_comment(&post_id, &content)
}

/// Get comments for a post.
#[tauri::command]
pub async fn get_comments(
    db: State<'_, Arc<Database>>,
    post_id: String,
) -> Result<Vec<PostComment>> {
    CommentsRepository::get_comments(&db, &post_id)
        .map_err(|e| AppError::DatabaseString(e.to_string()))
}

/// Delete a comment by emitting a signed syncable delete event.
#[tauri::command]
pub async fn delete_comment(
    wall_social_service: State<'_, Arc<WallSocialService>>,
    comment_id: String,
) -> Result<bool> {
    wall_social_service.delete_comment(&comment_id)
}

/// Get comment counts for multiple posts (efficient batch query).
#[tauri::command]
pub async fn get_comment_counts(
    db: State<'_, Arc<Database>>,
    post_ids: Vec<String>,
) -> Result<Vec<CommentCount>> {
    CommentsRepository::get_comment_counts_batch(&db, &post_ids)
        .map_err(|e| AppError::DatabaseString(e.to_string()))
}

/// Get signed social events for a post. Used by sync callers and frontend tests.
#[tauri::command]
pub async fn get_wall_social_events(
    wall_social_service: State<'_, Arc<WallSocialService>>,
    post_id: String,
) -> Result<Vec<WallSocialEvent>> {
    wall_social_service.list_events_for_post(&post_id)
}
