//! Tauri commands for feed functionality

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;

use crate::error::AppError;
use crate::services::{FeedItem, FeedService};

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
