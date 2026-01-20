//! Tauri commands for content synchronization

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::State;

use super::NetworkState;
use crate::error::AppError;
use crate::services::ContentSyncService;

/// Content sync status for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub peer_id: String,
    pub last_sync: Option<i64>,
    pub posts_synced: u32,
    pub status: String,
}

/// Request content manifest from a connected peer
#[tauri::command]
pub async fn request_content_manifest(
    network_state: State<'_, NetworkState>,
    peer_id: String,
    limit: Option<u32>,
) -> Result<(), AppError> {
    let handle = network_state.get_handle().await?;
    let peer_id = peer_id
        .parse()
        .map_err(|_| AppError::InvalidData("Invalid peer ID".to_string()))?;

    let cursor: HashMap<String, u64> = HashMap::new();
    let limit = limit.unwrap_or(50);

    handle
        .request_content_manifest(peer_id, cursor, limit)
        .await
}

/// Request content manifest with a specific cursor (for pagination)
#[tauri::command]
pub async fn request_content_manifest_with_cursor(
    network_state: State<'_, NetworkState>,
    peer_id: String,
    cursor: HashMap<String, u64>,
    limit: Option<u32>,
) -> Result<(), AppError> {
    let handle = network_state.get_handle().await?;
    let peer_id = peer_id
        .parse()
        .map_err(|_| AppError::InvalidData("Invalid peer ID".to_string()))?;

    let limit = limit.unwrap_or(50);

    handle
        .request_content_manifest(peer_id, cursor, limit)
        .await
}

/// Request to fetch a specific post from a peer
#[tauri::command]
pub async fn request_content_fetch(
    network_state: State<'_, NetworkState>,
    peer_id: String,
    post_id: String,
    include_media: Option<bool>,
) -> Result<(), AppError> {
    let handle = network_state.get_handle().await?;
    let peer_id = peer_id
        .parse()
        .map_err(|_| AppError::InvalidData("Invalid peer ID".to_string()))?;

    let include_media = include_media.unwrap_or(true);

    handle
        .request_content_fetch(peer_id, post_id, include_media)
        .await
}

/// Get sync cursor for a peer
#[tauri::command]
pub async fn get_sync_cursor(
    content_sync_service: State<'_, Arc<ContentSyncService>>,
    peer_id: String,
) -> Result<HashMap<String, u64>, AppError> {
    content_sync_service.get_sync_cursor(&peer_id)
}

/// Sync with all connected peers
#[tauri::command]
pub async fn sync_with_all_peers(
    network_state: State<'_, NetworkState>,
) -> Result<Vec<String>, AppError> {
    let handle = network_state.get_handle().await?;

    // Get connected peers
    let peers = handle.get_connected_peers().await?;
    let mut synced_peers = Vec::new();

    for peer in peers {
        let peer_id = peer
            .peer_id
            .parse()
            .map_err(|_| AppError::InvalidData("Invalid peer ID".to_string()))?;

        let cursor: HashMap<String, u64> = HashMap::new();

        // Request manifest from each peer (async, don't wait for response)
        match handle.request_content_manifest(peer_id, cursor, 50).await {
            Ok(_) => synced_peers.push(peer.peer_id),
            Err(e) => {
                tracing::warn!("Failed to request manifest from {}: {}", peer.peer_id, e);
            }
        }
    }

    Ok(synced_peers)
}
