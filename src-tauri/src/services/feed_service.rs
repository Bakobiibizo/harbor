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

        // Get posts from all allowed authors
        let mut all_posts = Vec::new();
        for author in &allowed_authors {
            let posts = PostsRepository::get_by_author(&self.db, author, limit, before_timestamp)
                .map_err(|e| AppError::DatabaseString(e.to_string()))?;

            // Filter: if it's not our own post and visibility is "contacts",
            // make sure we have permission (we already checked above)
            for post in posts {
                if post.author_peer_id == identity.peer_id {
                    // Our own posts are always visible
                    all_posts.push(post);
                } else if post.visibility == PostVisibility::Public {
                    // Public posts are always visible
                    all_posts.push(post);
                } else if post.visibility == PostVisibility::Contacts {
                    // Contacts-only posts require WallRead permission (already verified above)
                    all_posts.push(post);
                }
            }
        }

        // Sort by created_at descending
        all_posts.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        // Apply limit
        all_posts.truncate(limit as usize);

        // Build a cache of display names for authors
        let mut display_name_cache: HashMap<String, Option<String>> = HashMap::new();

        // Convert to FeedItems
        let feed_items: Vec<FeedItem> = all_posts
            .into_iter()
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
