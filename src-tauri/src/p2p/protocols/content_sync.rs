use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Summary of a post for manifest responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostSummary {
    /// Unique post ID
    pub post_id: String,
    /// Author's peer ID
    pub author_peer_id: String,
    /// Lamport clock for ordering
    pub lamport_clock: u64,
    /// Content type (text, image, etc.)
    pub content_type: String,
    /// Whether the post has media attachments
    pub has_media: bool,
    /// Media hashes if any
    pub media_hashes: Vec<String>,
    /// Creation timestamp
    pub created_at: i64,
}

/// Request for content manifest from a peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentManifestRequest {
    /// Peer ID of the requester
    pub requester_peer_id: String,
    /// Cursor: map of author_peer_id -> highest_lamport_clock we've seen
    pub cursor: HashMap<String, u64>,
    /// Maximum number of posts to return
    pub limit: u32,
    /// Request timestamp
    pub timestamp: i64,
    /// Signature over all fields above
    pub signature: Vec<u8>,
}

/// Response with content manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentManifestResponse {
    /// Peer ID of the responder
    pub responder_peer_id: String,
    /// Post summaries
    pub posts: Vec<PostSummary>,
    /// Whether there are more posts available
    pub has_more: bool,
    /// Cursor for next request
    pub next_cursor: HashMap<String, u64>,
    /// Response timestamp
    pub timestamp: i64,
    /// Signature over all fields above
    pub signature: Vec<u8>,
}

/// Request for full post content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentFetchRequest {
    /// Peer ID of the requester
    pub requester_peer_id: String,
    /// ID of the post to fetch
    pub post_id: String,
    /// Whether to include media content
    pub include_media: bool,
    /// Request timestamp
    pub timestamp: i64,
    /// Signature over all fields above
    pub signature: Vec<u8>,
}

/// Media metadata attached to a fetched post.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentPostMediaItem {
    pub media_hash: String,
    pub media_type: String,
    pub mime_type: String,
    pub file_name: String,
    pub file_size: i64,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub duration_seconds: Option<i32>,
    pub sort_order: i32,
    pub signature: Vec<u8>,
}

/// Response with full post content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentFetchResponse {
    /// Peer ID of the responder
    pub responder_peer_id: String,
    /// The full post ID
    pub post_id: String,
    /// Author's peer ID
    pub author_peer_id: String,
    /// Content type
    pub content_type: String,
    /// Text content if any
    pub content_text: Option<String>,
    /// Visibility setting
    pub visibility: String,
    /// Lamport clock
    pub lamport_clock: u64,
    /// Creation timestamp
    pub created_at: i64,
    /// Author's signature on the post
    pub post_signature: Vec<u8>,
    /// Ordered media hashes covered by the post signature
    pub media_hashes: Vec<String>,
    /// Signed media metadata items
    pub media_items: Vec<ContentPostMediaItem>,
    /// Response timestamp
    pub timestamp: i64,
    /// Responder's signature over all fields above
    pub signature: Vec<u8>,
}

/// Request for media chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaChunkRequest {
    /// Peer ID of the requester
    pub requester_peer_id: String,
    /// Media hash to fetch
    pub media_hash: String,
    /// Chunk index (0-based)
    pub chunk_index: u32,
    /// Request timestamp
    pub timestamp: i64,
    /// Signature over all fields above
    pub signature: Vec<u8>,
}

/// Response with media chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaChunkResponse {
    /// Peer ID of the responder
    pub responder_peer_id: String,
    /// Media hash
    pub media_hash: String,
    /// Chunk index
    pub chunk_index: u32,
    /// Total number of chunks
    pub total_chunks: u32,
    /// Chunk data
    pub data: Vec<u8>,
    /// Checksum of chunk data
    pub checksum: String,
    /// Response timestamp
    pub timestamp: i64,
    /// Signature over all fields above
    pub signature: Vec<u8>,
}

/// Content sync protocol message wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentSyncMessage {
    /// Request for content manifest
    ManifestRequest(ContentManifestRequest),
    /// Response with content manifest
    ManifestResponse(ContentManifestResponse),
    /// Request for full post content
    FetchRequest(ContentFetchRequest),
    /// Response with full post content
    FetchResponse(ContentFetchResponse),
    /// Request for media chunk
    MediaRequest(MediaChunkRequest),
    /// Response with media chunk
    MediaResponse(MediaChunkResponse),
}

/// Codec for content sync protocol
#[derive(Debug, Clone, Default)]
pub struct ContentSyncCodec;

impl ContentSyncCodec {
    /// Encode a content sync message to CBOR bytes
    pub fn encode(
        msg: &ContentSyncMessage,
    ) -> Result<Vec<u8>, ciborium::ser::Error<std::io::Error>> {
        let mut bytes = Vec::new();
        ciborium::into_writer(msg, &mut bytes)?;
        Ok(bytes)
    }

    /// Decode a content sync message from CBOR bytes
    pub fn decode(bytes: &[u8]) -> Result<ContentSyncMessage, ciborium::de::Error<std::io::Error>> {
        ciborium::from_reader(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_request_roundtrip() {
        let mut cursor = HashMap::new();
        cursor.insert("peer-123".to_string(), 42u64);

        let request = ContentManifestRequest {
            requester_peer_id: "requester-peer".to_string(),
            cursor,
            limit: 50,
            timestamp: 1234567890,
            signature: vec![1, 2, 3, 4],
        };

        let wrapped = ContentSyncMessage::ManifestRequest(request.clone());
        let encoded = ContentSyncCodec::encode(&wrapped).unwrap();
        let decoded = ContentSyncCodec::decode(&encoded).unwrap();

        if let ContentSyncMessage::ManifestRequest(decoded_req) = decoded {
            assert_eq!(decoded_req.requester_peer_id, request.requester_peer_id);
            assert_eq!(decoded_req.limit, request.limit);
            assert_eq!(decoded_req.cursor.get("peer-123"), Some(&42u64));
        } else {
            panic!("Expected ManifestRequest variant");
        }
    }

    #[test]
    fn test_manifest_response_roundtrip() {
        let posts = vec![PostSummary {
            post_id: "post-123".to_string(),
            author_peer_id: "author-456".to_string(),
            lamport_clock: 10,
            content_type: "text".to_string(),
            has_media: false,
            media_hashes: vec![],
            created_at: 1234567890,
        }];

        let mut next_cursor = HashMap::new();
        next_cursor.insert("author-456".to_string(), 10u64);

        let response = ContentManifestResponse {
            responder_peer_id: "responder-peer".to_string(),
            posts: posts.clone(),
            has_more: true,
            next_cursor,
            timestamp: 1234567890,
            signature: vec![5, 6, 7, 8],
        };

        let wrapped = ContentSyncMessage::ManifestResponse(response.clone());
        let encoded = ContentSyncCodec::encode(&wrapped).unwrap();
        let decoded = ContentSyncCodec::decode(&encoded).unwrap();

        if let ContentSyncMessage::ManifestResponse(decoded_resp) = decoded {
            assert_eq!(decoded_resp.responder_peer_id, response.responder_peer_id);
            assert_eq!(decoded_resp.posts.len(), 1);
            assert!(decoded_resp.has_more);
        } else {
            panic!("Expected ManifestResponse variant");
        }
    }

    #[test]
    fn test_fetch_request_roundtrip() {
        let request = ContentFetchRequest {
            requester_peer_id: "requester".to_string(),
            post_id: "post-123".to_string(),
            include_media: true,
            timestamp: 1234567890,
            signature: vec![1, 2, 3],
        };

        let wrapped = ContentSyncMessage::FetchRequest(request.clone());
        let encoded = ContentSyncCodec::encode(&wrapped).unwrap();
        let decoded = ContentSyncCodec::decode(&encoded).unwrap();

        if let ContentSyncMessage::FetchRequest(decoded_req) = decoded {
            assert_eq!(decoded_req.post_id, request.post_id);
            assert!(decoded_req.include_media);
        } else {
            panic!("Expected FetchRequest variant");
        }
    }

    #[test]
    fn test_fetch_response_media_metadata_roundtrip() {
        let media_hashes = vec!["a".repeat(64), "b".repeat(64), "c".repeat(64)];
        let response = ContentFetchResponse {
            responder_peer_id: "responder".to_string(),
            post_id: "post-123".to_string(),
            author_peer_id: "author-456".to_string(),
            content_type: "mixed".to_string(),
            content_text: Some("media post".to_string()),
            visibility: "public".to_string(),
            lamport_clock: 7,
            created_at: 1234567890,
            post_signature: vec![1; 64],
            media_hashes: media_hashes.clone(),
            media_items: vec![
                ContentPostMediaItem {
                    media_hash: media_hashes[0].clone(),
                    media_type: "image".to_string(),
                    mime_type: "image/png".to_string(),
                    file_name: "photo.png".to_string(),
                    file_size: 100,
                    width: Some(640),
                    height: Some(480),
                    duration_seconds: None,
                    sort_order: 0,
                    signature: vec![2; 64],
                },
                ContentPostMediaItem {
                    media_hash: media_hashes[1].clone(),
                    media_type: "video".to_string(),
                    mime_type: "video/mp4".to_string(),
                    file_name: "clip.mp4".to_string(),
                    file_size: 200,
                    width: Some(1280),
                    height: Some(720),
                    duration_seconds: Some(3),
                    sort_order: 1,
                    signature: vec![3; 64],
                },
                ContentPostMediaItem {
                    media_hash: media_hashes[2].clone(),
                    media_type: "audio".to_string(),
                    mime_type: "audio/mpeg".to_string(),
                    file_name: "sound.mp3".to_string(),
                    file_size: 300,
                    width: None,
                    height: None,
                    duration_seconds: Some(5),
                    sort_order: 2,
                    signature: vec![4; 64],
                },
            ],
            timestamp: 1234567891,
            signature: vec![5; 64],
        };

        let wrapped = ContentSyncMessage::FetchResponse(response.clone());
        let encoded = ContentSyncCodec::encode(&wrapped).unwrap();
        let decoded = ContentSyncCodec::decode(&encoded).unwrap();

        if let ContentSyncMessage::FetchResponse(decoded_resp) = decoded {
            assert_eq!(decoded_resp.media_hashes, media_hashes);
            assert_eq!(decoded_resp.media_items.len(), 3);
            assert_eq!(decoded_resp.media_items[0].media_type, "image");
            assert_eq!(decoded_resp.media_items[1].media_type, "video");
            assert_eq!(decoded_resp.media_items[2].media_type, "audio");
        } else {
            panic!("Expected FetchResponse variant");
        }
    }

    #[test]
    fn test_media_chunk_roundtrip() {
        let response = MediaChunkResponse {
            responder_peer_id: "responder".to_string(),
            media_hash: "abc123".to_string(),
            chunk_index: 0,
            total_chunks: 5,
            data: vec![0, 1, 2, 3, 4, 5],
            checksum: "checksum123".to_string(),
            timestamp: 1234567890,
            signature: vec![7, 8, 9],
        };

        let wrapped = ContentSyncMessage::MediaResponse(response.clone());
        let encoded = ContentSyncCodec::encode(&wrapped).unwrap();
        let decoded = ContentSyncCodec::decode(&encoded).unwrap();

        if let ContentSyncMessage::MediaResponse(decoded_resp) = decoded {
            assert_eq!(decoded_resp.media_hash, response.media_hash);
            assert_eq!(decoded_resp.chunk_index, 0);
            assert_eq!(decoded_resp.total_chunks, 5);
            assert_eq!(decoded_resp.data.len(), 6);
        } else {
            panic!("Expected MediaResponse variant");
        }
    }
}
