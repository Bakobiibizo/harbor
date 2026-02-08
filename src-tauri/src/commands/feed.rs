//! Tauri commands for feed functionality

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;

use crate::db::repositories::{PostVisibility, PostsRepository};
use crate::db::Database;
use crate::error::AppError;
use crate::services::{FeedItem, FeedService, IdentityService};

/// Feed item info for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedItemInfo {
    pub post_id: String,
    pub author_peer_id: String,
    pub author_display_name: Option<String>,
    pub content_type: String,
    pub content_text: Option<String>,
    pub visibility: String,
    pub lamport_clock: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub is_local: bool,
}

impl From<FeedItem> for FeedItemInfo {
    fn from(item: FeedItem) -> Self {
        Self {
            post_id: item.post.post_id,
            author_peer_id: item.post.author_peer_id,
            author_display_name: item.author_display_name,
            content_type: item.post.content_type,
            content_text: item.post.content_text,
            visibility: item.post.visibility.as_str().to_string(),
            lamport_clock: item.post.lamport_clock,
            created_at: item.post.created_at,
            updated_at: item.post.updated_at,
            is_local: item.post.is_local,
        }
    }
}

/// Get the user's feed
#[tauri::command]
pub async fn get_feed(
    feed_service: State<'_, Arc<FeedService>>,
    limit: Option<i64>,
    before_timestamp: Option<i64>,
) -> Result<Vec<FeedItemInfo>, AppError> {
    let limit = limit.unwrap_or(50);
    let items = feed_service.get_feed(limit, before_timestamp)?;
    Ok(items.into_iter().map(FeedItemInfo::from).collect())
}

/// Get a specific user's wall
#[tauri::command]
pub async fn get_wall(
    feed_service: State<'_, Arc<FeedService>>,
    author_peer_id: String,
    limit: Option<i64>,
    before_timestamp: Option<i64>,
) -> Result<Vec<FeedItemInfo>, AppError> {
    let limit = limit.unwrap_or(50);
    let items = feed_service.get_wall(&author_peer_id, limit, before_timestamp)?;
    Ok(items.into_iter().map(FeedItemInfo::from).collect())
}

/// View perspective for wall preview
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ViewPerspective {
    /// View as a guest (public only)
    Guest,
    /// View as a contact (public + contacts-only)
    Contact,
    /// View as owner (all posts)
    Owner,
}

/// Get a preview of the current user's wall from a specific perspective
/// This allows users to see what their wall looks like to others
#[tauri::command]
pub async fn get_wall_preview(
    db: State<'_, Arc<Database>>,
    identity_service: State<'_, Arc<IdentityService>>,
    perspective: ViewPerspective,
    limit: Option<i64>,
    before_timestamp: Option<i64>,
) -> Result<Vec<FeedItemInfo>, AppError> {
    let identity = identity_service
        .get_identity()?
        .ok_or_else(|| AppError::IdentityNotFound("No identity found".to_string()))?;

    let limit = limit.unwrap_or(50);

    // Get all posts from our wall
    let posts = PostsRepository::get_by_author(&db, &identity.peer_id, limit, before_timestamp)
        .map_err(|e| AppError::DatabaseString(e.to_string()))?;

    // Filter based on perspective
    let filtered_posts: Vec<_> = posts
        .into_iter()
        .filter(|post| match perspective {
            ViewPerspective::Guest => post.visibility == PostVisibility::Public,
            ViewPerspective::Contact => true, // Contacts can see all posts
            ViewPerspective::Owner => true,   // Owner can see everything
        })
        .map(|post| FeedItemInfo {
            post_id: post.post_id,
            author_peer_id: post.author_peer_id,
            author_display_name: Some(identity.display_name.clone()),
            content_type: post.content_type,
            content_text: post.content_text,
            visibility: post.visibility.as_str().to_string(),
            lamport_clock: post.lamport_clock,
            created_at: post.created_at,
            updated_at: post.updated_at,
            is_local: post.is_local,
        })
        .collect();

    Ok(filtered_posts)
}

/// Get stats about how your wall appears to different perspectives
#[tauri::command]
pub async fn get_wall_visibility_stats(
    db: State<'_, Arc<Database>>,
    identity_service: State<'_, Arc<IdentityService>>,
) -> Result<WallVisibilityStats, AppError> {
    let identity = identity_service
        .get_identity()?
        .ok_or_else(|| AppError::IdentityNotFound("No identity found".to_string()))?;

    // Get all posts (using a large limit)
    let posts = PostsRepository::get_by_author(&db, &identity.peer_id, 1000, None)
        .map_err(|e| AppError::DatabaseString(e.to_string()))?;

    let total_posts = posts.len();
    let public_posts = posts
        .iter()
        .filter(|p| p.visibility == PostVisibility::Public)
        .count();
    let contacts_only_posts = posts
        .iter()
        .filter(|p| p.visibility == PostVisibility::Contacts)
        .count();

    Ok(WallVisibilityStats {
        total_posts,
        public_posts,
        contacts_only_posts,
        guest_visible: public_posts,
        contact_visible: total_posts,
    })
}

/// Stats about wall visibility
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WallVisibilityStats {
    /// Total number of posts
    pub total_posts: usize,
    /// Number of public posts
    pub public_posts: usize,
    /// Number of contacts-only posts
    pub contacts_only_posts: usize,
    /// Number of posts visible to guests
    pub guest_visible: usize,
    /// Number of posts visible to contacts
    pub contact_visible: usize,
}
