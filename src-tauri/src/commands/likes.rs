//! Tauri commands for post likes/reactions

use crate::db::repositories::{LikeSummary, LikesRepository};
use crate::db::Database;
use crate::error::{AppError, Result};
use crate::services::{IdentityService, WallSocialService};
use std::sync::Arc;
use tauri::State;

/// Like a post
#[tauri::command]
pub async fn like_post(
    db: State<'_, Arc<Database>>,
    identity_service: State<'_, Arc<IdentityService>>,
    wall_social_service: State<'_, Arc<WallSocialService>>,
    post_id: String,
) -> Result<LikeSummary> {
    let identity = identity_service
        .get_identity()?
        .ok_or_else(|| AppError::IdentityNotFound("No identity found".to_string()))?;
    wall_social_service.add_reaction(&post_id, "like")?;
    LikesRepository::get_like_summary(&db, &post_id, &identity.peer_id)
        .map_err(|e| AppError::DatabaseString(e.to_string()))
}

/// Unlike a post
#[tauri::command]
pub async fn unlike_post(
    db: State<'_, Arc<Database>>,
    identity_service: State<'_, Arc<IdentityService>>,
    wall_social_service: State<'_, Arc<WallSocialService>>,
    post_id: String,
) -> Result<LikeSummary> {
    let identity = identity_service
        .get_identity()?
        .ok_or_else(|| AppError::IdentityNotFound("No identity found".to_string()))?;
    wall_social_service.remove_reaction(&post_id, "like")?;
    LikesRepository::get_like_summary(&db, &post_id, &identity.peer_id)
        .map_err(|e| AppError::DatabaseString(e.to_string()))
}

/// Get like summary for a single post
#[tauri::command]
pub async fn get_post_likes(
    db: State<'_, Arc<Database>>,
    identity_service: State<'_, Arc<IdentityService>>,
    post_id: String,
) -> Result<LikeSummary> {
    // Get current identity (or use empty string for non-logged-in users)
    let current_peer_id = identity_service
        .get_identity()?
        .map(|i| i.peer_id)
        .unwrap_or_default();

    LikesRepository::get_like_summary(&db, &post_id, &current_peer_id)
        .map_err(|e| AppError::DatabaseString(e.to_string()))
}

/// Get like summaries for multiple posts (efficient batch query)
#[tauri::command]
pub async fn get_posts_likes_batch(
    db: State<'_, Arc<Database>>,
    identity_service: State<'_, Arc<IdentityService>>,
    post_ids: Vec<String>,
) -> Result<Vec<LikeSummary>> {
    // Get current identity (or use empty string for non-logged-in users)
    let current_peer_id = identity_service
        .get_identity()?
        .map(|i| i.peer_id)
        .unwrap_or_default();

    LikesRepository::get_like_summaries_batch(&db, &post_ids, &current_peer_id)
        .map_err(|e| AppError::DatabaseString(e.to_string()))
}

/// Get all posts that the current user has liked
#[tauri::command]
pub async fn get_my_liked_posts(
    db: State<'_, Arc<Database>>,
    identity_service: State<'_, Arc<IdentityService>>,
) -> Result<Vec<String>> {
    // Get current identity
    let identity = identity_service
        .get_identity()?
        .ok_or_else(|| AppError::IdentityNotFound("No identity found".to_string()))?;

    LikesRepository::get_liked_posts(&db, &identity.peer_id)
        .map_err(|e| AppError::DatabaseString(e.to_string()))
}
