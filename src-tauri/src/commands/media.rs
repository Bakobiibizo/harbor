//! Tauri commands for media storage (content-addressed by SHA256)

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::Arc;
use tauri::State;

use crate::commands::NetworkState;
use crate::db::Database;
use crate::services::{
    ContactsService, IdentityService, MediaCacheDiagnostics, MediaCacheSettings,
    MediaStorageService, MediaTransferState, MediaTransferUpdate,
};

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnsureMediaTransferInput {
    pub media_hash: String,
    pub source_peer_id: Option<String>,
    pub media_type: String,
    pub mime_type: Option<String>,
    pub file_name: Option<String>,
    pub total_bytes: Option<u64>,
}

/// Store a media file from a filesystem path, returning its SHA256 hash.
///
/// The frontend calls this with the path to a file the user selected,
/// and the hash is subsequently passed to `add_post_media` as the
/// `media_hash`.
#[tauri::command]
pub async fn store_media(
    file_path: String,
    mime_type: String,
    media_service: State<'_, Arc<MediaStorageService>>,
) -> Result<String, String> {
    let data = std::fs::read(&file_path)
        .map_err(|e| format!("Failed to read file {}: {}", file_path, e))?;

    let hash = media_service
        .store_media(&data, &mime_type)
        .map_err(|e| format!("Failed to store media: {}", e))?;
    media_service
        .pin_local_media(&hash)
        .map_err(|e| format!("Failed to protect local media: {}", e))?;

    Ok(hash)
}

/// Store media from raw bytes (base64-encoded from the frontend).
///
/// This is useful when the frontend already has the file data in memory
/// (e.g., from a drag-and-drop or paste event) rather than a file path.
#[tauri::command]
pub async fn store_media_bytes(
    data: Vec<u8>,
    mime_type: String,
    media_service: State<'_, Arc<MediaStorageService>>,
) -> Result<String, String> {
    let hash = media_service
        .store_media(&data, &mime_type)
        .map_err(|e| format!("Failed to store media: {}", e))?;
    media_service
        .pin_local_media(&hash)
        .map_err(|e| format!("Failed to protect local media: {}", e))?;

    Ok(hash)
}

/// Get a URL that the frontend can use in `<img>` or `<video>` tags to
/// display a stored media file.
///
/// Returns a `data:` URL with the file contents base64-encoded. This avoids
/// needing the Tauri asset protocol (which requires additional configuration)
/// and works reliably on all platforms.
#[tauri::command]
pub async fn get_media_url(
    hash: String,
    media_service: State<'_, Arc<MediaStorageService>>,
) -> Result<String, String> {
    let path = media_service
        .get_media_path(&hash)
        .map_err(|e| format!("Media not found: {}", e))?;

    let data = std::fs::read(&path).map_err(|e| format!("Failed to read media file: {}", e))?;
    let _ = media_service.touch_cache_entry(&hash);

    // Determine MIME type from file extension
    let mime = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(extension_to_mime)
        .unwrap_or("application/octet-stream");

    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &data);

    Ok(format!("data:{};base64,{}", mime, encoded))
}

#[tauri::command]
pub async fn get_media_cache_diagnostics(
    media_service: State<'_, Arc<MediaStorageService>>,
) -> Result<MediaCacheDiagnostics, String> {
    let evicted = media_service
        .enforce_cache_policy()
        .map_err(|error| error.to_string())?;
    media_service
        .cache_diagnostics(evicted)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn update_media_cache_settings(
    settings: MediaCacheSettings,
    media_service: State<'_, Arc<MediaStorageService>>,
) -> Result<MediaCacheDiagnostics, String> {
    media_service
        .update_cache_settings(settings)
        .map_err(|error| error.to_string())?;
    let evicted = media_service
        .enforce_cache_policy()
        .map_err(|error| error.to_string())?;
    media_service
        .cache_diagnostics(evicted)
        .map_err(|error| error.to_string())
}

/// Map a file extension back to a MIME type for data URLs.
fn extension_to_mime(ext: &str) -> &'static str {
    match ext {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "avi" => "video/x-msvideo",
        "mkv" => "video/x-matroska",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        _ => "application/octet-stream",
    }
}

/// Check whether a media file exists locally by its hash.
#[tauri::command]
pub async fn has_media(
    hash: String,
    media_service: State<'_, Arc<MediaStorageService>>,
) -> Result<bool, String> {
    Ok(media_service.has_media(&hash))
}

#[tauri::command]
pub async fn ensure_media_transfer(
    input: EnsureMediaTransferInput,
    media_service: State<'_, Arc<MediaStorageService>>,
) -> Result<MediaTransferState, String> {
    media_service
        .ensure_transfer(
            &input.media_hash,
            input.source_peer_id.as_deref(),
            &input.media_type,
            input.mime_type.as_deref(),
            input.file_name.as_deref(),
            input.total_bytes,
        )
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_media_transfer(
    media_hash: String,
    media_service: State<'_, Arc<MediaStorageService>>,
) -> Result<Option<MediaTransferState>, String> {
    media_service
        .get_transfer(&media_hash)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn retry_media_transfer(
    media_hash: String,
    media_service: State<'_, Arc<MediaStorageService>>,
    contacts_service: State<'_, Arc<ContactsService>>,
    network_state: State<'_, NetworkState>,
) -> Result<MediaTransferState, String> {
    let existing = media_service
        .get_transfer(&media_hash)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "This attachment is not registered on this device".to_string())?;
    if existing.status == "ready" {
        return Ok(existing);
    }
    let source = existing
        .source_peer_id
        .as_deref()
        .ok_or_else(|| "The attachment source is not currently known".to_string())?;
    let authorized = contacts_service
        .get_contact(source)
        .map_err(|error| error.to_string())?
        .is_some_and(|contact| !contact.is_blocked);
    if !authorized {
        return Err("The attachment source is no longer an active contact".to_string());
    }
    let peer_id = source
        .parse::<libp2p::PeerId>()
        .map_err(|_| "The attachment source is invalid".to_string())?;
    let _retrying = media_service
        .update_transfer(
            &media_hash,
            MediaTransferUpdate {
                status: "retrying",
                bytes_received: Some(0),
                total_bytes: existing.total_bytes,
                error_code: None,
                error_message: None,
                increment_attempt: true,
            },
        )
        .map_err(|error| error.to_string())?;
    let handle = match network_state.get_handle().await {
        Ok(handle) => handle,
        Err(_) => {
            let _ = media_service.update_transfer(
                &media_hash,
                MediaTransferUpdate {
                    status: "unavailable",
                    bytes_received: Some(0),
                    total_bytes: existing.total_bytes,
                    error_code: Some("offline"),
                    error_message: Some("Harbor is offline. Reconnect before retrying."),
                    increment_attempt: false,
                },
            );
            return Err("Harbor is offline; reconnect before retrying".to_string());
        }
    };
    if handle
        .fetch_media(peer_id, media_hash.clone())
        .await
        .is_err()
    {
        let _ = media_service.update_transfer(
            &media_hash,
            MediaTransferUpdate {
                status: "failed",
                bytes_received: Some(0),
                total_bytes: existing.total_bytes,
                error_code: Some("start_failed"),
                error_message: Some("The attachment retry could not be started."),
                increment_attempt: false,
            },
        );
        return Err("The attachment retry could not be started".to_string());
    }
    media_service
        .get_transfer(&media_hash)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Attachment lifecycle disappeared during retry".to_string())
}

/// Preload missing media from connected peers.
///
/// Scans post_media for supported media entries where the file is missing locally,
/// groups them by author peer ID, and either:
/// - Sends P2P fetch requests if the author is already connected
/// - Dials the author through the relay circuit to establish a connection
///   (media will be fetched on the next preloader invocation once connected)
///
/// Returns the number of fetch requests sent.
#[tauri::command]
pub async fn preload_missing_media(
    db: State<'_, Arc<Database>>,
    media_service: State<'_, Arc<MediaStorageService>>,
    identity_service: State<'_, Arc<IdentityService>>,
    contacts_service: State<'_, Arc<ContactsService>>,
    network_state: State<'_, NetworkState>,
) -> Result<u32, String> {
    // A locked profile cannot authorize or sign transfer requests. Avoid
    // leaking prior UI/network activity across the lock boundary.
    if !identity_service.is_unlocked() {
        return Ok(0);
    }
    let settings = media_service
        .cache_settings()
        .map_err(|error| error.to_string())?;
    let active_contacts: HashSet<String> = contacts_service
        .get_active_contacts()
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|contact| contact.peer_id)
        .collect();
    media_service
        .prune_unauthorized_cache_sources()
        .map_err(|error| error.to_string())?;
    media_service
        .enforce_cache_policy()
        .map_err(|error| error.to_string())?;
    if !settings.enabled || active_contacts.is_empty() {
        return Ok(0);
    }

    // Get local peer ID to exclude own posts (our media is already local)
    let local_peer_id = identity_service
        .get_identity()
        .ok()
        .flatten()
        .map(|id| id.peer_id);

    // Query all supported media entries with their author (excluding own posts)
    let all_media = db
        .with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT pm.media_hash, pm.media_type, p.author_peer_id,
                        pm.mime_type, pm.file_name, pm.file_size, MAX(p.created_at)
                 FROM post_media pm
                 JOIN posts p ON pm.post_id = p.post_id
                 JOIN contacts c ON c.peer_id = p.author_peer_id AND c.is_blocked = 0
                 WHERE pm.media_type IN ('image', 'video', 'audio')
                   AND p.created_at >= ?
                 GROUP BY pm.media_hash, pm.media_type, p.author_peer_id,
                          pm.mime_type, pm.file_name, pm.file_size
                 ORDER BY MAX(p.created_at) DESC, pm.media_hash ASC
                 LIMIT 512",
            )?;

            let mut results = Vec::new();
            let cutoff = chrono::Utc::now()
                .timestamp()
                .saturating_sub(settings.retention_seconds as i64);
            let mut rows = stmt.query([cutoff])?;
            while let Some(row) = rows.next()? {
                let media_hash: String = row.get(0)?;
                let media_type: String = row.get(1)?;
                let author_peer_id: String = row.get(2)?;
                let mime_type: String = row.get(3)?;
                let file_name: String = row.get(4)?;
                let file_size: i64 = row.get(5)?;
                let observed_at: i64 = row.get(6)?;
                results.push((
                    media_hash,
                    author_peer_id,
                    media_type,
                    mime_type,
                    file_name,
                    file_size,
                    observed_at,
                ));
            }
            Ok(results)
        })
        .map_err(|e| format!("Failed to query post_media: {}", e))?;

    // Only verified metadata from accepted contacts enters the bounded cache.
    // Keep every authorized source for a shared hash so a blocked/offline peer
    // cannot invalidate media still available from another contact.
    let mut candidate_sources = BTreeMap::<String, BTreeSet<String>>::new();
    let mut reserved_bytes = media_service
        .cache_reserved_bytes()
        .map_err(|error| error.to_string())?;
    for (hash, author, media_type, mime_type, file_name, file_size, observed_at) in all_media {
        if local_peer_id.as_deref() == Some(author.as_str()) || !active_contacts.contains(&author) {
            continue;
        }
        let Ok(size) = u64::try_from(file_size) else {
            continue;
        };
        let is_new_hash = !candidate_sources.contains_key(&hash);
        let already_tracked = media_service
            .is_cache_tracked(&hash)
            .map_err(|error| error.to_string())?;
        if size > crate::services::posts_service::MAX_POST_MEDIA_BYTES as u64
            || (is_new_hash
                && !already_tracked
                && reserved_bytes.saturating_add(size) > settings.max_bytes)
        {
            continue;
        }
        let Ok(state) = media_service.ensure_transfer(
            &hash,
            Some(&author),
            &media_type,
            Some(&mime_type),
            Some(&file_name),
            Some(size),
        ) else {
            continue;
        };
        if !media_service
            .register_cache_candidate(&hash, &author, observed_at, Some(size))
            .unwrap_or(false)
        {
            continue;
        }
        if !media_service.has_media(&hash)
            && matches!(state.status.as_str(), "queued" | "unavailable" | "failed")
            && candidate_sources.entry(hash).or_default().insert(author)
            && is_new_hash
            && !already_tracked
        {
            reserved_bytes = reserved_bytes.saturating_add(size);
        }
    }

    if candidate_sources.is_empty() {
        return Ok(0);
    }

    // Get network handle
    let handle = match network_state.get_handle().await {
        Ok(h) => h,
        Err(_) => return Ok(0), // Network not running, skip silently
    };

    // Get connected peers and network stats (for relay addresses)
    let connected_peers = handle.get_connected_peers().await.unwrap_or_default();
    let stats = handle.get_stats().await.ok();

    let connected_peer_ids: HashSet<String> = connected_peers
        .iter()
        .filter(|p| p.is_connected)
        .map(|p| p.peer_id.clone())
        .collect();

    // Extract relay base addresses for circuit dialing.
    // Relay addresses look like: /ip4/.../tcp/.../p2p/RELAY_ID/p2p-circuit/p2p/LOCAL_ID
    // We strip from /p2p-circuit onward to get the relay base:
    //   /ip4/.../tcp/.../p2p/RELAY_ID
    let relay_base_addrs: Vec<String> = stats
        .as_ref()
        .map(|s| {
            s.relay_addresses
                .iter()
                .filter_map(|addr| {
                    addr.split("/p2p-circuit")
                        .next()
                        .map(|base| base.to_string())
                })
                .collect()
        })
        .unwrap_or_default();

    // Prefer a connected authorized source, using the peer ID as a stable
    // tie-breaker. Record that selected source in the canonical lifecycle so
    // an explicit retry uses the same valid peer.
    let missing_count = candidate_sources.len();
    let mut missing_by_author = BTreeMap::<String, Vec<String>>::new();
    for (hash, sources) in candidate_sources {
        let source = sources
            .iter()
            .find(|source| connected_peer_ids.contains(*source))
            .or_else(|| sources.iter().next());
        let Some(source) = source else { continue };
        if let Ok(Some(state)) = media_service.get_transfer(&hash) {
            let _ = media_service.ensure_transfer(
                &hash,
                Some(source),
                &state.media_type,
                state.mime_type.as_deref(),
                state.file_name.as_deref(),
                state.total_bytes,
            );
        }
        missing_by_author
            .entry(source.clone())
            .or_default()
            .push(hash);
    }

    let mut requests_sent = 0u32;
    let mut dials_initiated = 0u32;

    for (author_peer_id, hashes) in &missing_by_author {
        if !identity_service.is_unlocked()
            || contacts_service
                .get_contact(author_peer_id)
                .map_err(|error| error.to_string())?
                .is_none_or(|contact| contact.is_blocked)
        {
            continue;
        }
        let peer_id = match author_peer_id.parse::<libp2p::PeerId>() {
            Ok(id) => id,
            Err(_) => continue,
        };

        if connected_peer_ids.contains(author_peer_id) {
            // Author is directly connected — send fetch requests
            for hash in hashes {
                match handle.fetch_media(peer_id, hash.clone()).await {
                    Ok(_) => {
                        requests_sent += 1;
                        tracing::debug!(
                            "Sent media fetch request for {} to {}",
                            hash,
                            author_peer_id
                        );
                    }
                    Err(e) => {
                        tracing::warn!("Failed to send media fetch for {}: {}", hash, e);
                    }
                }
            }
        } else if !relay_base_addrs.is_empty() {
            for hash in hashes {
                let _ = media_service.update_transfer(
                    hash,
                    MediaTransferUpdate {
                        status: "discovering",
                        bytes_received: Some(0),
                        total_bytes: None,
                        error_code: None,
                        error_message: None,
                        increment_attempt: false,
                    },
                );
            }
            // Author is NOT connected — dial them through the relay circuit.
            // On the next preloader invocation (triggered by peer_connected or
            // wall_posts_received), they'll be connected and we can fetch.
            for base_addr in &relay_base_addrs {
                let circuit_addr_str = format!("{}/p2p-circuit/p2p/{}", base_addr, author_peer_id);
                if let Ok(addr) = circuit_addr_str.parse::<libp2p::Multiaddr>() {
                    match handle.dial(peer_id, vec![addr]).await {
                        Ok(_) => {
                            tracing::info!(
                                "Dialing {} through relay for media fetch ({} media items pending)",
                                author_peer_id,
                                hashes.len()
                            );
                            dials_initiated += 1;
                            break; // one successful dial attempt is enough
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to dial {} through relay: {}",
                                author_peer_id,
                                e
                            );
                        }
                    }
                }
            }
        } else {
            for hash in hashes {
                let _ = media_service.update_transfer(
                    hash,
                    MediaTransferUpdate {
                        status: "unavailable",
                        bytes_received: None,
                        total_bytes: None,
                        error_code: Some("source_offline"),
                        error_message: Some(
                            "The attachment source is offline. You can retry when they reconnect.",
                        ),
                        increment_attempt: false,
                    },
                );
            }
            tracing::debug!(
                "Cannot fetch media from {}: not connected and no relay available",
                author_peer_id
            );
        }
    }

    tracing::info!(
        "Media preloader: {} missing from {} authors, {} peers connected, {} fetch requests sent, {} relay dials initiated",
        missing_count,
        missing_by_author.len(),
        connected_peer_ids.len(),
        requests_sent,
        dials_initiated,
    );

    Ok(requests_sent)
}
