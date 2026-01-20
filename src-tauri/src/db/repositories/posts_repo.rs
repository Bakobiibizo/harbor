//! Posts repository for storing and retrieving wall/blog posts

use crate::db::Database;
use rusqlite::{params, Connection, Result as SqliteResult};

/// Post visibility
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostVisibility {
    /// Visible only to contacts with wall_read permission
    Contacts,
    /// Visible to everyone (public)
    Public,
}

impl PostVisibility {
    pub fn as_str(&self) -> &'static str {
        match self {
            PostVisibility::Contacts => "contacts",
            PostVisibility::Public => "public",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "contacts" => Some(PostVisibility::Contacts),
            "public" => Some(PostVisibility::Public),
            _ => None,
        }
    }
}

impl std::fmt::Display for PostVisibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A stored post
#[derive(Debug, Clone)]
pub struct Post {
    pub id: i64,
    pub post_id: String,
    pub author_peer_id: String,
    pub content_type: String,
    pub content_text: Option<String>,
    pub visibility: PostVisibility,
    pub lamport_clock: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
    pub is_local: bool,
    pub signature: Vec<u8>,
}

/// Data for inserting a new post
#[derive(Debug, Clone)]
pub struct PostData {
    pub post_id: String,
    pub author_peer_id: String,
    pub content_type: String,
    pub content_text: Option<String>,
    pub visibility: PostVisibility,
    pub lamport_clock: i64,
    pub created_at: i64,
    pub signature: Vec<u8>,
}

/// Post media metadata
#[derive(Debug, Clone)]
pub struct PostMedia {
    pub id: i64,
    pub post_id: String,
    pub media_hash: String,
    pub media_type: String,
    pub mime_type: String,
    pub file_name: String,
    pub file_size: i64,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub duration_seconds: Option<i32>,
    pub sort_order: i32,
}

/// Data for inserting post media
#[derive(Debug, Clone)]
pub struct PostMediaData {
    pub post_id: String,
    pub media_hash: String,
    pub media_type: String,
    pub mime_type: String,
    pub file_name: String,
    pub file_size: i64,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub duration_seconds: Option<i32>,
    pub sort_order: i32,
}

/// Repository for post operations
pub struct PostsRepository;

impl PostsRepository {
    /// Insert a new post
    pub fn insert_post(db: &Database, post: &PostData) -> SqliteResult<i64> {
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO posts (
                    post_id, author_peer_id, content_type, content_text,
                    visibility, lamport_clock, created_at, updated_at,
                    is_local, signature
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    post.post_id,
                    post.author_peer_id,
                    post.content_type,
                    post.content_text,
                    post.visibility.as_str(),
                    post.lamport_clock,
                    post.created_at,
                    post.created_at, // updated_at = created_at initially
                    1i32,            // is_local = true for posts we create
                    post.signature,
                ],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    /// Insert a remote post (received from network)
    pub fn insert_remote_post(db: &Database, post: &PostData) -> SqliteResult<i64> {
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO posts (
                    post_id, author_peer_id, content_type, content_text,
                    visibility, lamport_clock, created_at, updated_at,
                    is_local, signature
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    post.post_id,
                    post.author_peer_id,
                    post.content_type,
                    post.content_text,
                    post.visibility.as_str(),
                    post.lamport_clock,
                    post.created_at,
                    post.created_at,
                    0i32, // is_local = false for remote posts
                    post.signature,
                ],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    /// Get a post by ID
    pub fn get_by_post_id(db: &Database, post_id: &str) -> SqliteResult<Option<Post>> {
        db.with_connection(|conn| Self::get_by_post_id_inner(conn, post_id))
    }

    fn get_by_post_id_inner(conn: &Connection, post_id: &str) -> SqliteResult<Option<Post>> {
        let mut stmt = conn.prepare(
            "SELECT id, post_id, author_peer_id, content_type, content_text,
                    visibility, lamport_clock, created_at, updated_at,
                    deleted_at, is_local, signature
             FROM posts WHERE post_id = ?",
        )?;

        let mut rows = stmt.query([post_id])?;

        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_post(row)?))
        } else {
            Ok(None)
        }
    }

    fn row_to_post(row: &rusqlite::Row) -> SqliteResult<Post> {
        let visibility_str: String = row.get(5)?;
        let visibility =
            PostVisibility::from_str(&visibility_str).unwrap_or(PostVisibility::Contacts);

        Ok(Post {
            id: row.get(0)?,
            post_id: row.get(1)?,
            author_peer_id: row.get(2)?,
            content_type: row.get(3)?,
            content_text: row.get(4)?,
            visibility,
            lamport_clock: row.get(6)?,
            created_at: row.get(7)?,
            updated_at: row.get(8)?,
            deleted_at: row.get(9)?,
            is_local: row.get::<_, i32>(10)? != 0,
            signature: row.get(11)?,
        })
    }

    /// Get posts by author
    pub fn get_by_author(
        db: &Database,
        author_peer_id: &str,
        limit: i64,
        before_timestamp: Option<i64>,
    ) -> SqliteResult<Vec<Post>> {
        db.with_connection(|conn| {
            let mut posts = Vec::new();

            if let Some(before) = before_timestamp {
                let mut stmt = conn.prepare(
                    "SELECT id, post_id, author_peer_id, content_type, content_text,
                            visibility, lamport_clock, created_at, updated_at,
                            deleted_at, is_local, signature
                     FROM posts
                     WHERE author_peer_id = ? AND deleted_at IS NULL AND created_at < ?
                     ORDER BY created_at DESC
                     LIMIT ?",
                )?;
                let mut rows = stmt.query(params![author_peer_id, before, limit])?;
                while let Some(row) = rows.next()? {
                    posts.push(Self::row_to_post(row)?);
                }
            } else {
                let mut stmt = conn.prepare(
                    "SELECT id, post_id, author_peer_id, content_type, content_text,
                            visibility, lamport_clock, created_at, updated_at,
                            deleted_at, is_local, signature
                     FROM posts
                     WHERE author_peer_id = ? AND deleted_at IS NULL
                     ORDER BY created_at DESC
                     LIMIT ?",
                )?;
                let mut rows = stmt.query(params![author_peer_id, limit])?;
                while let Some(row) = rows.next()? {
                    posts.push(Self::row_to_post(row)?);
                }
            }

            Ok(posts)
        })
    }

    /// Get local posts (for own wall)
    pub fn get_local_posts(
        db: &Database,
        limit: i64,
        before_timestamp: Option<i64>,
    ) -> SqliteResult<Vec<Post>> {
        db.with_connection(|conn| {
            let mut posts = Vec::new();

            if let Some(before) = before_timestamp {
                let mut stmt = conn.prepare(
                    "SELECT id, post_id, author_peer_id, content_type, content_text,
                            visibility, lamport_clock, created_at, updated_at,
                            deleted_at, is_local, signature
                     FROM posts
                     WHERE is_local = 1 AND deleted_at IS NULL AND created_at < ?
                     ORDER BY created_at DESC
                     LIMIT ?",
                )?;
                let mut rows = stmt.query(params![before, limit])?;
                while let Some(row) = rows.next()? {
                    posts.push(Self::row_to_post(row)?);
                }
            } else {
                let mut stmt = conn.prepare(
                    "SELECT id, post_id, author_peer_id, content_type, content_text,
                            visibility, lamport_clock, created_at, updated_at,
                            deleted_at, is_local, signature
                     FROM posts
                     WHERE is_local = 1 AND deleted_at IS NULL
                     ORDER BY created_at DESC
                     LIMIT ?",
                )?;
                let mut rows = stmt.query(params![limit])?;
                while let Some(row) = rows.next()? {
                    posts.push(Self::row_to_post(row)?);
                }
            }

            Ok(posts)
        })
    }

    /// Update post content
    pub fn update_post(
        db: &Database,
        post_id: &str,
        content_text: Option<&str>,
        updated_at: i64,
        lamport_clock: i64,
    ) -> SqliteResult<bool> {
        db.with_connection(|conn| {
            let rows = conn.execute(
                "UPDATE posts SET content_text = ?, updated_at = ?, lamport_clock = ?
                 WHERE post_id = ?",
                params![content_text, updated_at, lamport_clock, post_id],
            )?;
            Ok(rows > 0)
        })
    }

    /// Soft delete a post
    pub fn delete_post(db: &Database, post_id: &str, deleted_at: i64) -> SqliteResult<bool> {
        db.with_connection(|conn| {
            let rows = conn.execute(
                "UPDATE posts SET deleted_at = ?
                 WHERE post_id = ? AND deleted_at IS NULL",
                params![deleted_at, post_id],
            )?;
            Ok(rows > 0)
        })
    }

    /// Check if a post exists
    pub fn post_exists(db: &Database, post_id: &str) -> SqliteResult<bool> {
        db.with_connection(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM posts WHERE post_id = ?",
                [post_id],
                |row| row.get(0),
            )?;
            Ok(count > 0)
        })
    }

    /// Add media to a post
    pub fn add_media(db: &Database, media: &PostMediaData) -> SqliteResult<()> {
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO post_media (
                    post_id, media_hash, media_type, mime_type,
                    file_name, file_size, width, height,
                    duration_seconds, sort_order
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    media.post_id,
                    media.media_hash,
                    media.media_type,
                    media.mime_type,
                    media.file_name,
                    media.file_size,
                    media.width,
                    media.height,
                    media.duration_seconds,
                    media.sort_order,
                ],
            )?;
            Ok(())
        })
    }

    /// Get media for a post
    pub fn get_post_media(db: &Database, post_id: &str) -> SqliteResult<Vec<PostMedia>> {
        db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, post_id, media_hash, media_type, mime_type,
                        file_name, file_size, width, height,
                        duration_seconds, sort_order
                 FROM post_media
                 WHERE post_id = ?
                 ORDER BY sort_order ASC",
            )?;

            let mut media = Vec::new();
            let mut rows = stmt.query([post_id])?;
            while let Some(row) = rows.next()? {
                media.push(PostMedia {
                    id: row.get(0)?,
                    post_id: row.get(1)?,
                    media_hash: row.get(2)?,
                    media_type: row.get(3)?,
                    mime_type: row.get(4)?,
                    file_name: row.get(5)?,
                    file_size: row.get(6)?,
                    width: row.get(7)?,
                    height: row.get(8)?,
                    duration_seconds: row.get(9)?,
                    sort_order: row.get(10)?,
                });
            }

            Ok(media)
        })
    }

    /// Record a post event (for event sourcing)
    pub fn record_post_event(
        db: &Database,
        event_id: &str,
        event_type: &str,
        post_id: &str,
        author_peer_id: &str,
        lamport_clock: i64,
        timestamp: i64,
        payload_cbor: &[u8],
        signature: &[u8],
    ) -> SqliteResult<i64> {
        db.with_connection(|conn| {
            let received_at = chrono::Utc::now().timestamp();
            conn.execute(
                "INSERT INTO post_events (
                    event_id, event_type, post_id, author_peer_id,
                    lamport_clock, timestamp, payload_cbor, signature, received_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    event_id,
                    event_type,
                    post_id,
                    author_peer_id,
                    lamport_clock,
                    timestamp,
                    payload_cbor,
                    signature,
                    received_at,
                ],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    /// Check if a post event exists (for deduplication)
    pub fn event_exists(db: &Database, event_id: &str) -> SqliteResult<bool> {
        db.with_connection(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM post_events WHERE event_id = ?",
                [event_id],
                |row| row.get(0),
            )?;
            Ok(count > 0)
        })
    }

    /// Get media hashes for a post
    pub fn get_media_hashes(db: &Database, post_id: &str) -> SqliteResult<Vec<String>> {
        db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT media_hash FROM post_media WHERE post_id = ? ORDER BY sort_order",
            )?;
            let mut hashes = Vec::new();
            let mut rows = stmt.query([post_id])?;
            while let Some(row) = rows.next()? {
                hashes.push(row.get(0)?);
            }
            Ok(hashes)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_db() -> Database {
        Database::in_memory().unwrap()
    }

    #[test]
    fn test_insert_and_get_post() {
        let db = create_test_db();

        let post = PostData {
            post_id: "post-123".to_string(),
            author_peer_id: "peer-a".to_string(),
            content_type: "text".to_string(),
            content_text: Some("Hello, world!".to_string()),
            visibility: PostVisibility::Contacts,
            lamport_clock: 1,
            created_at: 1234567890,
            signature: vec![1, 2, 3, 4],
        };

        let id = PostsRepository::insert_post(&db, &post).unwrap();
        assert!(id > 0);

        let stored = PostsRepository::get_by_post_id(&db, "post-123")
            .unwrap()
            .unwrap();
        assert_eq!(stored.post_id, "post-123");
        assert_eq!(stored.content_text, Some("Hello, world!".to_string()));
        assert_eq!(stored.visibility, PostVisibility::Contacts);
        assert!(stored.is_local);
    }

    #[test]
    fn test_update_post() {
        let db = create_test_db();

        let post = PostData {
            post_id: "post-456".to_string(),
            author_peer_id: "peer-a".to_string(),
            content_type: "text".to_string(),
            content_text: Some("Original".to_string()),
            visibility: PostVisibility::Contacts,
            lamport_clock: 1,
            created_at: 1234567890,
            signature: vec![1, 2, 3, 4],
        };

        PostsRepository::insert_post(&db, &post).unwrap();

        let updated =
            PostsRepository::update_post(&db, "post-456", Some("Updated content"), 1234567891, 2)
                .unwrap();
        assert!(updated);

        let stored = PostsRepository::get_by_post_id(&db, "post-456")
            .unwrap()
            .unwrap();
        assert_eq!(stored.content_text, Some("Updated content".to_string()));
        assert_eq!(stored.lamport_clock, 2);
    }

    #[test]
    fn test_delete_post() {
        let db = create_test_db();

        let post = PostData {
            post_id: "post-789".to_string(),
            author_peer_id: "peer-a".to_string(),
            content_type: "text".to_string(),
            content_text: Some("To be deleted".to_string()),
            visibility: PostVisibility::Contacts,
            lamport_clock: 1,
            created_at: 1234567890,
            signature: vec![1, 2, 3, 4],
        };

        PostsRepository::insert_post(&db, &post).unwrap();

        let deleted = PostsRepository::delete_post(&db, "post-789", 1234567892).unwrap();
        assert!(deleted);

        let stored = PostsRepository::get_by_post_id(&db, "post-789")
            .unwrap()
            .unwrap();
        assert!(stored.deleted_at.is_some());

        // Should not appear in get_by_author (filtered out)
        let posts = PostsRepository::get_by_author(&db, "peer-a", 10, None).unwrap();
        assert!(posts.is_empty());
    }

    #[test]
    fn test_post_media() {
        let db = create_test_db();

        let post = PostData {
            post_id: "post-media".to_string(),
            author_peer_id: "peer-a".to_string(),
            content_type: "text".to_string(),
            content_text: Some("Post with media".to_string()),
            visibility: PostVisibility::Contacts,
            lamport_clock: 1,
            created_at: 1234567890,
            signature: vec![1, 2, 3, 4],
        };

        PostsRepository::insert_post(&db, &post).unwrap();

        let media = PostMediaData {
            post_id: "post-media".to_string(),
            media_hash: "abc123".to_string(),
            media_type: "image".to_string(),
            mime_type: "image/jpeg".to_string(),
            file_name: "photo.jpg".to_string(),
            file_size: 12345,
            width: Some(800),
            height: Some(600),
            duration_seconds: None,
            sort_order: 0,
        };

        PostsRepository::add_media(&db, &media).unwrap();

        let stored_media = PostsRepository::get_post_media(&db, "post-media").unwrap();
        assert_eq!(stored_media.len(), 1);
        assert_eq!(stored_media[0].media_hash, "abc123");
        assert_eq!(stored_media[0].width, Some(800));

        let hashes = PostsRepository::get_media_hashes(&db, "post-media").unwrap();
        assert_eq!(hashes, vec!["abc123"]);
    }
}
