//! Feed service for aggregating posts from contacts

use std::collections::HashMap;
use std::sync::Arc;

use crate::db::{Capability, Database, Post, PostVisibility, PostsRepository};
use crate::error::{AppError, Result};
use crate::services::{ContactsService, IdentityService, PermissionsService};

/// Service for managing the user's feed
pub struct FeedService {
    db: Arc<Database>,
    identity_service: Arc<IdentityService>,
    permissions_service: Arc<PermissionsService>,
    contacts_service: Arc<ContactsService>,
}

/// A feed item (post with additional context)
#[derive(Debug, Clone)]
pub struct FeedItem {
    pub post: Post,
    pub author_display_name: Option<String>,
}

impl FeedService {
    /// Create a new feed service
    pub fn new(
        db: Arc<Database>,
        identity_service: Arc<IdentityService>,
        permissions_service: Arc<PermissionsService>,
        contacts_service: Arc<ContactsService>,
    ) -> Self {
        Self {
            db,
            identity_service,
            permissions_service,
            contacts_service,
        }
    }

    /// Get the user's feed (posts from contacts who granted us WallRead)
    ///
    /// The feed includes:
    /// - Our own posts (always visible)
    /// - Posts from contacts who granted us WallRead permission
    /// - Only non-deleted posts
    /// - Sorted by creation time, newest first
    pub fn get_feed(&self, limit: i64, before_timestamp: Option<i64>) -> Result<Vec<FeedItem>> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        // Get all peer IDs who granted us WallRead
        let permissions = self.permissions_service.get_received_permissions()?;
        let mut allowed_authors: Vec<String> = permissions
            .iter()
            .filter(|p| p.capability == "wall_read" && p.revoked_at.is_none())
            .map(|p| p.issuer_peer_id.clone())
            .collect();

        // Always include our own posts
        allowed_authors.push(identity.peer_id.clone());

        // Deduplicate
        allowed_authors.sort();
        allowed_authors.dedup();

        // Get posts from all allowed authors in a single efficient query
        // sorted by created_at DESC with proper limit applied globally
        let all_posts =
            PostsRepository::get_feed_posts(&self.db, &allowed_authors, limit, before_timestamp)
                .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        // Build a cache of display names for authors
        let mut display_name_cache: HashMap<String, Option<String>> = HashMap::new();

        // Convert to FeedItems with visibility filtering
        let feed_items: Vec<FeedItem> = all_posts
            .into_iter()
            .filter(|post| {
                // Our own posts are always visible
                if post.author_peer_id == identity.peer_id {
                    return true;
                }
                // Public posts are always visible
                if post.visibility == PostVisibility::Public {
                    return true;
                }
                // Contacts-only posts require WallRead permission
                // (already verified via allowed_authors list)
                if post.visibility == PostVisibility::Contacts {
                    return true;
                }
                false
            })
            .map(|post| {
                // Look up display name from cache or contacts
                let author_display_name = display_name_cache
                    .entry(post.author_peer_id.clone())
                    .or_insert_with(|| {
                        // Check if it's our own post
                        if post.author_peer_id == identity.peer_id {
                            Some(identity.display_name.clone())
                        } else {
                            // Look up from contacts
                            self.contacts_service
                                .get_contact(&post.author_peer_id)
                                .ok()
                                .flatten()
                                .map(|c| c.display_name)
                        }
                    })
                    .clone();

                FeedItem {
                    post,
                    author_display_name,
                }
            })
            .collect();

        Ok(feed_items)
    }

    /// Get posts from a specific author (their wall)
    /// Requires WallRead permission if not our own posts
    pub fn get_wall(
        &self,
        author_peer_id: &str,
        limit: i64,
        before_timestamp: Option<i64>,
    ) -> Result<Vec<FeedItem>> {
        let identity = self
            .identity_service
            .get_identity()?
            .ok_or_else(|| AppError::IdentityNotFound("No identity".to_string()))?;

        // Check permission if not our own wall
        if author_peer_id != identity.peer_id
            && !self
                .permissions_service
                .we_have_capability(author_peer_id, Capability::WallRead)?
        {
            return Err(AppError::PermissionDenied(
                "No permission to view this wall".to_string(),
            ));
        }

        let posts =
            PostsRepository::get_by_author(&self.db, author_peer_id, limit, before_timestamp)
                .map_err(|e| AppError::DatabaseString(e.to_string()))?;

        // Look up display name for the author
        let author_display_name = if author_peer_id == identity.peer_id {
            Some(identity.display_name.clone())
        } else {
            self.contacts_service
                .get_contact(author_peer_id)
                .ok()
                .flatten()
                .map(|c| c.display_name)
        };

        // All posts are visible (permission was verified above)
        let feed_items: Vec<FeedItem> = posts
            .into_iter()
            .map(|post| FeedItem {
                post,
                author_display_name: author_display_name.clone(),
            })
            .collect();

        Ok(feed_items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{ContactData, ContactsRepository, PostData, PostsRepository};
    use crate::models::CreateIdentityRequest;
    use crate::services::{ContactsService, IdentityService, PermissionsService};
    use std::sync::Arc;

    fn create_test_env() -> (
        FeedService,
        Arc<Database>,
        Arc<IdentityService>,
        Arc<PermissionsService>,
        String, // our peer_id
    ) {
        let db = Arc::new(Database::in_memory().unwrap());
        let identity_service = Arc::new(IdentityService::new(db.clone()));
        let contacts_service = Arc::new(ContactsService::new(db.clone(), identity_service.clone()));
        let permissions_service = Arc::new(PermissionsService::new(
            db.clone(),
            identity_service.clone(),
        ));

        let info = identity_service
            .create_identity(CreateIdentityRequest {
                display_name: "Feed User".to_string(),
                passphrase: "test-pass".to_string(),
                bio: None,
                passphrase_hint: None,
            })
            .unwrap();

        let feed_service = FeedService::new(
            db.clone(),
            identity_service.clone(),
            permissions_service.clone(),
            contacts_service.clone(),
        );

        (
            feed_service,
            db,
            identity_service,
            permissions_service,
            info.peer_id,
        )
    }

    /// Helper to insert a post directly into the database
    fn insert_test_post(
        db: &Database,
        post_id: &str,
        author: &str,
        content: &str,
        created_at: i64,
        visibility: PostVisibility,
    ) {
        let post_data = PostData {
            post_id: post_id.to_string(),
            author_peer_id: author.to_string(),
            content_type: "text".to_string(),
            content_text: Some(content.to_string()),
            visibility,
            lamport_clock: 1,
            created_at,
            signature: vec![0u8; 64],
        };
        PostsRepository::insert_post(db, &post_data).unwrap();
    }

    #[test]
    fn test_get_feed_own_posts() {
        let (service, db, _identity, _perms, peer_id) = create_test_env();

        // Insert our own posts
        insert_test_post(
            &db,
            "post-1",
            &peer_id,
            "My post 1",
            1000,
            PostVisibility::Public,
        );
        insert_test_post(
            &db,
            "post-2",
            &peer_id,
            "My post 2",
            2000,
            PostVisibility::Contacts,
        );

        let feed = service.get_feed(10, None).unwrap();
        assert_eq!(feed.len(), 2);

        // Most recent first
        assert_eq!(feed[0].post.post_id, "post-2");
        assert_eq!(feed[1].post.post_id, "post-1");
    }

    #[test]
    fn test_get_feed_empty() {
        let (service, _db, _identity, _perms, _peer_id) = create_test_env();

        let feed = service.get_feed(10, None).unwrap();
        assert!(feed.is_empty());
    }

    #[test]
    fn test_get_feed_with_limit() {
        let (service, db, _identity, _perms, peer_id) = create_test_env();

        for i in 0..5 {
            insert_test_post(
                &db,
                &format!("post-{}", i),
                &peer_id,
                &format!("Post {}", i),
                1000 + i,
                PostVisibility::Public,
            );
        }

        let feed = service.get_feed(3, None).unwrap();
        assert_eq!(feed.len(), 3);
    }

    #[test]
    fn test_get_feed_requires_identity() {
        let db = Arc::new(Database::in_memory().unwrap());
        let identity_service = Arc::new(IdentityService::new(db.clone()));
        let contacts_service = Arc::new(ContactsService::new(db.clone(), identity_service.clone()));
        let permissions_service = Arc::new(PermissionsService::new(
            db.clone(),
            identity_service.clone(),
        ));

        let feed_service =
            FeedService::new(db, identity_service, permissions_service, contacts_service);

        let result = feed_service.get_feed(10, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_feed_includes_display_name() {
        let (service, db, _identity, _perms, peer_id) = create_test_env();

        insert_test_post(
            &db,
            "post-1",
            &peer_id,
            "My post",
            1000,
            PostVisibility::Public,
        );

        let feed = service.get_feed(10, None).unwrap();
        assert_eq!(feed.len(), 1);
        assert_eq!(feed[0].author_display_name, Some("Feed User".to_string()));
    }

    #[test]
    fn test_get_wall_own_posts() {
        let (service, db, _identity, _perms, peer_id) = create_test_env();

        insert_test_post(
            &db,
            "post-1",
            &peer_id,
            "Wall post 1",
            1000,
            PostVisibility::Public,
        );
        insert_test_post(
            &db,
            "post-2",
            &peer_id,
            "Wall post 2",
            2000,
            PostVisibility::Contacts,
        );

        let wall = service.get_wall(&peer_id, 10, None).unwrap();
        assert_eq!(wall.len(), 2);
        assert_eq!(wall[0].author_display_name, Some("Feed User".to_string()));
    }

    #[test]
    fn test_get_wall_other_user_no_permission() {
        let (service, _db, _identity, _perms, _peer_id) = create_test_env();

        let result = service.get_wall("12D3KooWOtherPeer", 10, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_wall_other_user_with_permission() {
        let (service, db, _identity, _permissions, peer_id) = create_test_env();

        let other_peer = "12D3KooWOtherPeer".to_string();

        // Add the other peer as a contact
        let contact_data = ContactData {
            peer_id: other_peer.clone(),
            public_key: vec![1u8; 32],
            x25519_public: vec![2u8; 32],
            display_name: "Other Peer".to_string(),
            avatar_hash: None,
            bio: None,
        };
        ContactsRepository::add_contact(&db, &contact_data).unwrap();

        // Insert a post from the other peer
        insert_test_post(
            &db,
            "other-post-1",
            &other_peer,
            "Other post",
            1000,
            PostVisibility::Public,
        );

        // Grant WallRead permission from the other peer to us
        // We need to simulate that the other peer granted us WallRead.
        // Use raw repo insert since we can't sign with other peer's key in this test.
        use crate::db::{GrantData, PermissionsRepository};
        let grant_data = GrantData {
            grant_id: "grant-wr-1".to_string(),
            issuer_peer_id: other_peer.clone(),
            subject_peer_id: peer_id.clone(),
            capability: "wall_read".to_string(),
            scope_json: None,
            lamport_clock: 1,
            issued_at: 1000,
            expires_at: None,
            payload_cbor: vec![0],
            signature: vec![0],
        };
        PermissionsRepository::upsert_grant(&db, &grant_data).unwrap();

        let wall = service.get_wall(&other_peer, 10, None).unwrap();
        assert_eq!(wall.len(), 1);
        assert_eq!(wall[0].post.content_text, Some("Other post".to_string()));
    }

    #[test]
    fn test_feed_sorted_chronologically() {
        let (service, db, _identity, _perms, peer_id) = create_test_env();

        insert_test_post(
            &db,
            "post-old",
            &peer_id,
            "Old post",
            1000,
            PostVisibility::Public,
        );
        insert_test_post(
            &db,
            "post-mid",
            &peer_id,
            "Middle post",
            2000,
            PostVisibility::Public,
        );
        insert_test_post(
            &db,
            "post-new",
            &peer_id,
            "New post",
            3000,
            PostVisibility::Public,
        );

        let feed = service.get_feed(10, None).unwrap();
        assert_eq!(feed.len(), 3);
        assert_eq!(feed[0].post.post_id, "post-new");
        assert_eq!(feed[1].post.post_id, "post-mid");
        assert_eq!(feed[2].post.post_id, "post-old");
    }
}
