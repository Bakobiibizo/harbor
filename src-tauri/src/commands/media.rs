//! Tauri commands for media storage (content-addressed by SHA256)

use std::collections::HashMap;
use std::sync::Arc;
use tauri::State;

use crate::commands::NetworkState;
use crate::db::Database;
use crate::services::MediaStorageService;

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

    let data = std::fs::read(&path)
        .map_err(|e| format!("Failed to read media file: {}", e))?;

    // Determine MIME type from file extension
    let mime = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(extension_to_mime)
        .unwrap_or("application/octet-stream");

    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &data);

    Ok(format!("data:{};base64,{}", mime, encoded))
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

/// Preload missing media from connected peers.
///
/// Scans post_media for image entries where the file is missing locally,
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
    network_state: State<'_, NetworkState>,
) -> Result<u32, String> {
    // Query all image-type media entries with their author
    let all_media = db
        .with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT pm.media_hash, pm.media_type, p.author_peer_id
                 FROM post_media pm
                 JOIN posts p ON pm.post_id = p.post_id
                 WHERE pm.media_type = 'image'",
            )?;

            let mut results = Vec::new();
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next()? {
                let media_hash: String = row.get(0)?;
                let _media_type: String = row.get(1)?;
                let author_peer_id: String = row.get(2)?;
                results.push((media_hash, author_peer_id));
            }
            Ok(results)
        })
        .map_err(|e| format!("Failed to query post_media: {}", e))?;

    // Filter to missing media only
    let missing: Vec<(String, String)> = all_media
        .into_iter()
        .filter(|(hash, _)| !media_service.has_media(hash))
        .collect();

    if missing.is_empty() {
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

    let connected_peer_ids: std::collections::HashSet<String> = connected_peers
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

    // Group missing hashes by author
    let mut missing_by_author: HashMap<String, Vec<String>> = HashMap::new();
    for (hash, author) in &missing {
        missing_by_author
            .entry(author.clone())
            .or_default()
            .push(hash.clone());
    }

    let mut requests_sent = 0u32;
    let mut dials_initiated = 0u32;

    for (author_peer_id, hashes) in &missing_by_author {
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
                        tracing::warn!(
                            "Failed to send media fetch for {}: {}",
                            hash,
                            e
                        );
                    }
                }
            }
        } else if !relay_base_addrs.is_empty() {
            // Author is NOT connected — dial them through the relay circuit.
            // On the next preloader invocation (triggered by peer_connected or
            // wall_posts_received), they'll be connected and we can fetch.
            for base_addr in &relay_base_addrs {
                let circuit_addr_str =
                    format!("{}/p2p-circuit/p2p/{}", base_addr, author_peer_id);
                if let Ok(addr) = circuit_addr_str.parse::<libp2p::Multiaddr>() {
                    match handle.dial(peer_id, vec![addr]).await {
                        Ok(_) => {
                            tracing::info!(
                                "Dialing {} through relay for media fetch ({} images pending)",
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
            tracing::debug!(
                "Cannot fetch media from {}: not connected and no relay available",
                author_peer_id
            );
        }
    }

    tracing::info!(
        "Media preloader: {} missing from {} authors, {} peers connected, {} fetch requests sent, {} relay dials initiated",
        missing.len(),
        missing_by_author.len(),
        connected_peer_ids.len(),
        requests_sent,
        dials_initiated,
    );

    Ok(requests_sent)
}
