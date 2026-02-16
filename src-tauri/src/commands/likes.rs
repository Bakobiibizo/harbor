//! Tauri commands for post likes/reactions

use crate::db::repositories::{LikeData, LikeSummary, LikesRepository};
use crate::db::Database;
use crate::error::{AppError, Result};
use crate::services::signing::SignablePostLike;
use crate::services::IdentityService;
use std::sync::Arc;
use tauri::State;

/// Like a post
#[tauri::command]
pub async fn like_post(
    db: State<'_, Arc<Database>>,
    identity_service: State<'_, Arc<IdentityService>>,
    post_id: String,
) -> Result<LikeSummary> {
    // Get current identity
    let identity = identity_service
        .get_identity()?
        .ok_or_else(|| AppError::IdentityNotFound("No identity found".to_string()))?;

    // Create signable data for the like
    let timestamp = chrono::Utc::now().timestamp();
    let signable = SignablePostLike {
        post_id: post_id.clone(),
        liker_peer_id: identity.peer_id.clone(),
        reaction_type: "like".to_string(),
        timestamp,
    };

    // Sign the like
    let signature = identity_service.sign(&signable)?;

    let data = LikeData {
        post_id: post_id.clone(),
        liker_peer_id: identity.peer_id.clone(),
        reaction_type: "like".to_string(),
        timestamp,
        signature,
    };

    LikesRepository::add_like(&db, &data).map_err(|e| AppError::DatabaseString(e.to_string()))?;

    // Return updated summary
    LikesRepository::get_like_summary(&db, &post_id, &identity.peer_id)
        .map_err(|e| AppError::DatabaseString(e.to_string()))
}

/// Unlike a post
#[tauri::command]
pub async fn unlike_post(
    db: State<'_, Arc<Database>>,
    identity_service: State<'_, Arc<IdentityService>>,
    post_id: String,
) -> Result<LikeSummary> {
    // Get current identity
    let identity = identity_service
        .get_identity()?
        .ok_or_else(|| AppError::IdentityNotFound("No identity found".to_string()))?;

    LikesRepository::remove_like(&db, &post_id, &identity.peer_id)
        .map_err(|e| AppError::DatabaseString(e.to_string()))?;

    // Return updated summary
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
