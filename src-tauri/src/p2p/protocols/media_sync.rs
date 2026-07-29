//! Media sync protocol types
//!
//! P2P request-response protocol for fetching media bytes by SHA256 hash
//! directly from the author peer.

use serde::{Deserialize, Serialize};

/// Protocol version string for media sync
pub const MEDIA_SYNC_PROTOCOL: &str = "/harbor/media/1.0.0";

/// Request to fetch media bytes from a peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaFetchRequest {
    pub media_hash: String,
    pub offset: u64,
    pub max_bytes: u32,
    pub requester_peer_id: String,
    pub timestamp: i64,
    pub signature: Vec<u8>,
}

/// Response with media bytes or an error
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MediaFetchResponse {
    /// Successful response with the media data
    MediaChunk {
        media_hash: String,
        mime_type: String,
        offset: u64,
        total_bytes: u64,
        data: Vec<u8>,
        eof: bool,
    },
    /// Error response
    Error { error: String },
}
