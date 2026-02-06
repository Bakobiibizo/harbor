//! Board service for managing community board interactions

use std::sync::Arc;
use uuid::Uuid;

use crate::db::{BoardsRepository, Database};
use crate::error::{AppError, Result};
use crate::services::{
    IdentityService, SignableBoardListRequest, SignableBoardPost, SignableBoardPostDelete,
    SignableBoardPostsRequest, SignablePeerRegistration,
};

/// Service for managing community board operations
pub struct BoardService {
    db: Arc<Database>,
    identity_service: Arc<IdentityService>,
}

/// A board post ready to be sent to the relay
#[derive(Debug, Clone)]
pub struct OutgoingBoardPost {
    pub post_id: String,
    pub board_id: String,
    pub author_peer_id: String,
    pub content_type: String,
    pub content_text: Option<String>,
    pub lamport_clock: u64,
    pub created_at: i64,
    pub signature: Vec<u8>,
}

/// A peer registration request ready to be sent to the relay
#[derive(Debug, Clone)]
pub struct OutgoingPeerRegistration {
    pub peer_id: String,
    pub public_key: Vec<u8>,
    pub display_name: String,
    pub timestamp: i64,
    pub signature: Vec<u8>,
}

/// A board list request ready to be sent
#[derive(Debug, Clone)]
pub struct OutgoingBoardListRequest {
    pub requester_peer_id: String,
    pub timestamp: i64,
    pub signature: Vec<u8>,
}

/// A board posts request ready to be sent
#[derive(Debug, Clone)]
pub struct OutgoingBoardPostsRequest {
    pub requester_peer_id: String,
    pub board_id: String,
    pub after_timestamp: Option<i64>,
    pub limit: u32,
    pub timestamp: i64,
    pub signature: Vec<u8>,
}

/// A board post delete request
#[derive(Debug, Clone)]
pub struct OutgoingBoardPostDelete {
    pub post_id: String,
    pub author_peer_id: String,
    pub timestamp: i64,
    pub signature: Vec<u8>,
}

impl BoardService {
    pub fn new(db: Arc<Database>, identity_service: Arc<IdentityService>) -> Self {
        Self {
            db,
            identity_service,
        }
    }

    /// Create a signed board post for submission to a relay
    pub fn create_board_post(
        &self,
        board_id: &str,
        content_text: &str,
    ) -> Result<OutgoingBoardPost> {
        let info = self
            .identity_service
            .get_identity_info()?
            .ok_or_else(|| AppError::NotFound("No identity".to_string()))?;

        let post_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();
        let lamport_clock = self.db.next_lamport_clock(&info.peer_id)? as u64;

        let signable = SignableBoardPost {
            post_id: post_id.clone(),
            board_id: board_id.to_string(),
            author_peer_id: info.peer_id.clone(),
            content_type: "text".to_string(),
            content_text: Some(content_text.to_string()),
            lamport_clock,
            created_at: now,
        };

        let signature = self.identity_service.sign(&signable)?;

        Ok(OutgoingBoardPost {
            post_id,
            board_id: board_id.to_string(),
            author_peer_id: info.peer_id,
            content_type: "text".to_string(),
            content_text: Some(content_text.to_string()),
            lamport_clock,
            created_at: now,
            signature,
        })
    }

    /// Create a signed peer registration for a relay
    pub fn create_peer_registration(&self) -> Result<OutgoingPeerRegistration> {
        let info = self
            .identity_service
            .get_identity_info()?
            .ok_or_else(|| AppError::NotFound("No identity".to_string()))?;

        let now = chrono::Utc::now().timestamp();

        let signable = SignablePeerRegistration {
            peer_id: info.peer_id.clone(),
            display_name: info.display_name.clone(),
            timestamp: now,
        };

        let signature = self.identity_service.sign(&signable)?;

        // Decode public key from base64
        let public_key =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &info.public_key)
                .map_err(|e| AppError::Internal(format!("Failed to decode public key: {}", e)))?;

        Ok(OutgoingPeerRegistration {
            peer_id: info.peer_id,
            public_key,
            display_name: info.display_name,
            timestamp: now,
            signature,
        })
    }

    /// Create a signed board list request
    pub fn create_list_boards_request(&self) -> Result<OutgoingBoardListRequest> {
        let info = self
            .identity_service
            .get_identity_info()?
            .ok_or_else(|| AppError::NotFound("No identity".to_string()))?;

        let now = chrono::Utc::now().timestamp();
        let signable = SignableBoardListRequest {
            requester_peer_id: info.peer_id.clone(),
            timestamp: now,
        };
        let signature = self.identity_service.sign(&signable)?;

        Ok(OutgoingBoardListRequest {
            requester_peer_id: info.peer_id,
            timestamp: now,
            signature,
        })
    }

    /// Create a signed board posts request
    pub fn create_get_board_posts_request(
        &self,
        board_id: &str,
        after_timestamp: Option<i64>,
        limit: u32,
    ) -> Result<OutgoingBoardPostsRequest> {
        let info = self
            .identity_service
            .get_identity_info()?
            .ok_or_else(|| AppError::NotFound("No identity".to_string()))?;

        let now = chrono::Utc::now().timestamp();
        let signable = SignableBoardPostsRequest {
            requester_peer_id: info.peer_id.clone(),
            board_id: board_id.to_string(),
            timestamp: now,
        };
        let signature = self.identity_service.sign(&signable)?;

        Ok(OutgoingBoardPostsRequest {
            requester_peer_id: info.peer_id,
            board_id: board_id.to_string(),
            after_timestamp,
            limit,
            timestamp: now,
            signature,
        })
    }

    /// Create a signed board post delete request
    pub fn create_delete_post_request(&self, post_id: &str) -> Result<OutgoingBoardPostDelete> {
        let info = self
            .identity_service
            .get_identity_info()?
            .ok_or_else(|| AppError::NotFound("No identity".to_string()))?;

        let now = chrono::Utc::now().timestamp();
        let signable = SignableBoardPostDelete {
            post_id: post_id.to_string(),
            author_peer_id: info.peer_id.clone(),
            timestamp: now,
        };
        let signature = self.identity_service.sign(&signable)?;

        Ok(OutgoingBoardPostDelete {
            post_id: post_id.to_string(),
            author_peer_id: info.peer_id,
            timestamp: now,
            signature,
        })
    }

    // ===== Local data operations =====

    /// Join a community by storing it locally
    pub fn join_community(
        &self,
        relay_peer_id: &str,
        relay_address: &str,
        community_name: Option<&str>,
    ) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        BoardsRepository::upsert_relay_community(
            &self.db,
            relay_peer_id,
            relay_address,
            community_name,
            now,
        )
        .map_err(AppError::Database)
    }

    /// Leave a community
    pub fn leave_community(&self, relay_peer_id: &str) -> Result<()> {
        BoardsRepository::delete_relay_community(&self.db, relay_peer_id)
            .map_err(AppError::Database)?;
        Ok(())
    }

    /// Get all joined communities
    pub fn get_communities(&self) -> Result<Vec<crate::db::RelayCommunity>> {
        BoardsRepository::get_relay_communities(&self.db).map_err(AppError::Database)
    }

    /// Get boards for a relay (from local cache)
    pub fn get_boards(&self, relay_peer_id: &str) -> Result<Vec<crate::db::Board>> {
        BoardsRepository::get_boards_for_relay(&self.db, relay_peer_id).map_err(AppError::Database)
    }

    /// Get board posts from local cache
    pub fn get_board_posts(
        &self,
        relay_peer_id: &str,
        board_id: &str,
        limit: i64,
        before_timestamp: Option<i64>,
    ) -> Result<Vec<crate::db::BoardPost>> {
        BoardsRepository::get_board_posts(
            &self.db,
            board_id,
            relay_peer_id,
            limit,
            before_timestamp,
        )
        .map_err(AppError::Database)
    }

    /// Store boards received from a relay
    pub fn store_boards(
        &self,
        relay_peer_id: &str,
        boards: &[(String, String, Option<String>, bool)],
    ) -> Result<()> {
        for (board_id, name, description, is_default) in boards {
            BoardsRepository::upsert_board(
                &self.db,
                board_id,
                relay_peer_id,
                name,
                description.as_deref(),
                *is_default,
            )
            .map_err(AppError::Database)?;
        }
        Ok(())
    }

    /// Store board posts received from a relay
    pub fn store_board_posts(
        &self,
        relay_peer_id: &str,
        posts: &[StorableBoardPost],
    ) -> Result<()> {
        for post in posts {
            BoardsRepository::upsert_board_post(
                &self.db,
                &post.post_id,
                &post.board_id,
                relay_peer_id,
                &post.author_peer_id,
                post.author_display_name.as_deref(),
                &post.content_type,
                post.content_text.as_deref(),
                post.lamport_clock,
                post.created_at,
                post.deleted_at,
                &post.signature,
            )
            .map_err(AppError::Database)?;

            // Update sync cursor
            BoardsRepository::update_board_sync_cursor(
                &self.db,
                relay_peer_id,
                &post.board_id,
                post.created_at,
            )
            .map_err(AppError::Database)?;
        }

        // Update community sync time
        BoardsRepository::update_community_sync_time(&self.db, relay_peer_id)
            .map_err(AppError::Database)?;

        Ok(())
    }

    /// Get sync cursor for a board
    pub fn get_sync_cursor(&self, relay_peer_id: &str, board_id: &str) -> Result<Option<i64>> {
        BoardsRepository::get_board_sync_cursor(&self.db, relay_peer_id, board_id)
            .map_err(AppError::Database)
    }
}

/// A board post to be stored locally (from relay response)
#[derive(Debug, Clone)]
pub struct StorableBoardPost {
    pub post_id: String,
    pub board_id: String,
    pub author_peer_id: String,
    pub author_display_name: Option<String>,
    pub content_type: String,
    pub content_text: Option<String>,
    pub lamport_clock: i64,
    pub created_at: i64,
    pub deleted_at: Option<i64>,
    pub signature: Vec<u8>,
}
