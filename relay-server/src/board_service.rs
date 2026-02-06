//! Server-side board logic for the relay server

use crate::db::RelayDatabase;
use tracing::{info, warn};

/// Service for processing board sync requests on the relay server
pub struct BoardService {
    db: RelayDatabase,
    community_name: String,
}

impl BoardService {
    pub fn new(db: RelayDatabase, community_name: String) -> Self {
        Self { db, community_name }
    }

    pub fn community_name(&self) -> &str {
        &self.community_name
    }

    /// Register a peer so they can post
    pub fn process_register_peer(
        &self,
        peer_id: &str,
        public_key: &[u8],
        display_name: &str,
    ) -> Result<(), String> {
        if self.db.is_peer_banned(peer_id).unwrap_or(false) {
            return Err("Peer is banned".to_string());
        }

        self.db
            .register_peer(peer_id, public_key, display_name)
            .map_err(|e| format!("Failed to register peer: {}", e))?;

        info!("Registered peer: {} ({})", display_name, peer_id);
        Ok(())
    }

    /// Submit a post to a board
    pub fn process_submit_post(
        &self,
        post_id: &str,
        board_id: &str,
        author_peer_id: &str,
        content_type: &str,
        content_text: Option<&str>,
        lamport_clock: u64,
        created_at: i64,
        signature: &[u8],
    ) -> Result<(), String> {
        // Check peer is known
        if !self.db.is_peer_known(author_peer_id).unwrap_or(false) {
            return Err("Peer not registered. Call RegisterPeer first.".to_string());
        }

        // Check not banned
        if self.db.is_peer_banned(author_peer_id).unwrap_or(false) {
            return Err("Peer is banned".to_string());
        }

        // Check board exists
        if !self.db.board_exists(board_id).unwrap_or(false) {
            return Err(format!("Board {} does not exist", board_id));
        }

        self.db
            .insert_post(
                post_id,
                board_id,
                author_peer_id,
                content_type,
                content_text,
                lamport_clock,
                created_at,
                signature,
            )
            .map_err(|e| format!("Failed to insert post: {}", e))?;

        info!("Post {} accepted from {} on board {}", post_id, author_peer_id, board_id);
        Ok(())
    }

    /// List all boards
    pub fn process_list_boards(&self) -> Result<Vec<crate::db::BoardRow>, String> {
        self.db
            .list_boards()
            .map_err(|e| format!("Failed to list boards: {}", e))
    }

    /// Get paginated posts for a board
    pub fn process_get_board_posts(
        &self,
        board_id: &str,
        after_timestamp: Option<i64>,
        limit: u32,
    ) -> Result<(Vec<crate::db::PostRow>, bool), String> {
        let clamped_limit = limit.min(100);
        let posts = self
            .db
            .get_board_posts(board_id, after_timestamp, clamped_limit + 1)
            .map_err(|e| format!("Failed to get board posts: {}", e))?;

        let has_more = posts.len() > clamped_limit as usize;
        let posts = if has_more {
            posts[..clamped_limit as usize].to_vec()
        } else {
            posts
        };

        Ok((posts, has_more))
    }

    /// Delete a post (author-only)
    pub fn process_delete_post(
        &self,
        post_id: &str,
        author_peer_id: &str,
    ) -> Result<(), String> {
        let deleted = self
            .db
            .delete_post(post_id, author_peer_id)
            .map_err(|e| format!("Failed to delete post: {}", e))?;

        if !deleted {
            warn!("Post {} not found or not owned by {}", post_id, author_peer_id);
            return Err("Post not found or not owned by you".to_string());
        }

        info!("Post {} deleted by {}", post_id, author_peer_id);
        Ok(())
    }
}
