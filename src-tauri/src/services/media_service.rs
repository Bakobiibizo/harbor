//! Media storage service with SHA256 content-addressing
//!
//! Stores media files on disk using their SHA256 hash as the filename,
//! organized in a two-level directory structure to avoid having too many
//! files in a single directory.
//!
//! File layout: `{app_data}/media/{first-2-chars-of-hash}/{hash}.{ext}`

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::db::Database;
use crate::error::{AppError, Result};

/// Default chunk size for P2P media transfer (256 KB)
const DEFAULT_CHUNK_SIZE: u32 = 256 * 1024;
const MAX_TRANSFER_BYTES: u64 = 10 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MediaTransferState {
    pub media_hash: String,
    pub source_peer_id: Option<String>,
    pub media_type: String,
    pub mime_type: Option<String>,
    pub file_name: Option<String>,
    pub total_bytes: Option<u64>,
    pub bytes_received: u64,
    pub status: String,
    pub attempt_count: u32,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub updated_at: i64,
}

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
        if file_data.is_empty() || file_data.len() as u64 > MAX_TRANSFER_BYTES {
            return Err(AppError::InvalidData(
                "Attachment must be between 1 byte and 10 MB".into(),
            ));
        }
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

        let media_type = mime_type.split('/').next().unwrap_or("image");
        let _ = self.ensure_transfer(
            &hash,
            None,
            media_type,
            Some(mime_type),
            None,
            Some(file_data.len() as u64),
        );

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
        let total_chunks = total_size.div_ceil(chunk_size);

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

    /// Register signed/verified attachment metadata even when its bytes have
    /// not arrived yet. One bounded row per content hash is the canonical
    /// transfer lifecycle shared by posts, messages, commands, and P2P events.
    pub fn ensure_transfer(
        &self,
        hash: &str,
        source_peer_id: Option<&str>,
        media_type: &str,
        mime_type: Option<&str>,
        file_name: Option<&str>,
        total_bytes: Option<u64>,
    ) -> Result<MediaTransferState> {
        validate_hash(hash)?;
        if !matches!(media_type, "image" | "video" | "audio") {
            return Err(AppError::InvalidData("Unsupported attachment type".into()));
        }
        if total_bytes.is_some_and(|size| size > MAX_TRANSFER_BYTES)
            || source_peer_id.is_some_and(|value| value.len() > 128)
            || mime_type.is_some_and(|value| value.len() > 128)
            || file_name.is_some_and(|value| value.chars().count() > 255)
        {
            return Err(AppError::InvalidData(
                "Attachment metadata exceeds limits".into(),
            ));
        }
        let now = chrono::Utc::now().timestamp();
        let ready = self.has_media(hash);
        self.db
            .with_connection(|conn| {
                conn.execute(
                    "INSERT INTO media_transfers (
                        media_hash, source_peer_id, media_type, mime_type, file_name,
                        total_bytes, bytes_received, status, updated_at
                     ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                     ON CONFLICT(media_hash) DO UPDATE SET
                        source_peer_id = COALESCE(excluded.source_peer_id, source_peer_id),
                        media_type = excluded.media_type,
                        mime_type = COALESCE(excluded.mime_type, mime_type),
                        file_name = COALESCE(excluded.file_name, file_name),
                        total_bytes = COALESCE(excluded.total_bytes, total_bytes),
                        status = CASE
                            WHEN excluded.status = 'ready' THEN 'ready'
                            WHEN media_transfers.status = 'ready' THEN 'queued'
                            ELSE media_transfers.status
                        END,
                        bytes_received = CASE
                            WHEN excluded.status = 'ready' THEN COALESCE(excluded.total_bytes, bytes_received)
                            WHEN media_transfers.status = 'ready' THEN 0
                            ELSE media_transfers.bytes_received
                        END,
                        updated_at = CASE
                            WHEN excluded.status = 'ready' OR media_transfers.status = 'ready'
                                THEN excluded.updated_at
                            ELSE media_transfers.updated_at
                        END",
                    rusqlite::params![
                        hash,
                        source_peer_id,
                        media_type,
                        mime_type,
                        file_name,
                        total_bytes.map(|value| value as i64),
                        if ready { total_bytes.unwrap_or(0) as i64 } else { 0 },
                        if ready { "ready" } else { "queued" },
                        now,
                    ],
                )?;
                Ok(())
            })
            .map_err(|error| AppError::DatabaseString(error.to_string()))?;
        self.get_transfer(hash)?.ok_or_else(|| {
            AppError::Internal("Media transfer row disappeared after registration".into())
        })
    }

    pub fn update_transfer(
        &self,
        hash: &str,
        status: &str,
        bytes_received: Option<u64>,
        total_bytes: Option<u64>,
        error_code: Option<&str>,
        error_message: Option<&str>,
        increment_attempt: bool,
    ) -> Result<MediaTransferState> {
        validate_hash(hash)?;
        if bytes_received.is_some_and(|size| size > MAX_TRANSFER_BYTES)
            || total_bytes.is_some_and(|size| size > MAX_TRANSFER_BYTES)
        {
            return Err(AppError::InvalidData(
                "Attachment progress exceeds limits".into(),
            ));
        }
        if !matches!(
            status,
            "queued"
                | "discovering"
                | "transferring"
                | "ready"
                | "unavailable"
                | "retrying"
                | "failed"
        ) {
            return Err(AppError::InvalidData(
                "Invalid media transfer status".into(),
            ));
        }
        let now = chrono::Utc::now().timestamp();
        self.db
            .with_connection(|conn| {
                conn.execute(
                    "INSERT INTO media_transfers
                        (media_hash, status, bytes_received, total_bytes, attempt_count,
                         error_code, error_message, updated_at)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                     ON CONFLICT(media_hash) DO UPDATE SET
                        status = excluded.status,
                        bytes_received = COALESCE(?, bytes_received),
                        total_bytes = COALESCE(?, total_bytes),
                        attempt_count = attempt_count + ?,
                        error_code = excluded.error_code,
                        error_message = excluded.error_message,
                        updated_at = excluded.updated_at",
                    rusqlite::params![
                        hash,
                        status,
                        bytes_received.unwrap_or(0) as i64,
                        total_bytes.map(|value| value as i64),
                        u8::from(increment_attempt),
                        error_code,
                        error_message,
                        now,
                        bytes_received.map(|value| value as i64),
                        total_bytes.map(|value| value as i64),
                        u8::from(increment_attempt),
                    ],
                )?;
                Ok(())
            })
            .map_err(|error| AppError::DatabaseString(error.to_string()))?;
        self.get_transfer(hash)?
            .ok_or_else(|| AppError::Internal("Media transfer row disappeared after update".into()))
    }

    pub fn get_transfer(&self, hash: &str) -> Result<Option<MediaTransferState>> {
        validate_hash(hash)?;
        if self.has_media(hash) {
            let _ = self.update_ready_if_present(hash);
        }
        self.db
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT media_hash, source_peer_id, media_type, mime_type, file_name,
                            total_bytes, bytes_received, status, attempt_count,
                            error_code, error_message, updated_at
                     FROM media_transfers WHERE media_hash = ?",
                    [hash],
                    map_transfer_state,
                )
                .optional()
            })
            .map_err(|error| AppError::DatabaseString(error.to_string()))
    }

    /// Rebuild expected post attachment rows and turn interrupted in-flight
    /// states back into a safe queued state after restart. The query is capped
    /// so corrupt or adversarial metadata cannot grow startup memory without bound.
    pub fn reconstruct_transfers(&self) -> Result<usize> {
        let expected = self
            .db
            .with_connection(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT pm.media_hash, p.author_peer_id, pm.media_type, pm.mime_type,
                            pm.file_name, pm.file_size
                     FROM post_media pm JOIN posts p ON p.post_id = pm.post_id
                     ORDER BY pm.id DESC LIMIT 2048",
                )?;
                let rows = stmt.query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, i64>(5)?,
                    ))
                })?;
                rows.collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(|error| AppError::DatabaseString(error.to_string()))?;
        for (hash, source, media_type, mime_type, file_name, size) in &expected {
            self.ensure_transfer(
                hash,
                Some(source),
                media_type,
                Some(mime_type),
                Some(file_name),
                (*size).try_into().ok(),
            )?;
        }
        self.db
            .with_connection(|conn| {
                conn.execute(
                    "UPDATE media_transfers
                     SET status = 'queued', bytes_received = 0,
                         error_code = 'restart',
                         error_message = 'Transfer interrupted; ready to retry',
                         updated_at = strftime('%s','now')
                     WHERE status IN ('discovering', 'transferring', 'retrying')",
                    [],
                )
            })
            .map_err(|error| AppError::DatabaseString(error.to_string()))?;
        Ok(expected.len())
    }

    fn update_ready_if_present(&self, hash: &str) -> Result<()> {
        self.db
            .with_connection(|conn| {
                conn.execute(
                    "UPDATE media_transfers SET status = 'ready',
                        bytes_received = COALESCE(total_bytes, bytes_received),
                        error_code = NULL, error_message = NULL,
                        updated_at = strftime('%s','now')
                     WHERE media_hash = ? AND status != 'ready'",
                    [hash],
                )?;
                Ok(())
            })
            .map_err(|error| AppError::DatabaseString(error.to_string()))
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

fn validate_hash(hash: &str) -> Result<()> {
    if hash.len() != 64 || !hash.chars().all(|character| character.is_ascii_hexdigit()) {
        return Err(AppError::InvalidData(
            "Invalid media content identifier".into(),
        ));
    }
    Ok(())
}

fn map_transfer_state(row: &rusqlite::Row<'_>) -> rusqlite::Result<MediaTransferState> {
    Ok(MediaTransferState {
        media_hash: row.get(0)?,
        source_peer_id: row.get(1)?,
        media_type: row.get(2)?,
        mime_type: row.get(3)?,
        file_name: row.get(4)?,
        total_bytes: row
            .get::<_, Option<i64>>(5)?
            .map(|value| value.max(0) as u64),
        bytes_received: row.get::<_, i64>(6)?.max(0) as u64,
        status: row.get(7)?,
        attempt_count: row.get::<_, i64>(8)?.max(0) as u32,
        error_code: row.get(9)?,
        error_message: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

/// Known file extensions to try when resolving a hash to a path.
const KNOWN_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "svg", "bmp", "ico", // images
    "mp4", "webm", "mov", "avi", "mkv", // video
    "mp3", "m4a", "wav", "ogg", // audio
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
        "audio/mpeg" => "mp3",
        "audio/mp4" => "m4a",
        "audio/wav" => "wav",
        "audio/ogg" => "ogg",
        "audio/webm" => "webm",
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
        assert_eq!(mime_to_extension("audio/mpeg"), "mp3");
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

    #[test]
    fn local_media_is_immediately_ready() {
        let tmp = tempfile::tempdir().unwrap();
        let db = Arc::new(Database::in_memory().unwrap());
        let service = MediaStorageService::new(tmp.path(), db).unwrap();
        let data = b"ready image";
        let hash = service.store_media(data, "image/png").unwrap();

        let state = service.get_transfer(&hash).unwrap().unwrap();
        assert_eq!(state.status, "ready");
        assert_eq!(state.bytes_received, data.len() as u64);
        assert_eq!(state.total_bytes, Some(data.len() as u64));
    }

    #[test]
    fn remote_metadata_progress_failure_and_retry_share_one_row() {
        let tmp = tempfile::tempdir().unwrap();
        let db = Arc::new(Database::in_memory().unwrap());
        let service = MediaStorageService::new(tmp.path(), db).unwrap();
        let hash = "a".repeat(64);

        let discovered = service
            .ensure_transfer(
                &hash,
                Some("remote-peer"),
                "video",
                Some("video/mp4"),
                Some("clip.mp4"),
                Some(1_000),
            )
            .unwrap();
        assert_eq!(discovered.status, "queued");
        assert_eq!(discovered.source_peer_id.as_deref(), Some("remote-peer"));

        let progress = service
            .update_transfer(
                &hash,
                "transferring",
                Some(400),
                Some(1_000),
                None,
                None,
                true,
            )
            .unwrap();
        assert_eq!(progress.bytes_received, 400);
        assert_eq!(progress.attempt_count, 1);

        let failed = service
            .update_transfer(
                &hash,
                "failed",
                None,
                None,
                Some("transport_timeout"),
                Some("Timed out"),
                false,
            )
            .unwrap();
        assert_eq!(failed.error_code.as_deref(), Some("transport_timeout"));

        let retrying = service
            .update_transfer(&hash, "retrying", Some(0), None, None, None, true)
            .unwrap();
        assert_eq!(retrying.attempt_count, 2);
        assert_eq!(retrying.error_code, None);

        // Duplicate discovery updates metadata in place without creating a
        // second lifecycle or incrementing attempts.
        service
            .ensure_transfer(
                &hash,
                Some("remote-peer"),
                "video",
                Some("video/mp4"),
                Some("clip.mp4"),
                Some(1_000),
            )
            .unwrap();
        let count: i64 = service
            .db
            .with_connection(|conn| {
                conn.query_row("SELECT COUNT(*) FROM media_transfers", [], |row| row.get(0))
            })
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn interrupted_transfer_reconstructs_as_queued_after_restart() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("media.sqlite");
        let hash = "b".repeat(64);
        {
            let db = Arc::new(Database::new(db_path.clone()).unwrap());
            let service = MediaStorageService::new(tmp.path(), db).unwrap();
            service
                .ensure_transfer(&hash, Some("remote-peer"), "audio", None, None, Some(500))
                .unwrap();
            service
                .update_transfer(
                    &hash,
                    "transferring",
                    Some(100),
                    Some(500),
                    None,
                    None,
                    true,
                )
                .unwrap();
        }

        let reopened = Arc::new(Database::new(db_path).unwrap());
        let service = MediaStorageService::new(tmp.path(), reopened).unwrap();
        service.reconstruct_transfers().unwrap();
        let state = service.get_transfer(&hash).unwrap().unwrap();
        assert_eq!(state.status, "queued");
        assert_eq!(state.bytes_received, 0);
        assert_eq!(state.error_code.as_deref(), Some("restart"));
    }

    #[test]
    fn missing_file_downgrades_stale_ready_lifecycle_for_refetch() {
        let tmp = tempfile::tempdir().unwrap();
        let db = Arc::new(Database::in_memory().unwrap());
        let service = MediaStorageService::new(tmp.path(), db).unwrap();
        let hash = service.store_media(b"attachment", "image/png").unwrap();
        let path = service.get_media_path(&hash).unwrap();
        std::fs::remove_file(path).unwrap();

        let state = service
            .ensure_transfer(
                &hash,
                Some("remote-peer"),
                "image",
                Some("image/png"),
                Some("attachment.png"),
                Some(10),
            )
            .unwrap();
        assert_eq!(state.status, "queued");
        assert_eq!(state.bytes_received, 0);
    }
}
