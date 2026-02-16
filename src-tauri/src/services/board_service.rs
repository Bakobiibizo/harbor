//! Board service for managing community board interactions

use std::sync::Arc;
use uuid::Uuid;

use crate::db::{BoardsRepository, Database, UpsertBoardPostParams};
use crate::error::{AppError, Result};
use crate::services::{
    IdentityService, SignableBoardListRequest, SignableBoardPost, SignableBoardPostDelete,
    SignableBoardPostsRequest, SignableGetWallPosts, SignablePeerRegistration,
    SignableWallPostDelete, SignableWallPostSubmit,
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

/// A wall post submission request ready to be sent to the relay
#[derive(Debug, Clone)]
pub struct OutgoingWallPostSubmit {
    pub author_peer_id: String,
    pub post_id: String,
    pub content_type: String,
    pub content_text: Option<String>,
    pub visibility: String,
    pub lamport_clock: i64,
    pub created_at: i64,
    pub signature: Vec<u8>,
    pub timestamp: i64,
    pub request_signature: Vec<u8>,
}

/// A wall posts retrieval request ready to be sent
#[derive(Debug, Clone)]
pub struct OutgoingGetWallPosts {
    pub requester_peer_id: String,
    pub author_peer_id: String,
    pub since_lamport_clock: i64,
    pub limit: u32,
    pub timestamp: i64,
    pub signature: Vec<u8>,
}

/// A wall post delete request ready to be sent
#[derive(Debug, Clone)]
pub struct OutgoingWallPostDelete {
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
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

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
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

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
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

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
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

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
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

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

    // ===== Wall post relay operations =====

    /// Create a signed wall post submission for a relay
    #[allow(clippy::too_many_arguments)]
    pub fn create_wall_post_submit(
        &self,
        post_id: &str,
        content_type: &str,
        content_text: Option<&str>,
        visibility: &str,
        lamport_clock: i64,
        created_at: i64,
        post_signature: &[u8],
    ) -> Result<OutgoingWallPostSubmit> {
        let info = self
            .identity_service
            .get_identity_info()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        let now = chrono::Utc::now().timestamp();

        let signable = SignableWallPostSubmit {
            author_peer_id: info.peer_id.clone(),
            post_id: post_id.to_string(),
            content_type: content_type.to_string(),
            content_text: content_text.map(|t| t.to_string()),
            visibility: visibility.to_string(),
            lamport_clock,
            created_at,
            signature: post_signature.to_vec(),
            timestamp: now,
        };

        let request_signature = self.identity_service.sign(&signable)?;

        Ok(OutgoingWallPostSubmit {
            author_peer_id: info.peer_id,
            post_id: post_id.to_string(),
            content_type: content_type.to_string(),
            content_text: content_text.map(|t| t.to_string()),
            visibility: visibility.to_string(),
            lamport_clock,
            created_at,
            signature: post_signature.to_vec(),
            timestamp: now,
            request_signature,
        })
    }

    /// Create a signed request to get wall posts from a relay
    pub fn create_get_wall_posts_request(
        &self,
        author_peer_id: &str,
        since_lamport_clock: i64,
        limit: u32,
    ) -> Result<OutgoingGetWallPosts> {
        let info = self
            .identity_service
            .get_identity_info()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        let now = chrono::Utc::now().timestamp();
        let signable = SignableGetWallPosts {
            requester_peer_id: info.peer_id.clone(),
            author_peer_id: author_peer_id.to_string(),
            since_lamport_clock,
            limit,
            timestamp: now,
        };
        let signature = self.identity_service.sign(&signable)?;

        Ok(OutgoingGetWallPosts {
            requester_peer_id: info.peer_id,
            author_peer_id: author_peer_id.to_string(),
            since_lamport_clock,
            limit,
            timestamp: now,
            signature,
        })
    }

    /// Create a signed wall post delete request for a relay
    pub fn create_delete_wall_post_request(&self, post_id: &str) -> Result<OutgoingWallPostDelete> {
        let info = self
            .identity_service
            .get_identity_info()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        let now = chrono::Utc::now().timestamp();
        let signable = SignableWallPostDelete {
            author_peer_id: info.peer_id.clone(),
            post_id: post_id.to_string(),
            timestamp: now,
        };
        let signature = self.identity_service.sign(&signable)?;

        Ok(OutgoingWallPostDelete {
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
                &UpsertBoardPostParams {
                    post_id: &post.post_id,
                    board_id: &post.board_id,
                    relay_peer_id,
                    author_peer_id: &post.author_peer_id,
                    author_display_name: post.author_display_name.as_deref(),
                    content_type: &post.content_type,
                    content_text: post.content_text.as_deref(),
                    lamport_clock: post.lamport_clock,
                    created_at: post.created_at,
                    deleted_at: post.deleted_at,
                    signature: &post.signature,
                },
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CreateIdentityRequest;
    use crate::services::IdentityService;
    use std::sync::Arc;

    fn create_test_env() -> (
        BoardService,
        Arc<Database>,
        Arc<IdentityService>,
        String, // our peer_id
    ) {
        let db = Arc::new(Database::in_memory().unwrap());
        let identity_service = Arc::new(IdentityService::new(db.clone()));

        let info = identity_service
            .create_identity(CreateIdentityRequest {
                display_name: "Board User".to_string(),
                passphrase: "test-pass".to_string(),
                bio: None,
                passphrase_hint: None,
            })
            .unwrap();

        let board_service = BoardService::new(db.clone(), identity_service.clone());

        (board_service, db, identity_service, info.peer_id)
    }

    #[test]
    fn test_join_community() {
        let (service, _db, _identity, _peer_id) = create_test_env();

        service
            .join_community(
                "relay-peer-1",
                "/ip4/1.2.3.4/tcp/9000",
                Some("Test Community"),
            )
            .unwrap();

        let communities = service.get_communities().unwrap();
        assert_eq!(communities.len(), 1);
        assert_eq!(communities[0].relay_peer_id, "relay-peer-1");
        assert_eq!(
            communities[0].community_name,
            Some("Test Community".to_string())
        );
    }

    #[test]
    fn test_join_multiple_communities() {
        let (service, _db, _identity, _peer_id) = create_test_env();

        service
            .join_community("relay-1", "/ip4/1.2.3.4/tcp/9000", Some("Community 1"))
            .unwrap();
        service
            .join_community("relay-2", "/ip4/5.6.7.8/tcp/9001", Some("Community 2"))
            .unwrap();

        let communities = service.get_communities().unwrap();
        assert_eq!(communities.len(), 2);
    }

    #[test]
    fn test_leave_community() {
        let (service, _db, _identity, _peer_id) = create_test_env();

        service
            .join_community("relay-1", "/ip4/1.2.3.4/tcp/9000", Some("Community"))
            .unwrap();

        service.leave_community("relay-1").unwrap();

        let communities = service.get_communities().unwrap();
        assert!(communities.is_empty());
    }

    #[test]
    fn test_leave_nonexistent_community() {
        let (service, _db, _identity, _peer_id) = create_test_env();

        // Should not error, just no-op
        service.leave_community("nonexistent").unwrap();
    }

    #[test]
    fn test_store_and_get_boards() {
        let (service, _db, _identity, _peer_id) = create_test_env();

        service
            .join_community("relay-1", "/ip4/1.2.3.4/tcp/9000", Some("Community"))
            .unwrap();

        let boards = vec![
            (
                "board-1".to_string(),
                "General".to_string(),
                Some("General discussion".to_string()),
                true,
            ),
            ("board-2".to_string(), "Random".to_string(), None, false),
        ];

        service.store_boards("relay-1", &boards).unwrap();

        let stored_boards = service.get_boards("relay-1").unwrap();
        assert_eq!(stored_boards.len(), 2);

        // Default board should be first
        assert_eq!(stored_boards[0].name, "General");
        assert!(stored_boards[0].is_default);
    }

    #[test]
    fn test_store_and_get_board_posts() {
        let (service, _db, _identity, _peer_id) = create_test_env();

        service
            .join_community("relay-1", "/ip4/1.2.3.4/tcp/9000", None)
            .unwrap();

        let boards = vec![("board-1".to_string(), "General".to_string(), None, true)];
        service.store_boards("relay-1", &boards).unwrap();

        let posts = vec![
            StorableBoardPost {
                post_id: "bp-1".to_string(),
                board_id: "board-1".to_string(),
                author_peer_id: "author-1".to_string(),
                author_display_name: Some("Alice".to_string()),
                content_type: "text".to_string(),
                content_text: Some("Hello community!".to_string()),
                lamport_clock: 1,
                created_at: 1000,
                deleted_at: None,
                signature: vec![0u8; 64],
            },
            StorableBoardPost {
                post_id: "bp-2".to_string(),
                board_id: "board-1".to_string(),
                author_peer_id: "author-2".to_string(),
                author_display_name: Some("Bob".to_string()),
                content_type: "text".to_string(),
                content_text: Some("Hi everyone!".to_string()),
                lamport_clock: 2,
                created_at: 2000,
                deleted_at: None,
                signature: vec![0u8; 64],
            },
        ];

        service.store_board_posts("relay-1", &posts).unwrap();

        let stored_posts = service
            .get_board_posts("relay-1", "board-1", 10, None)
            .unwrap();
        assert_eq!(stored_posts.len(), 2);
    }

    #[test]
    fn test_get_board_posts_empty() {
        let (service, _db, _identity, _peer_id) = create_test_env();

        let posts = service
            .get_board_posts("relay-1", "board-1", 10, None)
            .unwrap();
        assert!(posts.is_empty());
    }

    #[test]
    fn test_sync_cursor() {
        let (service, _db, _identity, _peer_id) = create_test_env();

        // Initially no cursor
        let cursor = service.get_sync_cursor("relay-1", "board-1").unwrap();
        assert!(cursor.is_none());

        // Store board posts should update cursor
        service
            .join_community("relay-1", "/ip4/1.2.3.4/tcp/9000", None)
            .unwrap();
        let boards = vec![("board-1".to_string(), "General".to_string(), None, true)];
        service.store_boards("relay-1", &boards).unwrap();

        let posts = vec![StorableBoardPost {
            post_id: "bp-1".to_string(),
            board_id: "board-1".to_string(),
            author_peer_id: "author-1".to_string(),
            author_display_name: None,
            content_type: "text".to_string(),
            content_text: Some("Post".to_string()),
            lamport_clock: 1,
            created_at: 5000,
            deleted_at: None,
            signature: vec![0u8; 64],
        }];

        service.store_board_posts("relay-1", &posts).unwrap();

        let cursor = service.get_sync_cursor("relay-1", "board-1").unwrap();
        assert_eq!(cursor, Some(5000));
    }

    #[test]
    fn test_create_board_post_success() {
        let (service, _db, _identity, peer_id) = create_test_env();

        let post = service
            .create_board_post("board-1", "Hello board!")
            .unwrap();

        assert!(!post.post_id.is_empty());
        assert_eq!(post.board_id, "board-1");
        assert_eq!(post.author_peer_id, peer_id);
        assert_eq!(post.content_type, "text");
        assert_eq!(post.content_text, Some("Hello board!".to_string()));
        assert!(!post.signature.is_empty());
    }

    #[test]
    fn test_create_board_post_requires_identity() {
        let db = Arc::new(Database::in_memory().unwrap());
        let identity_service = Arc::new(IdentityService::new(db.clone()));
        let service = BoardService::new(db, identity_service);

        let result = service.create_board_post("board-1", "Hello");
        assert!(result.is_err());
    }

    #[test]
    fn test_create_peer_registration() {
        let (service, _db, _identity, peer_id) = create_test_env();

        let reg = service.create_peer_registration().unwrap();

        assert_eq!(reg.peer_id, peer_id);
        assert_eq!(reg.display_name, "Board User");
        assert!(!reg.public_key.is_empty());
        assert!(!reg.signature.is_empty());
    }

    #[test]
    fn test_create_list_boards_request() {
        let (service, _db, _identity, peer_id) = create_test_env();

        let req = service.create_list_boards_request().unwrap();

        assert_eq!(req.requester_peer_id, peer_id);
        assert!(!req.signature.is_empty());
    }

    #[test]
    fn test_create_get_board_posts_request() {
        let (service, _db, _identity, peer_id) = create_test_env();

        let req = service
            .create_get_board_posts_request("board-1", Some(1000), 50)
            .unwrap();

        assert_eq!(req.requester_peer_id, peer_id);
        assert_eq!(req.board_id, "board-1");
        assert_eq!(req.after_timestamp, Some(1000));
        assert_eq!(req.limit, 50);
        assert!(!req.signature.is_empty());
    }

    #[test]
    fn test_create_delete_post_request() {
        let (service, _db, _identity, peer_id) = create_test_env();

        let req = service.create_delete_post_request("post-123").unwrap();

        assert_eq!(req.post_id, "post-123");
        assert_eq!(req.author_peer_id, peer_id);
        assert!(!req.signature.is_empty());
    }

    #[test]
    fn test_upsert_community() {
        let (service, _db, _identity, _peer_id) = create_test_env();

        // Join community
        service
            .join_community("relay-1", "/ip4/1.2.3.4/tcp/9000", Some("Community V1"))
            .unwrap();

        // Re-join with updated name (upsert)
        service
            .join_community("relay-1", "/ip4/1.2.3.4/tcp/9001", Some("Community V2"))
            .unwrap();

        let communities = service.get_communities().unwrap();
        assert_eq!(communities.len(), 1);
        // Address should be updated
        assert_eq!(communities[0].relay_address, "/ip4/1.2.3.4/tcp/9001");
    }
}
