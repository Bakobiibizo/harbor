//! Tauri commands for community boards

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;

use crate::commands::NetworkState;
use crate::error::AppError;
use crate::services::BoardService;

/// Community info for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommunityInfo {
    pub relay_peer_id: String,
    pub relay_address: String,
    pub community_name: Option<String>,
    pub joined_at: i64,
    pub last_sync_at: Option<i64>,
}

/// Board info for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardInfoFe {
    pub board_id: String,
    pub relay_peer_id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_default: bool,
}

/// Board post info for the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardPostInfoFe {
    pub post_id: String,
    pub board_id: String,
    pub relay_peer_id: String,
    pub author_peer_id: String,
    pub author_display_name: Option<String>,
    pub content_type: String,
    pub content_text: Option<String>,
    pub lamport_clock: i64,
    pub created_at: i64,
}

/// Get all joined communities
#[tauri::command]
pub async fn get_communities(
    board_service: State<'_, Arc<BoardService>>,
) -> Result<Vec<CommunityInfo>, AppError> {
    let communities = board_service.get_communities()?;
    Ok(communities
        .into_iter()
        .map(|c| CommunityInfo {
            relay_peer_id: c.relay_peer_id,
            relay_address: c.relay_address,
            community_name: c.community_name,
            joined_at: c.joined_at,
            last_sync_at: c.last_sync_at,
        })
        .collect())
}

/// Join a community by connecting to a relay
#[tauri::command]
pub async fn join_community(
    network_state: State<'_, NetworkState>,
    relay_address: String,
) -> Result<(), AppError> {
    let handle = network_state.get_handle().await?;

    // Parse the multiaddress to extract peer ID
    let addr: libp2p::Multiaddr = relay_address
        .parse()
        .map_err(|e| AppError::Network(format!("Invalid address: {}", e)))?;

    let relay_peer_id = addr
        .iter()
        .find_map(|proto| {
            if let libp2p::multiaddr::Protocol::P2p(peer_id) = proto {
                Some(peer_id)
            } else {
                None
            }
        })
        .ok_or_else(|| AppError::Network("Address must contain peer ID (/p2p/...)".to_string()))?;

    // Dial the relay first
    handle.dial(relay_peer_id, vec![addr.clone()]).await.ok();

    // Join the community
    handle.join_community(relay_peer_id, relay_address).await
}

/// Leave a community
#[tauri::command]
pub async fn leave_community(
    board_service: State<'_, Arc<BoardService>>,
    relay_peer_id: String,
) -> Result<(), AppError> {
    board_service.leave_community(&relay_peer_id)
}

/// Get boards for a community (from local cache)
#[tauri::command]
pub async fn get_boards(
    board_service: State<'_, Arc<BoardService>>,
    relay_peer_id: String,
) -> Result<Vec<BoardInfoFe>, AppError> {
    let boards = board_service.get_boards(&relay_peer_id)?;
    Ok(boards
        .into_iter()
        .map(|b| BoardInfoFe {
            board_id: b.board_id,
            relay_peer_id: b.relay_peer_id,
            name: b.name,
            description: b.description,
            is_default: b.is_default,
        })
        .collect())
}

/// Get board posts from local cache
#[tauri::command]
pub async fn get_board_posts(
    board_service: State<'_, Arc<BoardService>>,
    relay_peer_id: String,
    board_id: String,
    limit: Option<i64>,
    before_timestamp: Option<i64>,
) -> Result<Vec<BoardPostInfoFe>, AppError> {
    let limit = limit.unwrap_or(50);
    let posts =
        board_service.get_board_posts(&relay_peer_id, &board_id, limit, before_timestamp)?;
    Ok(posts
        .into_iter()
        .map(|p| BoardPostInfoFe {
            post_id: p.post_id,
            board_id: p.board_id,
            relay_peer_id: p.relay_peer_id,
            author_peer_id: p.author_peer_id,
            author_display_name: p.author_display_name,
            content_type: p.content_type,
            content_text: p.content_text,
            lamport_clock: p.lamport_clock,
            created_at: p.created_at,
        })
        .collect())
}

/// Submit a post to a board on a relay
#[tauri::command]
pub async fn submit_board_post(
    network_state: State<'_, NetworkState>,
    relay_peer_id: String,
    board_id: String,
    content_text: String,
) -> Result<(), AppError> {
    let handle = network_state.get_handle().await?;

    let peer_id: libp2p::PeerId = relay_peer_id
        .parse()
        .map_err(|e| AppError::Network(format!("Invalid peer ID: {}", e)))?;

    handle
        .submit_board_post(peer_id, board_id, content_text)
        .await
}

/// Delete a board post on a relay
#[tauri::command]
pub async fn delete_board_post(
    network_state: State<'_, NetworkState>,
    relay_peer_id: String,
    post_id: String,
) -> Result<(), AppError> {
    let handle = network_state.get_handle().await?;

    let peer_id: libp2p::PeerId = relay_peer_id
        .parse()
        .map_err(|e| AppError::Network(format!("Invalid peer ID: {}", e)))?;

    handle.delete_board_post(peer_id, post_id).await
}

/// Sync a board (fetch latest posts from relay)
#[tauri::command]
pub async fn sync_board(
    network_state: State<'_, NetworkState>,
    relay_peer_id: String,
    board_id: String,
) -> Result<(), AppError> {
    let handle = network_state.get_handle().await?;

    let peer_id: libp2p::PeerId = relay_peer_id
        .parse()
        .map_err(|e| AppError::Network(format!("Invalid peer ID: {}", e)))?;

    // Use list_boards as a simple way to trigger sync — actually use get_board_posts
    handle.get_board_posts(peer_id, board_id, None, 50).await
}
