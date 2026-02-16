//! Tauri commands for media storage (content-addressed by SHA256)

use std::sync::Arc;
use tauri::State;

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
/// In Tauri v2 we return the absolute filesystem path with an
/// `asset://localhost/` prefix so the webview can load it via the asset
/// protocol. The CSP is updated to allow `asset:` and `https://asset.localhost`.
#[tauri::command]
pub async fn get_media_url(
    hash: String,
    media_service: State<'_, Arc<MediaStorageService>>,
) -> Result<String, String> {
    let path = media_service
        .get_media_path(&hash)
        .map_err(|e| format!("Media not found: {}", e))?;

    // Convert to a URL the Tauri webview can load.
    // Tauri v2 uses the `asset://localhost/{path}` scheme on all platforms.
    let path_str = path.to_string_lossy();

    // On Windows the path will have backslashes and a drive letter (e.g. D:\...).
    // The asset protocol expects a URI-encoded forward-slash path.
    #[cfg(target_os = "windows")]
    let url = {
        // Convert Windows path to a file URI-style path: /D:/path/to/file
        let normalized = path_str.replace('\\', "/");
        let with_slash = if normalized.starts_with('/') {
            normalized
        } else {
            format!("/{}", normalized)
        };
        format!(
            "https://asset.localhost{}",
            urlencoding::encode(&with_slash).replace("%2F", "/")
        )
    };

    #[cfg(not(target_os = "windows"))]
    let url = format!(
        "https://asset.localhost{}",
        urlencoding::encode(&*path_str).replace("%2F", "/")
    );

    Ok(url)
}

/// Check whether a media file exists locally by its hash.
#[tauri::command]
pub async fn has_media(
    hash: String,
    media_service: State<'_, Arc<MediaStorageService>>,
) -> Result<bool, String> {
    Ok(media_service.has_media(&hash))
}
