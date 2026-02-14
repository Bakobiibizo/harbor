//! Media storage service with SHA256 content-addressing
//!
//! Stores media files on disk using their SHA256 hash as the filename,
//! organized in a two-level directory structure to avoid having too many
//! files in a single directory.
//!
//! File layout: `{app_data}/media/{first-2-chars-of-hash}/{hash}.{ext}`

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::db::Database;
use crate::error::{AppError, Result};

/// Default chunk size for P2P media transfer (256 KB)
const DEFAULT_CHUNK_SIZE: u32 = 256 * 1024;

/// Service for content-addressed media file storage
pub struct MediaStorageService {
    media_dir: PathBuf,
    db: Arc<Database>,
}

impl MediaStorageService {
    /// Create a new media storage service.
    ///
    /// The `media/` directory is created under `app_data_dir` if it does not
    /// already exist.
    pub fn new(app_data_dir: &Path, db: Arc<Database>) -> Result<Self> {
        let media_dir = app_data_dir.join("media");
        std::fs::create_dir_all(&media_dir)?;

        Ok(Self { media_dir, db })
    }

    /// Store media file data, returning the hex-encoded SHA256 hash.
    ///
    /// If a file with the same hash already exists on disk it is not
    /// overwritten -- the existing path is reused (content-addressing
    /// guarantees identical content).
    pub fn store_media(&self, file_data: &[u8], mime_type: &str) -> Result<String> {
        // Compute SHA256 hash
        let mut hasher = Sha256::new();
        hasher.update(file_data);
        let hash_bytes = hasher.finalize();
        let hash = hex::encode(hash_bytes);

        // Determine file extension from MIME type
        let ext = mime_to_extension(mime_type);

        // Build storage path: media/{first2}/{hash}.{ext}
        let subdir = &hash[..2];
        let dir_path = self.media_dir.join(subdir);
        std::fs::create_dir_all(&dir_path)?;

        let file_name = format!("{}.{}", hash, ext);
        let file_path = dir_path.join(&file_name);

        // Only write if the file doesn't already exist (idempotent)
        if !file_path.exists() {
            std::fs::write(&file_path, file_data)?;
            tracing::info!(
                hash = %hash,
                size = file_data.len(),
                mime = %mime_type,
                "Stored media file"
            );
        } else {
            tracing::debug!(hash = %hash, "Media file already exists, skipping write");
        }

        Ok(hash)
    }

    /// Read the full media file for a given hash.
    pub fn get_media(&self, hash: &str) -> Result<Vec<u8>> {
        let file_path = self.resolve_path(hash)?;
        let data = std::fs::read(&file_path)?;
        Ok(data)
    }

    /// Read a chunk of a media file for P2P transfer.
    ///
    /// Returns `(chunk_data, total_chunks)`.
    pub fn get_media_chunk(
        &self,
        hash: &str,
        chunk_index: u32,
        chunk_size: u32,
    ) -> Result<(Vec<u8>, u32)> {
        let chunk_size = if chunk_size == 0 {
            DEFAULT_CHUNK_SIZE
        } else {
            chunk_size
        };

        let data = self.get_media(hash)?;
        let total_size = data.len() as u32;
        let total_chunks = (total_size + chunk_size - 1) / chunk_size; // ceiling division

        if chunk_index >= total_chunks {
            return Err(AppError::InvalidData(format!(
                "Chunk index {} out of range (total chunks: {})",
                chunk_index, total_chunks
            )));
        }

        let start = (chunk_index * chunk_size) as usize;
        let end = std::cmp::min(start + chunk_size as usize, data.len());
        let chunk = data[start..end].to_vec();

        Ok((chunk, total_chunks))
    }

    /// Check whether a media file exists on disk.
    pub fn has_media(&self, hash: &str) -> bool {
        self.resolve_path(hash).is_ok()
    }

    /// Delete a media file from disk if no other `post_media` rows reference
    /// the same hash.
    pub fn delete_media_if_orphaned(&self, hash: &str) -> Result<()> {
        // Count how many post_media rows still reference this hash
        let count: i64 = self
            .db
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT COUNT(*) FROM post_media WHERE media_hash = ?",
                    [hash],
                    |row| row.get(0),
                )
            })
            .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        if count == 0 {
            // No references remain -- safe to delete the file
            if let Ok(file_path) = self.resolve_path(hash) {
                std::fs::remove_file(&file_path)?;
                tracing::info!(hash = %hash, "Deleted orphaned media file");

                // Try to remove the parent sub-directory if it is now empty
                if let Some(parent) = file_path.parent() {
                    let _ = std::fs::remove_dir(parent); // ignore error (dir may not be empty)
                }
            }
        }

        Ok(())
    }

    /// Get the absolute filesystem path for a media file.
    ///
    /// This is used by the `get_media_url` command to return a path the
    /// frontend can load via Tauri's asset protocol.
    pub fn get_media_path(&self, hash: &str) -> Result<PathBuf> {
        self.resolve_path(hash)
    }

    // ── private helpers ──────────────────────────────────────────────

    /// Resolve the on-disk path for a hash, trying known extensions.
    fn resolve_path(&self, hash: &str) -> Result<PathBuf> {
        // Validate hash looks reasonable (hex, 64 chars for SHA256)
        if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(AppError::InvalidData(format!(
                "Invalid media hash: {}",
                hash
            )));
        }

        let subdir = &hash[..2];
        let dir_path = self.media_dir.join(subdir);

        // Try common extensions
        for ext in KNOWN_EXTENSIONS {
            let candidate = dir_path.join(format!("{}.{}", hash, ext));
            if candidate.exists() {
                return Ok(candidate);
            }
        }

        Err(AppError::NotFound(format!(
            "Media file not found for hash: {}",
            hash
        )))
    }
}

/// Known file extensions to try when resolving a hash to a path.
const KNOWN_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "svg", "bmp", "ico", // images
    "mp4", "webm", "mov", "avi", "mkv", // video
    "bin", // fallback
];

/// Map a MIME type to a file extension.
fn mime_to_extension(mime_type: &str) -> &'static str {
    match mime_type {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        "image/bmp" => "bmp",
        "image/x-icon" | "image/vnd.microsoft.icon" => "ico",
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "video/quicktime" => "mov",
        "video/x-msvideo" => "avi",
        "video/x-matroska" => "mkv",
        _ => "bin",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mime_to_extension() {
        assert_eq!(mime_to_extension("image/jpeg"), "jpg");
        assert_eq!(mime_to_extension("image/png"), "png");
        assert_eq!(mime_to_extension("video/mp4"), "mp4");
        assert_eq!(mime_to_extension("application/octet-stream"), "bin");
    }

    #[test]
    fn test_store_and_retrieve() {
        let tmp = tempfile::tempdir().unwrap();
        let db = Arc::new(Database::in_memory().unwrap());
        let service = MediaStorageService::new(tmp.path(), db).unwrap();

        let data = b"hello world media content";
        let hash = service.store_media(data, "image/png").unwrap();

        // Hash should be 64 hex chars
        assert_eq!(hash.len(), 64);

        // Should be able to retrieve
        assert!(service.has_media(&hash));
        let retrieved = service.get_media(&hash).unwrap();
        assert_eq!(retrieved, data);
    }

    #[test]
    fn test_idempotent_store() {
        let tmp = tempfile::tempdir().unwrap();
        let db = Arc::new(Database::in_memory().unwrap());
        let service = MediaStorageService::new(tmp.path(), db).unwrap();

        let data = b"same content";
        let hash1 = service.store_media(data, "image/jpeg").unwrap();
        let hash2 = service.store_media(data, "image/jpeg").unwrap();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_chunked_read() {
        let tmp = tempfile::tempdir().unwrap();
        let db = Arc::new(Database::in_memory().unwrap());
        let service = MediaStorageService::new(tmp.path(), db).unwrap();

        // 10 bytes of data, 4-byte chunks => 3 chunks (4 + 4 + 2)
        let data = b"0123456789";
        let hash = service.store_media(data, "image/png").unwrap();

        let (chunk0, total) = service.get_media_chunk(&hash, 0, 4).unwrap();
        assert_eq!(total, 3);
        assert_eq!(chunk0, b"0123");

        let (chunk1, _) = service.get_media_chunk(&hash, 1, 4).unwrap();
        assert_eq!(chunk1, b"4567");

        let (chunk2, _) = service.get_media_chunk(&hash, 2, 4).unwrap();
        assert_eq!(chunk2, b"89");

        // Out of range
        assert!(service.get_media_chunk(&hash, 3, 4).is_err());
    }

    #[test]
    fn test_invalid_hash() {
        let tmp = tempfile::tempdir().unwrap();
        let db = Arc::new(Database::in_memory().unwrap());
        let service = MediaStorageService::new(tmp.path(), db).unwrap();

        assert!(!service.has_media("not-a-valid-hash"));
        assert!(service.get_media("tooshort").is_err());
    }
}
