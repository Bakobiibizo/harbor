//! Tauri commands for wall post relay synchronization

use std::sync::Arc;
use tauri::State;

use crate::commands::NetworkState;
use crate::db::WallSocialEventsRepository;
use crate::error::AppError;
use crate::p2p::protocols::board_sync::{WallPostMediaItem, WallSocialEventItem};
use crate::services::{
    ContactsService, ContentSyncService, IdentityService, PostsService, WallSocialService,
};

/// Submit all local wall posts to the relay for offline availability.
/// This finds the connected community relay and sends each unsynced post.
/// Media metadata for supported images, video, and audio is included so receiving clients know what to fetch.
#[tauri::command]
pub async fn sync_wall_to_relay(
    network_state: State<'_, NetworkState>,
    posts_service: State<'_, Arc<PostsService>>,
) -> Result<u32, AppError> {
    posts_service.assert_can_publish()?;
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

        // Collect signed media metadata for this post.
        let media_items: Vec<WallPostMediaItem> = match posts_service.get_post_media(&post.post_id)
        {
            Ok(media_list) => media_list
                .into_iter()
                .map(|m| WallPostMediaItem {
                    media_hash: m.media_hash,
                    media_type: m.media_type,
                    mime_type: m.mime_type,
                    file_name: m.file_name,
                    file_size: m.file_size,
                    width: m.width,
                    height: m.height,
                    duration_seconds: m.duration_seconds,
                    sort_order: m.sort_order,
                    signature: m.signature,
                })
                .collect(),
            Err(_) => Vec::new(),
        };
        let mut sorted_media = media_items.clone();
        sorted_media.sort_by_key(|m| m.sort_order);
        let media_hashes = sorted_media.iter().map(|m| m.media_hash.clone()).collect();

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
                media_hashes,
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
    content_sync_service: State<'_, Arc<ContentSyncService>>,
    author_peer_id: String,
    since_lamport_clock: Option<i64>,
    limit: Option<u32>,
) -> Result<(), AppError> {
    let handle = network_state.get_handle().await?;

    let stats = handle.get_stats().await?;
    let relay_peer_id = find_relay_peer_id(&stats.relay_addresses)?;

    let cursor = content_sync_service.get_sync_cursor(&relay_peer_id.to_string())?;
    let stored_since = cursor.get(&author_peer_id).copied().unwrap_or(0) as i64;
    let since_lamport_clock = since_lamport_clock.unwrap_or(stored_since);

    handle
        .get_wall_posts_from_relay(
            relay_peer_id,
            author_peer_id,
            since_lamport_clock,
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
    content_sync_service: State<'_, Arc<ContentSyncService>>,
    limit: Option<u32>,
) -> Result<u32, AppError> {
    let handle = network_state.get_handle().await?;

    let stats = handle.get_stats().await?;
    let relay_peer_id = find_relay_peer_id(&stats.relay_addresses)?;

    let contacts = contacts_service.get_active_contacts()?;
    let limit = limit.unwrap_or(50);
    let mut requested = 0u32;

    let cursor = content_sync_service.get_sync_cursor(&relay_peer_id.to_string())?;

    for contact in contacts {
        let since_lamport_clock = cursor.get(&contact.peer_id).copied().unwrap_or(0) as i64;
        match handle
            .get_wall_posts_from_relay(
                relay_peer_id,
                contact.peer_id.clone(),
                since_lamport_clock,
                limit,
            )
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

/// Submit this peer's signed wall social events (comments/reactions) to the relay.
#[tauri::command]
pub async fn sync_wall_social_events_to_relay(
    network_state: State<'_, NetworkState>,
    identity_service: State<'_, Arc<IdentityService>>,
    db: State<'_, Arc<crate::db::Database>>,
) -> Result<u32, AppError> {
    let handle = network_state.get_handle().await?;
    let stats = handle.get_stats().await?;
    let relay_peer_id = find_relay_peer_id(&stats.relay_addresses)?;
    let identity = identity_service
        .get_identity()?
        .ok_or_else(|| AppError::IdentityNotFound("No identity found".to_string()))?;
    let events = WallSocialEventsRepository::list_by_actor_since(&db, &identity.peer_id, 0, 500)
        .map_err(|e| AppError::DatabaseString(e.to_string()))?;
    let items: Vec<WallSocialEventItem> = events
        .into_iter()
        .map(|event| WallSocialEventItem {
            event_id: event.event_id,
            event_type: event.event_type.as_str().to_string(),
            post_id: event.post_id,
            actor_peer_id: event.actor_peer_id,
            author_name: event.author_name,
            comment_id: event.comment_id,
            content: event.content,
            reaction_type: event.reaction_type,
            timestamp: event.timestamp,
            payload_cbor: event.payload_cbor,
            signature: event.signature,
        })
        .collect();
    let submitted = items.len() as u32;
    if !items.is_empty() {
        handle
            .submit_wall_social_events_to_relay(relay_peer_id, items)
            .await?;
    }
    Ok(submitted)
}

/// Fetch and apply signed wall social events for a set of posts from the relay.
#[tauri::command]
pub async fn fetch_wall_social_events_from_relay(
    network_state: State<'_, NetworkState>,
    wall_social_service: State<'_, Arc<WallSocialService>>,
    author_peer_id: String,
    post_ids: Vec<String>,
    after_timestamp: Option<i64>,
    limit: Option<u32>,
) -> Result<(), AppError> {
    let handle = network_state.get_handle().await?;
    let stats = handle.get_stats().await?;
    let relay_peer_id = find_relay_peer_id(&stats.relay_addresses)?;
    if post_ids.is_empty() {
        return Ok(());
    }
    handle
        .get_wall_social_events_from_relay(
            relay_peer_id,
            author_peer_id,
            post_ids,
            after_timestamp.unwrap_or(0),
            limit.unwrap_or(500),
        )
        .await?;
    let _ = wall_social_service; // managed state ensures service is initialized for async application.
    Ok(())
}

/// Delete a wall post from the relay.
#[tauri::command]
pub async fn delete_wall_post_on_relay(
    network_state: State<'_, NetworkState>,
    posts_service: State<'_, Arc<PostsService>>,
    post_id: String,
) -> Result<(), AppError> {
    let handle = network_state.get_handle().await?;

    let stats = handle.get_stats().await?;
    let relay_peer_id = find_relay_peer_id(&stats.relay_addresses)?;

    let tombstone = posts_service
        .get_post(&post_id)?
        .ok_or_else(|| AppError::NotFound("Post not found".to_string()))?;
    let deleted_at = tombstone.deleted_at.ok_or_else(|| {
        AppError::InvalidData(
            "Post must be deleted locally before its relay tombstone can be published".to_string(),
        )
    })?;

    handle
        .delete_wall_post_on_relay(
            relay_peer_id,
            post_id,
            tombstone.lamport_clock as u64,
            deleted_at,
            tombstone.signature,
        )
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
