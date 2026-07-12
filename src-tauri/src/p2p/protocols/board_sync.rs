//! Board sync protocol types
//!
//! Used by both the relay server and the client for community board operations.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayAuthChallenge {
    pub domain: String,
    pub version: u8,
    pub id: String,
    pub relay: String,
    pub peer_id: String,
    pub audience: String,
    pub issued_at: i64,
    pub expires_at: i64,
    pub nonce: String,
    pub key_id: String,
    pub relay_signature: Vec<u8>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NameClaimRequest {
    pub domain: String,
    pub version: u16,
    pub local_name: String,
    pub relay: String,
    pub peer_id: String,
    pub ed25519_public_key: Vec<u8>,
    pub x25519_public_key: Vec<u8>,
    pub sequence: u64,
    pub issued_at: i64,
    pub nonce: Vec<u8>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedNameClaimRequest {
    pub request: NameClaimRequest,
    pub user_signature: Vec<u8>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NameClaim {
    pub request: NameClaimRequest,
    pub user_signature: Vec<u8>,
    pub status: String,
    pub not_before: i64,
    pub not_after: i64,
    pub relay_key_id: String,
    pub relay_signature: Vec<u8>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkChallenge {
    pub id: String,
    pub relay: String,
    pub requester: String,
    pub target: String,
    pub action: String,
    pub expires_at: i64,
    pub difficulty: u8,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntroductionEnvelope {
    pub version: u8,
    pub request_id: String,
    pub target: String,
    pub requester_peer_id: String,
    pub requester_ephemeral_x25519_key: Vec<u8>,
    pub message_ciphertext: Vec<u8>,
    pub issued_at: i64,
    pub expires_at: i64,
    pub work_challenge: WorkChallenge,
    pub work_nonce: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedEnvelope {
    pub request_id: String,
    pub requester_peer_id: String,
    pub requester_ephemeral_x25519_key: Vec<u8>,
    pub message_ciphertext: Vec<u8>,
    pub issued_at: i64,
    pub expires_at: i64,
}

/// Media metadata attached to a wall post.
///
/// Synced through the relay so that the receiving client knows which
/// media hashes to fetch via the P2P media protocol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallPostMediaItem {
    pub media_hash: String,
    pub media_type: String, // "image", "video", or "audio"
    pub mime_type: String,
    pub file_name: String,
    pub file_size: i64,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub duration_seconds: Option<i32>,
    pub sort_order: i32,
    pub signature: Vec<u8>,
}

/// Signed wall social event stored or returned by a relay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallSocialEventItem {
    pub event_id: String,
    pub event_type: String,
    pub post_id: String,
    pub actor_peer_id: String,
    pub author_name: Option<String>,
    pub comment_id: Option<String>,
    pub content: Option<String>,
    pub reaction_type: Option<String>,
    pub timestamp: i64,
    pub payload_cbor: Vec<u8>,
    pub signature: Vec<u8>,
}

/// Board sync request (wire protocol)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BoardSyncRequest {
    RelayAuthChallenge {
        peer_id: String,
        audience: String,
    },
    RelayAuthComplete {
        challenge: RelayAuthChallenge,
        public_key: Vec<u8>,
        signature: Vec<u8>,
    },
    RegisterRelayName {
        session_token: String,
        signed_request: SignedNameClaimRequest,
    },
    SubmitIntroduction {
        session_token: String,
        envelope: IntroductionEnvelope,
    },
    FetchIntroductions {
        session_token: String,
        limit: u32,
    },
    /// List all boards on the relay
    ListBoards {
        requester_peer_id: String,
        timestamp: i64,
        signature: Vec<u8>,
    },
    /// Get posts for a specific board
    GetBoardPosts {
        requester_peer_id: String,
        board_id: String,
        after_timestamp: Option<i64>,
        limit: u32,
        timestamp: i64,
        signature: Vec<u8>,
    },
    /// Submit a new post to a board
    SubmitPost {
        post_id: String,
        board_id: String,
        author_peer_id: String,
        content_type: String,
        content_text: Option<String>,
        lamport_clock: u64,
        created_at: i64,
        signature: Vec<u8>,
    },
    /// Register a peer with the relay (required before posting)
    RegisterPeer {
        peer_id: String,
        public_key: Vec<u8>,
        display_name: String,
        timestamp: i64,
        signature: Vec<u8>,
    },
    /// Delete a post from a board
    DeletePost {
        post_id: String,
        author_peer_id: String,
        timestamp: i64,
        signature: Vec<u8>,
    },
    /// Submit a wall post to the relay for offline availability
    SubmitWallPost {
        author_peer_id: String,
        post_id: String,
        content_type: String,
        content_text: Option<String>,
        visibility: String,
        lamport_clock: i64,
        created_at: i64,
        signature: Vec<u8>,
        #[serde(default)]
        media_hashes: Vec<String>,
        timestamp: i64,
        request_signature: Vec<u8>,
        #[serde(default)]
        media_items: Vec<WallPostMediaItem>,
    },
    /// Get wall posts for a specific author
    GetWallPosts {
        requester_peer_id: String,
        author_peer_id: String,
        since_lamport_clock: i64,
        limit: u32,
        timestamp: i64,
        signature: Vec<u8>,
    },
    /// Delete a wall post from the relay
    DeleteWallPost {
        author_peer_id: String,
        post_id: String,
        lamport_clock: u64,
        deleted_at: i64,
        signature: Vec<u8>,
    },
    /// Submit a signed wall social event to the relay.
    SubmitWallSocialEvent {
        event: WallSocialEventItem,
        timestamp: i64,
        request_signature: Vec<u8>,
    },
    /// Get signed wall social events for posts visible to requester.
    GetWallSocialEvents {
        requester_peer_id: String,
        author_peer_id: String,
        post_ids: Vec<String>,
        after_timestamp: i64,
        limit: u32,
        timestamp: i64,
        signature: Vec<u8>,
    },
}

/// Board info in responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardInfo {
    pub board_id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_default: bool,
}

/// Board post in responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardPostInfo {
    pub post_id: String,
    pub board_id: String,
    pub author_peer_id: String,
    pub author_display_name: Option<String>,
    pub content_type: String,
    pub content_text: Option<String>,
    pub lamport_clock: u64,
    pub created_at: i64,
    pub deleted_at: Option<i64>,
    pub signature: Vec<u8>,
}

/// Wall post data in responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallPostData {
    pub post_id: String,
    pub author_peer_id: String,
    pub content_type: String,
    pub content_text: Option<String>,
    pub visibility: String,
    pub lamport_clock: i64,
    pub created_at: i64,
    pub deleted_at: Option<i64>,
    pub signature: Vec<u8>,
    #[serde(default)]
    pub media_hashes: Vec<String>,
    pub stored_at: i64,
    #[serde(default)]
    pub media_items: Vec<WallPostMediaItem>,
}

/// Board sync response (wire protocol)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BoardSyncResponse {
    RelayAuthChallenge {
        challenge: RelayAuthChallenge,
    },
    RelaySession {
        token: String,
    },
    RelayNameRegistered {
        claim: NameClaim,
    },
    IntroductionAccepted {
        request_id: String,
        retry_after: u32,
    },
    Introductions {
        envelopes: Vec<QueuedEnvelope>,
    },
    /// List of boards
    BoardList {
        boards: Vec<BoardInfo>,
        relay_peer_id: String,
    },
    /// Posts for a board
    BoardPosts {
        board_id: String,
        posts: Vec<BoardPostInfo>,
        has_more: bool,
    },
    /// Post was accepted
    PostAccepted {
        post_id: String,
    },
    /// Peer was registered
    PeerRegistered {
        peer_id: String,
    },
    /// Post was deleted
    PostDeleted {
        post_id: String,
    },
    /// Wall posts for a specific author
    WallPosts {
        posts: Vec<WallPostData>,
        has_more: bool,
    },
    /// Wall post was stored on the relay
    WallPostStored {
        post_id: String,
    },
    /// Wall post was deleted from the relay
    WallPostDeleted {
        post_id: String,
    },
    /// Wall social event was stored on the relay
    WallSocialEventStored {
        event_id: String,
    },
    /// Wall social events visible to the requester
    WallSocialEvents {
        events: Vec<WallSocialEventItem>,
        has_more: bool,
        next_timestamp: i64,
    },
    /// Error response
    Error {
        error: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wall_post_relay_media_metadata_roundtrip() {
        let media_hashes = vec!["a".repeat(64), "b".repeat(64), "c".repeat(64)];
        let media_items = vec![
            WallPostMediaItem {
                media_hash: media_hashes[0].clone(),
                media_type: "image".to_string(),
                mime_type: "image/png".to_string(),
                file_name: "photo.png".to_string(),
                file_size: 100,
                width: Some(640),
                height: Some(480),
                duration_seconds: None,
                sort_order: 0,
                signature: vec![1; 64],
            },
            WallPostMediaItem {
                media_hash: media_hashes[1].clone(),
                media_type: "video".to_string(),
                mime_type: "video/mp4".to_string(),
                file_name: "clip.mp4".to_string(),
                file_size: 200,
                width: Some(1280),
                height: Some(720),
                duration_seconds: Some(4),
                sort_order: 1,
                signature: vec![2; 64],
            },
            WallPostMediaItem {
                media_hash: media_hashes[2].clone(),
                media_type: "audio".to_string(),
                mime_type: "audio/mpeg".to_string(),
                file_name: "sound.mp3".to_string(),
                file_size: 300,
                width: None,
                height: None,
                duration_seconds: Some(6),
                sort_order: 2,
                signature: vec![3; 64],
            },
        ];

        let request = BoardSyncRequest::SubmitWallPost {
            author_peer_id: "author".to_string(),
            post_id: "post-1".to_string(),
            content_type: "mixed".to_string(),
            content_text: Some("media".to_string()),
            visibility: "public".to_string(),
            lamport_clock: 1,
            created_at: 1234567890,
            signature: vec![4; 64],
            media_hashes: media_hashes.clone(),
            timestamp: 1234567891,
            request_signature: vec![5; 64],
            media_items: media_items.clone(),
        };

        let mut encoded = Vec::new();
        ciborium::into_writer(&request, &mut encoded).unwrap();
        let decoded: BoardSyncRequest = ciborium::from_reader(encoded.as_slice()).unwrap();

        match decoded {
            BoardSyncRequest::SubmitWallPost {
                media_hashes: decoded_hashes,
                media_items: decoded_items,
                ..
            } => {
                assert_eq!(decoded_hashes, media_hashes);
                assert_eq!(decoded_items.len(), 3);
                assert_eq!(decoded_items[0].media_type, "image");
                assert_eq!(decoded_items[1].media_type, "video");
                assert_eq!(decoded_items[2].media_type, "audio");
                assert_eq!(decoded_items[1].duration_seconds, Some(4));
            }
            _ => panic!("Expected SubmitWallPost"),
        }

        let response = BoardSyncResponse::WallPosts {
            posts: vec![WallPostData {
                post_id: "post-1".to_string(),
                author_peer_id: "author".to_string(),
                content_type: "mixed".to_string(),
                content_text: Some("media".to_string()),
                visibility: "public".to_string(),
                lamport_clock: 1,
                created_at: 1234567890,
                deleted_at: None,
                signature: vec![4; 64],
                media_hashes: media_hashes.clone(),
                stored_at: 1234567892,
                media_items,
            }],
            has_more: false,
        };

        let mut encoded = Vec::new();
        ciborium::into_writer(&response, &mut encoded).unwrap();
        let decoded: BoardSyncResponse = ciborium::from_reader(encoded.as_slice()).unwrap();

        match decoded {
            BoardSyncResponse::WallPosts { posts, has_more } => {
                assert!(!has_more);
                assert_eq!(posts[0].media_hashes, media_hashes);
                assert_eq!(posts[0].media_items[2].media_type, "audio");
                assert_eq!(posts[0].media_items[2].signature, vec![3; 64]);
            }
            _ => panic!("Expected WallPosts"),
        }
    }
}
