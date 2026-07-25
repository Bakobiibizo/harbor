//! Posts repository for storing and retrieving wall/blog posts

use crate::db::Database;
use rusqlite::{params, Connection, OptionalExtension, Result as SqliteResult, Transaction};

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
    pub relay_status: String,
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
    pub signature: Vec<u8>,
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
    pub signature: Vec<u8>,
}

/// Aggregated visibility counts for an author's posts.
///
/// Computed entirely in SQL via `COUNT`/`GROUP BY` -- no post rows are
/// transferred to Rust.
#[derive(Debug, Clone)]
pub struct VisibilityCounts {
    /// Total number of non-deleted posts
    pub total_posts: usize,
    /// Number of posts with `public` visibility
    pub public_posts: usize,
    /// Number of posts with `contacts` visibility
    pub contacts_only_posts: usize,
}

/// Repository for post operations
/// Parameters for recording a post event
pub struct RecordPostEventParams<'a> {
    pub event_id: &'a str,
    pub event_type: &'a str,
    pub post_id: &'a str,
    pub author_peer_id: &'a str,
    pub lamport_clock: i64,
    pub timestamp: i64,
    pub payload_cbor: &'a [u8],
    pub signature: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostRelayOutboxState {
    Queued,
    InFlight,
    Acknowledged,
    Conflict,
    Failed,
}

impl PostRelayOutboxState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::InFlight => "in_flight",
            Self::Acknowledged => "acknowledged",
            Self::Conflict => "conflict",
            Self::Failed => "failed",
        }
    }

    fn parse(value: &str) -> SqliteResult<Self> {
        match value {
            "queued" => Ok(Self::Queued),
            "in_flight" => Ok(Self::InFlight),
            "acknowledged" => Ok(Self::Acknowledged),
            "conflict" => Ok(Self::Conflict),
            "failed" => Ok(Self::Failed),
            _ => Err(rusqlite::Error::InvalidQuery),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PostRelayOutboxEntry {
    pub event_id: String,
    pub post_id: String,
    pub mutation_type: String,
    pub payload_cbor: Vec<u8>,
    pub state: PostRelayOutboxState,
    pub attempt_count: i64,
    pub max_attempts: i64,
    pub next_attempt_at: i64,
    pub attempt_deadline_at: Option<i64>,
    pub relay_peer_id: Option<String>,
    pub last_error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub terminal_at: Option<i64>,
}

pub struct EnqueuePostRelayMutation<'a> {
    pub event_id: &'a str,
    pub post_id: &'a str,
    pub mutation_type: &'a str,
    pub payload_cbor: &'a [u8],
    pub created_at: i64,
}

pub struct PostsRepository;

impl PostsRepository {
    pub(crate) fn next_lamport_in_transaction(
        transaction: &Transaction<'_>,
        author_peer_id: &str,
    ) -> SqliteResult<i64> {
        let current = transaction
            .query_row(
                "SELECT current_value FROM lamport_clocks WHERE author_peer_id = ?1",
                [author_peer_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .unwrap_or(0);
        let next = current
            .checked_add(1)
            .ok_or(rusqlite::Error::IntegralValueOutOfRange(0, current))?;
        transaction.execute(
            "INSERT INTO lamport_clocks(author_peer_id, current_value) VALUES (?1, ?2)
             ON CONFLICT(author_peer_id) DO UPDATE SET current_value = excluded.current_value",
            params![author_peer_id, next],
        )?;
        Ok(next)
    }

    pub(crate) fn insert_local_post_in_transaction(
        transaction: &Transaction<'_>,
        post: &PostData,
    ) -> SqliteResult<()> {
        transaction.execute(
            "INSERT INTO posts(
                post_id, author_peer_id, content_type, content_text, visibility,
                lamport_clock, created_at, updated_at, is_local, relay_status, signature
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?7,1,'local_pending',?8)",
            params![
                post.post_id,
                post.author_peer_id,
                post.content_type,
                post.content_text,
                post.visibility.as_str(),
                post.lamport_clock,
                post.created_at,
                post.signature,
            ],
        )?;
        Ok(())
    }

    pub(crate) fn add_media_in_transaction(
        transaction: &Transaction<'_>,
        media: &PostMediaData,
    ) -> SqliteResult<()> {
        transaction.execute(
            "INSERT INTO post_media(
                post_id, media_hash, media_type, mime_type, file_name, file_size,
                width, height, duration_seconds, sort_order, signature
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
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
                media.signature,
            ],
        )?;
        Ok(())
    }

    pub(crate) fn update_local_post_in_transaction(
        transaction: &Transaction<'_>,
        post_id: &str,
        content_text: Option<&str>,
        updated_at: i64,
        lamport_clock: i64,
        signature: &[u8],
    ) -> SqliteResult<bool> {
        Ok(transaction.execute(
            "UPDATE posts SET content_text=?1, updated_at=?2, lamport_clock=?3,
                    deleted_at=NULL, relay_status='local_pending', signature=?4
             WHERE post_id=?5 AND deleted_at IS NULL",
            params![content_text, updated_at, lamport_clock, signature, post_id],
        )? > 0)
    }

    pub(crate) fn delete_local_post_in_transaction(
        transaction: &Transaction<'_>,
        post_id: &str,
        deleted_at: i64,
        lamport_clock: i64,
        signature: &[u8],
    ) -> SqliteResult<bool> {
        Ok(transaction.execute(
            "UPDATE posts SET deleted_at=?1, updated_at=?1, lamport_clock=?2,
                    relay_status='local_pending', signature=?3
             WHERE post_id=?4 AND deleted_at IS NULL",
            params![deleted_at, lamport_clock, signature, post_id],
        )? > 0)
    }

    pub(crate) fn record_post_event_in_transaction(
        transaction: &Transaction<'_>,
        event: &RecordPostEventParams<'_>,
    ) -> SqliteResult<()> {
        transaction.execute(
            "INSERT INTO post_events(
                event_id,event_type,post_id,author_peer_id,lamport_clock,
                timestamp,payload_cbor,signature,received_at
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?6)",
            params![
                event.event_id,
                event.event_type,
                event.post_id,
                event.author_peer_id,
                event.lamport_clock,
                event.timestamp,
                event.payload_cbor,
                event.signature,
            ],
        )?;
        Ok(())
    }

    pub(crate) fn enqueue_post_relay_in_transaction(
        transaction: &Transaction<'_>,
        mutation: &EnqueuePostRelayMutation<'_>,
    ) -> SqliteResult<()> {
        transaction.execute(
            "INSERT INTO post_relay_outbox(
                event_id,post_id,mutation_type,payload_cbor,state,attempt_count,
                max_attempts,next_attempt_at,created_at,updated_at
             ) VALUES (?1,?2,?3,?4,'queued',0,8,?5,?5,?5)",
            params![
                mutation.event_id,
                mutation.post_id,
                mutation.mutation_type,
                mutation.payload_cbor,
                mutation.created_at,
            ],
        )?;
        Ok(())
    }

    /// Insert a new post
    pub fn insert_post(db: &Database, post: &PostData) -> SqliteResult<i64> {
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO posts (
                    post_id, author_peer_id, content_type, content_text,
                    visibility, lamport_clock, created_at, updated_at,
                    is_local, relay_status, signature
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
                    "local_pending",
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
                    is_local, relay_status, signature
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
                    "relay_acknowledged",
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

    pub(crate) fn get_by_post_id_inner(
        conn: &Connection,
        post_id: &str,
    ) -> SqliteResult<Option<Post>> {
        let mut stmt = conn.prepare(
            "SELECT id, post_id, author_peer_id, content_type, content_text,
                    visibility, lamport_clock, created_at, updated_at,
                    deleted_at, is_local, relay_status, signature
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
            relay_status: row.get(11)?,
            signature: row.get(12)?,
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
                            deleted_at, is_local, relay_status, signature
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
                            deleted_at, is_local, relay_status, signature
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

    /// Get posts by author with lamport_clock greater than the given cursor value.
    /// Results are ordered by lamport_clock ascending so the caller receives posts
    /// in causal order, which is the expected ordering for sync cursor advancement.
    pub fn get_by_author_after_cursor(
        db: &Database,
        author_peer_id: &str,
        cursor: i64,
        limit: i64,
    ) -> SqliteResult<Vec<Post>> {
        db.with_connection(|conn| {
            let mut posts = Vec::new();
            let mut stmt = conn.prepare(
                "SELECT id, post_id, author_peer_id, content_type, content_text,
                        visibility, lamport_clock, created_at, updated_at,
                        deleted_at, is_local, relay_status, signature
                 FROM posts
                 WHERE author_peer_id = ? AND deleted_at IS NULL AND lamport_clock > ?
                 ORDER BY lamport_clock ASC
                 LIMIT ?",
            )?;
            let mut rows = stmt.query(params![author_peer_id, cursor, limit])?;
            while let Some(row) = rows.next()? {
                posts.push(Self::row_to_post(row)?);
            }
            Ok(posts)
        })
    }

    /// Get posts by author with lamport_clock greater than the given cursor,
    /// optionally filtered to a specific visibility.
    pub fn get_by_author_after_cursor_with_visibility(
        db: &Database,
        author_peer_id: &str,
        cursor: i64,
        visibility: Option<PostVisibility>,
        limit: i64,
    ) -> SqliteResult<Vec<Post>> {
        db.with_connection(|conn| {
            let mut posts = Vec::new();
            match visibility {
                Some(vis) => {
                    let mut stmt = conn.prepare(
                        "SELECT id, post_id, author_peer_id, content_type, content_text,
                                visibility, lamport_clock, created_at, updated_at,
                                deleted_at, is_local, relay_status, signature
                         FROM posts
                         WHERE author_peer_id = ? AND deleted_at IS NULL
                               AND lamport_clock > ? AND visibility = ?
                         ORDER BY lamport_clock ASC
                         LIMIT ?",
                    )?;
                    let mut rows =
                        stmt.query(params![author_peer_id, cursor, vis.as_str(), limit])?;
                    while let Some(row) = rows.next()? {
                        posts.push(Self::row_to_post(row)?);
                    }
                }
                None => {
                    let mut stmt = conn.prepare(
                        "SELECT id, post_id, author_peer_id, content_type, content_text,
                                visibility, lamport_clock, created_at, updated_at,
                                deleted_at, is_local, relay_status, signature
                         FROM posts
                         WHERE author_peer_id = ? AND deleted_at IS NULL AND lamport_clock > ?
                         ORDER BY lamport_clock ASC
                         LIMIT ?",
                    )?;
                    let mut rows = stmt.query(params![author_peer_id, cursor, limit])?;
                    while let Some(row) = rows.next()? {
                        posts.push(Self::row_to_post(row)?);
                    }
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
                            deleted_at, is_local, relay_status, signature
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
                            deleted_at, is_local, relay_status, signature
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

    /// Update post content while preserving the previously stored post signature.
    pub fn update_post(
        db: &Database,
        post_id: &str,
        content_text: Option<&str>,
        updated_at: i64,
        lamport_clock: i64,
    ) -> SqliteResult<bool> {
        db.with_connection(|conn| {
            let rows = conn.execute(
                "UPDATE posts SET content_text = ?, updated_at = ?, lamport_clock = ?, deleted_at = NULL
                 WHERE post_id = ?",
                params![content_text, updated_at, lamport_clock, post_id],
            )?;
            Ok(rows > 0)
        })
    }

    /// Update post content and replace the materialized post signature.
    pub fn update_post_with_signature(
        db: &Database,
        post_id: &str,
        content_text: Option<&str>,
        updated_at: i64,
        lamport_clock: i64,
        signature: &[u8],
    ) -> SqliteResult<bool> {
        db.with_connection(|conn| {
            let rows = conn.execute(
                "UPDATE posts SET content_text = ?, updated_at = ?, lamport_clock = ?, deleted_at = NULL, signature = ?
                 WHERE post_id = ?",
                params![content_text, updated_at, lamport_clock, signature, post_id],
            )?;
            Ok(rows > 0)
        })
    }

    /// Soft delete a post while preserving the previous lamport/signature state.
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

    /// Soft delete a post and store the tombstone lamport/signature state.
    pub fn delete_post_with_tombstone(
        db: &Database,
        post_id: &str,
        deleted_at: i64,
        lamport_clock: i64,
        signature: &[u8],
    ) -> SqliteResult<bool> {
        db.with_connection(|conn| {
            let rows = conn.execute(
                "UPDATE posts SET deleted_at = ?, updated_at = ?, lamport_clock = ?, signature = ?
                 WHERE post_id = ?",
                params![deleted_at, deleted_at, lamport_clock, signature, post_id],
            )?;
            Ok(rows > 0)
        })
    }

    /// Insert a remote tombstone for a post we have not seen yet.
    pub fn insert_remote_tombstone(
        db: &Database,
        post_id: &str,
        author_peer_id: &str,
        lamport_clock: i64,
        deleted_at: i64,
        signature: &[u8],
    ) -> SqliteResult<i64> {
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO posts (
                    post_id, author_peer_id, content_type, content_text,
                    visibility, lamport_clock, created_at, updated_at,
                    deleted_at, is_local, relay_status, signature
                ) VALUES (?, ?, ?, NULL, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    post_id,
                    author_peer_id,
                    "deleted",
                    PostVisibility::Public.as_str(),
                    lamport_clock,
                    deleted_at,
                    deleted_at,
                    deleted_at,
                    0i32,
                    "relay_acknowledged",
                    signature,
                ],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    /// Get feed posts from multiple authors, sorted by created_at DESC.
    ///
    /// This is more efficient than querying per-author and merging,
    /// and correctly applies the limit across all authors.
    ///
    /// The SQL `IN` clause is built dynamically using
    /// [`build_in_clause_placeholders`](crate::db::sql_utils::build_in_clause_placeholders),
    /// which produces only literal `?` characters.  All actual peer-id values
    /// are bound via rusqlite parameter binding, so no user data is ever
    /// interpolated into the query string.
    pub fn get_feed_posts(
        db: &Database,
        author_peer_ids: &[String],
        limit: i64,
        before_timestamp: Option<i64>,
    ) -> SqliteResult<Vec<Post>> {
        if author_peer_ids.is_empty() {
            return Ok(Vec::new());
        }

        db.with_connection(|conn| {
            let mut posts = Vec::new();

            // SAFETY: `build_in_clause_placeholders` returns only literal "?"
            // characters joined by commas (e.g., "?,?,?").  No user input is
            // interpolated into the SQL structure.  All actual values are bound
            // via parameterized placeholders.
            let placeholders =
                crate::db::sql_utils::build_in_clause_placeholders(author_peer_ids.len());

            if let Some(before) = before_timestamp {
                let sql = format!(
                    "SELECT id, post_id, author_peer_id, content_type, content_text,
                            visibility, lamport_clock, created_at, updated_at,
                            deleted_at, is_local, relay_status, signature
                     FROM posts
                     WHERE author_peer_id IN ({}) AND deleted_at IS NULL AND created_at < ?
                     ORDER BY created_at DESC
                     LIMIT ?",
                    placeholders
                );
                let mut stmt = conn.prepare(&sql)?;

                // Build params: author_peer_ids + before + limit
                let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
                for id in author_peer_ids {
                    param_values.push(Box::new(id.clone()));
                }
                param_values.push(Box::new(before));
                param_values.push(Box::new(limit));

                let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                    param_values.iter().map(|p| p.as_ref()).collect();

                let mut rows = stmt.query(param_refs.as_slice())?;
                while let Some(row) = rows.next()? {
                    posts.push(Self::row_to_post(row)?);
                }
            } else {
                let sql = format!(
                    "SELECT id, post_id, author_peer_id, content_type, content_text,
                            visibility, lamport_clock, created_at, updated_at,
                            deleted_at, is_local, relay_status, signature
                     FROM posts
                     WHERE author_peer_id IN ({}) AND deleted_at IS NULL
                     ORDER BY created_at DESC
                     LIMIT ?",
                    placeholders
                );
                let mut stmt = conn.prepare(&sql)?;

                // Build params: author_peer_ids + limit
                let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
                for id in author_peer_ids {
                    param_values.push(Box::new(id.clone()));
                }
                param_values.push(Box::new(limit));

                let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                    param_values.iter().map(|p| p.as_ref()).collect();

                let mut rows = stmt.query(param_refs.as_slice())?;
                while let Some(row) = rows.next()? {
                    posts.push(Self::row_to_post(row)?);
                }
            }

            Ok(posts)
        })
    }

    /// Count posts by visibility for a given author.
    ///
    /// Returns a [`VisibilityCounts`] with the total, public, and contacts-only
    /// counts computed entirely in SQL -- no rows are transferred to Rust.
    pub fn count_by_visibility(
        db: &Database,
        author_peer_id: &str,
    ) -> SqliteResult<VisibilityCounts> {
        db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT visibility, COUNT(*) as cnt
                 FROM posts
                 WHERE author_peer_id = ? AND deleted_at IS NULL
                 GROUP BY visibility",
            )?;

            let mut public_posts: usize = 0;
            let mut contacts_only_posts: usize = 0;

            let mut rows = stmt.query(params![author_peer_id])?;
            while let Some(row) = rows.next()? {
                let visibility: String = row.get(0)?;
                let count: usize = row.get::<_, i64>(1)? as usize;
                match visibility.as_str() {
                    "public" => public_posts = count,
                    "contacts" => contacts_only_posts = count,
                    _ => {} // ignore unknown visibility values
                }
            }

            let total_posts = public_posts + contacts_only_posts;

            Ok(VisibilityCounts {
                total_posts,
                public_posts,
                contacts_only_posts,
            })
        })
    }

    /// Get posts by author, optionally filtered to a specific visibility.
    ///
    /// When `visibility` is `Some`, only posts matching that visibility are
    /// returned.  When `None`, all non-deleted posts for the author are returned
    /// (same behaviour as [`get_by_author`]).
    pub fn get_by_author_with_visibility(
        db: &Database,
        author_peer_id: &str,
        visibility: Option<PostVisibility>,
        limit: i64,
        before_timestamp: Option<i64>,
    ) -> SqliteResult<Vec<Post>> {
        db.with_connection(|conn| {
            let mut posts = Vec::new();

            match (visibility, before_timestamp) {
                (Some(vis), Some(before)) => {
                    let mut stmt = conn.prepare(
                        "SELECT id, post_id, author_peer_id, content_type, content_text,
                                visibility, lamport_clock, created_at, updated_at,
                                deleted_at, is_local, relay_status, signature
                         FROM posts
                         WHERE author_peer_id = ? AND deleted_at IS NULL
                               AND visibility = ? AND created_at < ?
                         ORDER BY created_at DESC
                         LIMIT ?",
                    )?;
                    let mut rows =
                        stmt.query(params![author_peer_id, vis.as_str(), before, limit])?;
                    while let Some(row) = rows.next()? {
                        posts.push(Self::row_to_post(row)?);
                    }
                }
                (Some(vis), None) => {
                    let mut stmt = conn.prepare(
                        "SELECT id, post_id, author_peer_id, content_type, content_text,
                                visibility, lamport_clock, created_at, updated_at,
                                deleted_at, is_local, relay_status, signature
                         FROM posts
                         WHERE author_peer_id = ? AND deleted_at IS NULL
                               AND visibility = ?
                         ORDER BY created_at DESC
                         LIMIT ?",
                    )?;
                    let mut rows = stmt.query(params![author_peer_id, vis.as_str(), limit])?;
                    while let Some(row) = rows.next()? {
                        posts.push(Self::row_to_post(row)?);
                    }
                }
                (None, Some(before)) => {
                    let mut stmt = conn.prepare(
                        "SELECT id, post_id, author_peer_id, content_type, content_text,
                                visibility, lamport_clock, created_at, updated_at,
                                deleted_at, is_local, relay_status, signature
                         FROM posts
                         WHERE author_peer_id = ? AND deleted_at IS NULL AND created_at < ?
                         ORDER BY created_at DESC
                         LIMIT ?",
                    )?;
                    let mut rows = stmt.query(params![author_peer_id, before, limit])?;
                    while let Some(row) = rows.next()? {
                        posts.push(Self::row_to_post(row)?);
                    }
                }
                (None, None) => {
                    let mut stmt = conn.prepare(
                        "SELECT id, post_id, author_peer_id, content_type, content_text,
                                visibility, lamport_clock, created_at, updated_at,
                                deleted_at, is_local, relay_status, signature
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
            }

            Ok(posts)
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
                "INSERT OR REPLACE INTO post_media (
                    post_id, media_hash, media_type, mime_type,
                    file_name, file_size, width, height,
                    duration_seconds, sort_order, signature
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
                    media.signature,
                ],
            )?;
            Ok(())
        })
    }

    /// Get media for a post
    pub fn get_post_media(db: &Database, post_id: &str) -> SqliteResult<Vec<PostMedia>> {
        db.with_connection(|conn| Self::get_post_media_inner(conn, post_id))
    }

    pub(crate) fn get_post_media_inner(
        conn: &Connection,
        post_id: &str,
    ) -> SqliteResult<Vec<PostMedia>> {
        let mut stmt = conn.prepare(
            "SELECT id, post_id, media_hash, media_type, mime_type,
                        file_name, file_size, width, height,
                        duration_seconds, sort_order, signature
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
                signature: row.get(11)?,
            });
        }

        Ok(media)
    }

    /// Record a post event (for event sourcing)
    pub fn record_post_event(
        db: &Database,
        params: &RecordPostEventParams<'_>,
    ) -> SqliteResult<i64> {
        db.with_connection(|conn| {
            let received_at = chrono::Utc::now().timestamp();
            conn.execute(
                "INSERT OR REPLACE INTO post_events (
                    event_id, event_type, post_id, author_peer_id,
                    lamport_clock, timestamp, payload_cbor, signature, received_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    params.event_id,
                    params.event_type,
                    params.post_id,
                    params.author_peer_id,
                    params.lamport_clock,
                    params.timestamp,
                    params.payload_cbor,
                    params.signature,
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

    /// Fetch attachment hashes for many posts with a bounded number of SQL
    /// statements. Chunking stays below SQLite's parameter limit while
    /// avoiding one query per manifest entry.
    pub fn get_media_hashes_batch(
        db: &Database,
        post_ids: &[String],
    ) -> SqliteResult<std::collections::HashMap<String, Vec<String>>> {
        db.with_connection(|connection| {
            let mut result = std::collections::HashMap::new();
            for chunk in post_ids.chunks(400) {
                if chunk.is_empty() {
                    continue;
                }
                let placeholders = std::iter::repeat_n("?", chunk.len())
                    .collect::<Vec<_>>()
                    .join(",");
                let sql = format!(
                    "SELECT post_id, media_hash FROM post_media
                     WHERE post_id IN ({placeholders}) ORDER BY post_id, sort_order"
                );
                let mut statement = connection.prepare(&sql)?;
                let rows = statement
                    .query_map(rusqlite::params_from_iter(chunk.iter()), |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                    })?;
                for row in rows {
                    let (post_id, hash) = row?;
                    result.entry(post_id).or_insert_with(Vec::new).push(hash);
                }
            }
            Ok(result)
        })
    }

    fn row_to_relay_outbox(row: &rusqlite::Row<'_>) -> SqliteResult<PostRelayOutboxEntry> {
        let state: String = row.get(4)?;
        Ok(PostRelayOutboxEntry {
            event_id: row.get(0)?,
            post_id: row.get(1)?,
            mutation_type: row.get(2)?,
            payload_cbor: row.get(3)?,
            state: PostRelayOutboxState::parse(&state)?,
            attempt_count: row.get(5)?,
            max_attempts: row.get(6)?,
            next_attempt_at: row.get(7)?,
            attempt_deadline_at: row.get(8)?,
            relay_peer_id: row.get(9)?,
            last_error: row.get(10)?,
            created_at: row.get(11)?,
            updated_at: row.get(12)?,
            terminal_at: row.get(13)?,
        })
    }

    pub fn get_relay_outbox(
        db: &Database,
        event_id: &str,
    ) -> SqliteResult<Option<PostRelayOutboxEntry>> {
        db.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT event_id,post_id,mutation_type,payload_cbor,state,attempt_count,
                            max_attempts,next_attempt_at,attempt_deadline_at,relay_peer_id,
                            last_error,created_at,updated_at,terminal_at
                     FROM post_relay_outbox WHERE event_id=?1",
                    [event_id],
                    Self::row_to_relay_outbox,
                )
                .optional()
        })
    }

    pub fn claim_due_relay_outbox(
        db: &Database,
        relay_peer_id: &str,
        now: i64,
        lease_seconds: i64,
        limit: u32,
    ) -> SqliteResult<Vec<PostRelayOutboxEntry>> {
        db.with_connection_mut(|connection| {
            let transaction = connection.transaction()?;
            transaction.execute(
                "UPDATE post_relay_outbox
                 SET state='queued', attempt_deadline_at=NULL, relay_peer_id=NULL,
                     next_attempt_at=?1, updated_at=?1,
                     last_error='Relay attempt interrupted before acknowledgement'
                 WHERE state='in_flight' AND attempt_deadline_at IS NOT NULL
                       AND attempt_deadline_at<=?1",
                [now],
            )?;
            let event_ids = {
                let mut statement = transaction.prepare(
                    "SELECT event_id FROM post_relay_outbox
                     WHERE state='queued' AND next_attempt_at<=?1
                     ORDER BY next_attempt_at,created_at,event_id LIMIT ?2",
                )?;
                let rows =
                    statement.query_map(params![now, limit], |row| row.get::<_, String>(0))?;
                let event_ids = rows.collect::<SqliteResult<Vec<_>>>()?;
                event_ids
            };
            let deadline = now.saturating_add(lease_seconds.max(1));
            for event_id in &event_ids {
                transaction.execute(
                    "UPDATE post_relay_outbox
                     SET state='in_flight',attempt_count=attempt_count+1,
                         attempt_deadline_at=?1,relay_peer_id=?2,updated_at=?3,last_error=NULL
                     WHERE event_id=?4 AND state='queued'",
                    params![deadline, relay_peer_id, now, event_id],
                )?;
            }
            let mut claimed = Vec::with_capacity(event_ids.len());
            for event_id in event_ids {
                claimed.push(transaction.query_row(
                    "SELECT event_id,post_id,mutation_type,payload_cbor,state,attempt_count,
                            max_attempts,next_attempt_at,attempt_deadline_at,relay_peer_id,
                            last_error,created_at,updated_at,terminal_at
                     FROM post_relay_outbox WHERE event_id=?1",
                    [event_id],
                    Self::row_to_relay_outbox,
                )?);
            }
            transaction.commit()?;
            Ok(claimed)
        })
    }

    fn refresh_projection_relay_status(
        transaction: &Transaction<'_>,
        post_id: &str,
    ) -> SqliteResult<()> {
        let latest_state: Option<String> = transaction
            .query_row(
                "SELECT outbox.state
                 FROM post_relay_outbox outbox
                 JOIN post_events event ON event.event_id=outbox.event_id
                 WHERE outbox.post_id=?1
                 ORDER BY event.lamport_clock DESC,event.timestamp DESC,outbox.event_id DESC
                 LIMIT 1",
                [post_id],
                |row| row.get(0),
            )
            .optional()?;
        let relay_status = match latest_state.as_deref() {
            Some("acknowledged") => "relay_acknowledged",
            Some("conflict") => "conflict",
            Some("failed") => "failed",
            _ => "local_pending",
        };
        transaction.execute(
            "UPDATE posts SET relay_status=?1 WHERE post_id=?2",
            params![relay_status, post_id],
        )?;
        Ok(())
    }

    pub fn acknowledge_relay_outbox(
        db: &Database,
        event_id: &str,
        relay_peer_id: &str,
        now: i64,
    ) -> SqliteResult<bool> {
        db.with_connection_mut(|connection| {
            let transaction = connection.transaction()?;
            let post_id: Option<String> = transaction
                .query_row(
                    "SELECT post_id FROM post_relay_outbox WHERE event_id=?1",
                    [event_id],
                    |row| row.get(0),
                )
                .optional()?;
            let Some(post_id) = post_id else {
                return Ok(false);
            };
            let changed = transaction.execute(
                "UPDATE post_relay_outbox
                 SET state='acknowledged',relay_peer_id=?1,attempt_deadline_at=NULL,
                     updated_at=?2,terminal_at=?2,last_error=NULL
                 WHERE event_id=?3 AND state IN ('queued','in_flight')",
                params![relay_peer_id, now, event_id],
            )? > 0;
            Self::refresh_projection_relay_status(&transaction, &post_id)?;
            transaction.commit()?;
            Ok(changed)
        })
    }

    pub fn fail_relay_outbox_attempt(
        db: &Database,
        event_id: &str,
        error: &str,
        conflict: bool,
        now: i64,
    ) -> SqliteResult<bool> {
        db.with_connection_mut(|connection| {
            let transaction = connection.transaction()?;
            let entry: Option<(String, i64, i64)> = transaction
                .query_row(
                    "SELECT post_id,attempt_count,max_attempts FROM post_relay_outbox
                     WHERE event_id=?1 AND state='in_flight'",
                    [event_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()?;
            let Some((post_id, attempt_count, max_attempts)) = entry else {
                return Ok(false);
            };
            if conflict {
                transaction.execute(
                    "UPDATE post_relay_outbox SET state='conflict',attempt_deadline_at=NULL,
                         last_error=?1,updated_at=?2,terminal_at=?2 WHERE event_id=?3",
                    params![error, now, event_id],
                )?;
            } else if attempt_count >= max_attempts {
                transaction.execute(
                    "UPDATE post_relay_outbox SET state='failed',attempt_deadline_at=NULL,
                         last_error=?1,updated_at=?2,terminal_at=?2 WHERE event_id=?3",
                    params![error, now, event_id],
                )?;
            } else {
                let exponent = u32::try_from(attempt_count.saturating_sub(1).min(8)).unwrap_or(8);
                let backoff = 1i64.checked_shl(exponent).unwrap_or(256).min(300);
                transaction.execute(
                    "UPDATE post_relay_outbox SET state='queued',attempt_deadline_at=NULL,
                         relay_peer_id=NULL,last_error=?1,updated_at=?2,next_attempt_at=?3
                     WHERE event_id=?4",
                    params![error, now, now.saturating_add(backoff), event_id],
                )?;
            }
            Self::refresh_projection_relay_status(&transaction, &post_id)?;
            transaction.commit()?;
            Ok(true)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_db() -> Database {
        Database::in_memory().unwrap()
    }

    fn insert_test_post(
        db: &Database,
        post_id: &str,
        author_peer_id: &str,
        visibility: PostVisibility,
        lamport_clock: i64,
        created_at: i64,
    ) {
        PostsRepository::insert_post(
            db,
            &PostData {
                post_id: post_id.to_string(),
                author_peer_id: author_peer_id.to_string(),
                content_type: "text".to_string(),
                content_text: Some(post_id.to_string()),
                visibility,
                lamport_clock,
                created_at,
                signature: vec![1, 2, 3, 4],
            },
        )
        .unwrap();
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
            signature: vec![9, 9, 9],
        };

        PostsRepository::add_media(&db, &media).unwrap();

        let stored_media = PostsRepository::get_post_media(&db, "post-media").unwrap();
        assert_eq!(stored_media.len(), 1);
        assert_eq!(stored_media[0].media_hash, "abc123");
        assert_eq!(stored_media[0].width, Some(800));

        let hashes = PostsRepository::get_media_hashes(&db, "post-media").unwrap();
        assert_eq!(hashes, vec!["abc123"]);
    }

    #[test]
    fn test_visibility_counts_and_preview_filter_ignore_deleted_posts() {
        let db = create_test_db();
        insert_test_post(&db, "public-1", "peer-a", PostVisibility::Public, 1, 1000);
        insert_test_post(
            &db,
            "contacts-1",
            "peer-a",
            PostVisibility::Contacts,
            2,
            2000,
        );
        insert_test_post(
            &db,
            "public-deleted",
            "peer-a",
            PostVisibility::Public,
            3,
            3000,
        );
        PostsRepository::delete_post(&db, "public-deleted", 4000).unwrap();

        let counts = PostsRepository::count_by_visibility(&db, "peer-a").unwrap();
        assert_eq!(counts.total_posts, 2);
        assert_eq!(counts.public_posts, 1);
        assert_eq!(counts.contacts_only_posts, 1);

        let public_posts = PostsRepository::get_by_author_with_visibility(
            &db,
            "peer-a",
            Some(PostVisibility::Public),
            10,
            None,
        )
        .unwrap();
        assert_eq!(public_posts.len(), 1);
        assert_eq!(public_posts[0].post_id, "public-1");
    }

    #[test]
    fn test_after_cursor_visibility_filter_returns_public_posts_past_contacts_only() {
        let db = create_test_db();
        insert_test_post(
            &db,
            "contacts-older",
            "peer-a",
            PostVisibility::Contacts,
            1,
            1000,
        );
        insert_test_post(&db, "public-mid", "peer-a", PostVisibility::Public, 2, 2000);
        insert_test_post(
            &db,
            "contacts-newer",
            "peer-a",
            PostVisibility::Contacts,
            3,
            3000,
        );
        insert_test_post(
            &db,
            "public-newest",
            "peer-a",
            PostVisibility::Public,
            4,
            4000,
        );

        let public_posts = PostsRepository::get_by_author_after_cursor_with_visibility(
            &db,
            "peer-a",
            0,
            Some(PostVisibility::Public),
            10,
        )
        .unwrap();
        let ids: Vec<_> = public_posts.into_iter().map(|post| post.post_id).collect();
        assert_eq!(ids, vec!["public-mid", "public-newest"]);
    }
}
