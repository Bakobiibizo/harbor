//! Tauri commands for wall post relay synchronization

use std::sync::Arc;
use tauri::State;

use crate::commands::NetworkState;
use crate::error::AppError;
use crate::p2p::protocols::board_sync::WallPostMediaItem;
use crate::services::{ContactsService, PostsService};

/// Submit all local wall posts to the relay for offline availability.
/// This finds the connected community relay and sends each unsynced post.
/// Media metadata (images only) is included so receiving clients know what to fetch.
#[tauri::command]
pub async fn sync_wall_to_relay(
    network_state: State<'_, NetworkState>,
    posts_service: State<'_, Arc<PostsService>>,
) -> Result<u32, AppError> {
    let handle = network_state.get_handle().await?;

    // Get connected peers to find a relay
    let stats = handle.get_stats().await?;
    let relay_peer_id = find_relay_peer_id(&stats.relay_addresses)?;

    // Get all local posts
    let posts = posts_service.get_my_posts(500, None)?;
    let mut submitted = 0u32;

    for post in posts {
        if post.deleted_at.is_some() {
            continue;
        }

        // Collect image-only media metadata for this post
        let media_items: Vec<WallPostMediaItem> = match posts_service
            .get_post_media(&post.post_id)
        {
            Ok(media_list) => media_list
                .into_iter()
                .filter(|m| m.media_type == "image")
                .map(|m| WallPostMediaItem {
                    media_hash: m.media_hash,
                    media_type: m.media_type,
                    mime_type: m.mime_type,
                    file_name: m.file_name,
                    file_size: m.file_size,
                    width: m.width,
                    height: m.height,
                    sort_order: m.sort_order,
                })
                .collect(),
            Err(_) => Vec::new(),
        };

        handle
            .submit_wall_post_to_relay(
                relay_peer_id,
                post.post_id,
                post.content_type,
                post.content_text,
                post.visibility.as_str().to_string(),
                post.lamport_clock,
                post.created_at,
                post.signature,
                media_items,
            )
            .await?;
        submitted += 1;
    }

    Ok(submitted)
}

/// Fetch wall posts for a specific contact from the relay.
/// Uses lamport clock cursor for incremental sync.
#[tauri::command]
pub async fn fetch_contact_wall_from_relay(
    network_state: State<'_, NetworkState>,
    author_peer_id: String,
    since_lamport_clock: Option<i64>,
    limit: Option<u32>,
) -> Result<(), AppError> {
    let handle = network_state.get_handle().await?;

    let stats = handle.get_stats().await?;
    let relay_peer_id = find_relay_peer_id(&stats.relay_addresses)?;

    handle
        .get_wall_posts_from_relay(
            relay_peer_id,
            author_peer_id,
            since_lamport_clock.unwrap_or(0),
            limit.unwrap_or(50),
        )
        .await
}

/// Fetch wall posts for all contacts from the relay.
/// This iterates over all contacts and requests their wall posts.
#[tauri::command]
pub async fn sync_feed_from_relay(
    network_state: State<'_, NetworkState>,
    contacts_service: State<'_, Arc<ContactsService>>,
    limit: Option<u32>,
) -> Result<u32, AppError> {
    let handle = network_state.get_handle().await?;

    let stats = handle.get_stats().await?;
    let relay_peer_id = find_relay_peer_id(&stats.relay_addresses)?;

    let contacts = contacts_service.get_active_contacts()?;
    let limit = limit.unwrap_or(50);
    let mut requested = 0u32;

    for contact in contacts {
        match handle
            .get_wall_posts_from_relay(relay_peer_id, contact.peer_id.clone(), 0, limit)
            .await
        {
            Ok(_) => {
                requested += 1;
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to request wall posts for {} from relay: {}",
                    contact.peer_id,
                    e
                );
            }
        }
    }

    Ok(requested)
}

/// Delete a wall post from the relay.
#[tauri::command]
pub async fn delete_wall_post_on_relay(
    network_state: State<'_, NetworkState>,
    post_id: String,
) -> Result<(), AppError> {
    let handle = network_state.get_handle().await?;

    let stats = handle.get_stats().await?;
    let relay_peer_id = find_relay_peer_id(&stats.relay_addresses)?;

    handle
        .delete_wall_post_on_relay(relay_peer_id, post_id)
        .await
}

/// Helper to extract the relay PeerId from relay addresses.
/// Looks through the relay addresses for one that contains a /p2p/ component.
pub fn find_relay_peer_id(relay_addresses: &[String]) -> Result<libp2p::PeerId, AppError> {
    for addr_str in relay_addresses {
        // Parse the multiaddr to find the relay peer ID
        // Relay addresses look like: /ip4/.../tcp/.../p2p/RELAY_ID/p2p-circuit/p2p/LOCAL_ID
        // We want the first /p2p/ component which is the relay peer ID
        if let Ok(addr) = addr_str.parse::<libp2p::Multiaddr>() {
            // Find the first P2p protocol component (that's the relay)
            let mut found_first_p2p = false;
            for proto in addr.iter() {
                if let libp2p::multiaddr::Protocol::P2p(peer_id) = proto {
                    if !found_first_p2p {
                        return Ok(peer_id);
                    }
                    found_first_p2p = true;
                }
            }
        }
    }

    Err(AppError::Network(
        "No relay connected. Please connect to a relay first.".to_string(),
    ))
}
