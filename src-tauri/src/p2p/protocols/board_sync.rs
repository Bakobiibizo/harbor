//! Board sync protocol types
//!
//! Used by both the relay server and the client for community board operations.

use serde::{Deserialize, Serialize};

/// Board sync request (wire protocol)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BoardSyncRequest {
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

/// Board sync response (wire protocol)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BoardSyncResponse {
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
    PostAccepted { post_id: String },
    /// Peer was registered
    PeerRegistered { peer_id: String },
    /// Post was deleted
    PostDeleted { post_id: String },
    /// Error response
    Error { error: String },
}
