//! Board repository for storing and retrieving community board data

use crate::db::Database;
use rusqlite::{params, Result as SqliteResult};

/// A cached relay community
#[derive(Debug, Clone)]
pub struct RelayCommunity {
    pub relay_peer_id: String,
    pub relay_address: String,
    pub community_name: Option<String>,
    pub joined_at: i64,
    pub last_sync_at: Option<i64>,
}

/// A cached board
#[derive(Debug, Clone)]
pub struct Board {
    pub board_id: String,
    pub relay_peer_id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_default: bool,
    pub cached_at: i64,
}

/// A cached board post
#[derive(Debug, Clone)]
pub struct BoardPost {
    pub post_id: String,
    pub board_id: String,
    pub relay_peer_id: String,
    pub author_peer_id: String,
    pub author_display_name: Option<String>,
    pub content_type: String,
    pub content_text: Option<String>,
    pub lamport_clock: i64,
    pub created_at: i64,
    pub deleted_at: Option<i64>,
    pub signature: Vec<u8>,
    pub cached_at: i64,
}

/// Repository for board operations
pub struct BoardsRepository;

impl BoardsRepository {
    /// Insert or update a relay community
    pub fn upsert_relay_community(
        db: &Database,
        relay_peer_id: &str,
        relay_address: &str,
        community_name: Option<&str>,
        joined_at: i64,
    ) -> SqliteResult<()> {
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO relay_communities (relay_peer_id, relay_address, community_name, joined_at)
                 VALUES (?, ?, ?, ?)
                 ON CONFLICT(relay_peer_id) DO UPDATE SET
                     relay_address = excluded.relay_address,
                     community_name = COALESCE(excluded.community_name, relay_communities.community_name)",
                params![relay_peer_id, relay_address, community_name, joined_at],
            )?;
            Ok(())
        })
    }

    /// Get all relay communities
    pub fn get_relay_communities(db: &Database) -> SqliteResult<Vec<RelayCommunity>> {
        db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT relay_peer_id, relay_address, community_name, joined_at, last_sync_at
                 FROM relay_communities ORDER BY joined_at DESC",
            )?;
            let mut communities = Vec::new();
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next()? {
                communities.push(RelayCommunity {
                    relay_peer_id: row.get(0)?,
                    relay_address: row.get(1)?,
                    community_name: row.get(2)?,
                    joined_at: row.get(3)?,
                    last_sync_at: row.get(4)?,
                });
            }
            Ok(communities)
        })
    }

    /// Remove a relay community (cascade deletes boards and posts)
    pub fn delete_relay_community(db: &Database, relay_peer_id: &str) -> SqliteResult<bool> {
        db.with_connection(|conn| {
            let rows = conn.execute(
                "DELETE FROM relay_communities WHERE relay_peer_id = ?",
                [relay_peer_id],
            )?;
            Ok(rows > 0)
        })
    }

    /// Insert or update a board
    pub fn upsert_board(
        db: &Database,
        board_id: &str,
        relay_peer_id: &str,
        name: &str,
        description: Option<&str>,
        is_default: bool,
    ) -> SqliteResult<()> {
        let now = chrono::Utc::now().timestamp();
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO boards (board_id, relay_peer_id, name, description, is_default, cached_at)
                 VALUES (?, ?, ?, ?, ?, ?)
                 ON CONFLICT(board_id, relay_peer_id) DO UPDATE SET
                     name = excluded.name,
                     description = excluded.description,
                     is_default = excluded.is_default,
                     cached_at = excluded.cached_at",
                params![board_id, relay_peer_id, name, description, is_default as i32, now],
            )?;
            Ok(())
        })
    }

    /// Get boards for a relay
    pub fn get_boards_for_relay(db: &Database, relay_peer_id: &str) -> SqliteResult<Vec<Board>> {
        db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT board_id, relay_peer_id, name, description, is_default, cached_at
                 FROM boards WHERE relay_peer_id = ?
                 ORDER BY is_default DESC, name ASC",
            )?;
            let mut boards = Vec::new();
            let mut rows = stmt.query([relay_peer_id])?;
            while let Some(row) = rows.next()? {
                boards.push(Board {
                    board_id: row.get(0)?,
                    relay_peer_id: row.get(1)?,
                    name: row.get(2)?,
                    description: row.get(3)?,
                    is_default: row.get::<_, i32>(4)? != 0,
                    cached_at: row.get(5)?,
                });
            }
            Ok(boards)
        })
    }

    /// Insert or update a board post
    #[allow(clippy::too_many_arguments)]
    pub fn upsert_board_post(
        db: &Database,
        post_id: &str,
        board_id: &str,
        relay_peer_id: &str,
        author_peer_id: &str,
        author_display_name: Option<&str>,
        content_type: &str,
        content_text: Option<&str>,
        lamport_clock: i64,
        created_at: i64,
        deleted_at: Option<i64>,
        signature: &[u8],
    ) -> SqliteResult<()> {
        let now = chrono::Utc::now().timestamp();
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO board_posts (post_id, board_id, relay_peer_id, author_peer_id,
                    author_display_name, content_type, content_text, lamport_clock,
                    created_at, deleted_at, signature, cached_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(post_id, relay_peer_id) DO UPDATE SET
                     deleted_at = excluded.deleted_at,
                     cached_at = excluded.cached_at",
                params![
                    post_id,
                    board_id,
                    relay_peer_id,
                    author_peer_id,
                    author_display_name,
                    content_type,
                    content_text,
                    lamport_clock,
                    created_at,
                    deleted_at,
                    signature,
                    now
                ],
            )?;
            Ok(())
        })
    }

    /// Get posts for a board (paginated)
    pub fn get_board_posts(
        db: &Database,
        board_id: &str,
        relay_peer_id: &str,
        limit: i64,
        before_timestamp: Option<i64>,
    ) -> SqliteResult<Vec<BoardPost>> {
        db.with_connection(|conn| {
            let mut posts = Vec::new();
            if let Some(before) = before_timestamp {
                let mut stmt = conn.prepare(
                    "SELECT post_id, board_id, relay_peer_id, author_peer_id,
                            author_display_name, content_type, content_text, lamport_clock,
                            created_at, deleted_at, signature, cached_at
                     FROM board_posts
                     WHERE board_id = ? AND relay_peer_id = ? AND created_at < ? AND deleted_at IS NULL
                     ORDER BY created_at DESC LIMIT ?",
                )?;
                let mut rows = stmt.query(params![board_id, relay_peer_id, before, limit])?;
                while let Some(row) = rows.next()? {
                    posts.push(Self::row_to_board_post(row)?);
                }
            } else {
                let mut stmt = conn.prepare(
                    "SELECT post_id, board_id, relay_peer_id, author_peer_id,
                            author_display_name, content_type, content_text, lamport_clock,
                            created_at, deleted_at, signature, cached_at
                     FROM board_posts
                     WHERE board_id = ? AND relay_peer_id = ? AND deleted_at IS NULL
                     ORDER BY created_at DESC LIMIT ?",
                )?;
                let mut rows = stmt.query(params![board_id, relay_peer_id, limit])?;
                while let Some(row) = rows.next()? {
                    posts.push(Self::row_to_board_post(row)?);
                }
            }
            Ok(posts)
        })
    }

    fn row_to_board_post(row: &rusqlite::Row) -> SqliteResult<BoardPost> {
        Ok(BoardPost {
            post_id: row.get(0)?,
            board_id: row.get(1)?,
            relay_peer_id: row.get(2)?,
            author_peer_id: row.get(3)?,
            author_display_name: row.get(4)?,
            content_type: row.get(5)?,
            content_text: row.get(6)?,
            lamport_clock: row.get(7)?,
            created_at: row.get(8)?,
            deleted_at: row.get(9)?,
            signature: row.get(10)?,
            cached_at: row.get(11)?,
        })
    }

    /// Get sync cursor for a board
    pub fn get_board_sync_cursor(
        db: &Database,
        relay_peer_id: &str,
        board_id: &str,
    ) -> SqliteResult<Option<i64>> {
        db.with_connection(|conn| {
            conn.query_row(
                "SELECT last_post_timestamp FROM board_sync_cursors
                 WHERE relay_peer_id = ? AND board_id = ?",
                params![relay_peer_id, board_id],
                |row| row.get(0),
            )
            .or(Ok(None))
        })
    }

    /// Update sync cursor for a board
    pub fn update_board_sync_cursor(
        db: &Database,
        relay_peer_id: &str,
        board_id: &str,
        last_post_timestamp: i64,
    ) -> SqliteResult<()> {
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO board_sync_cursors (relay_peer_id, board_id, last_post_timestamp)
                 VALUES (?, ?, ?)
                 ON CONFLICT(relay_peer_id, board_id) DO UPDATE SET
                     last_post_timestamp = MAX(board_sync_cursors.last_post_timestamp, excluded.last_post_timestamp)",
                params![relay_peer_id, board_id, last_post_timestamp],
            )?;
            Ok(())
        })
    }

    /// Update last_sync_at for a community
    pub fn update_community_sync_time(db: &Database, relay_peer_id: &str) -> SqliteResult<()> {
        let now = chrono::Utc::now().timestamp();
        db.with_connection(|conn| {
            conn.execute(
                "UPDATE relay_communities SET last_sync_at = ? WHERE relay_peer_id = ?",
                params![now, relay_peer_id],
            )?;
            Ok(())
        })
    }

    /// Delete a board post locally
    pub fn delete_board_post(
        db: &Database,
        post_id: &str,
        relay_peer_id: &str,
    ) -> SqliteResult<bool> {
        let now = chrono::Utc::now().timestamp();
        db.with_connection(|conn| {
            let rows = conn.execute(
                "UPDATE board_posts SET deleted_at = ? WHERE post_id = ? AND relay_peer_id = ? AND deleted_at IS NULL",
                params![now, post_id, relay_peer_id],
            )?;
            Ok(rows > 0)
        })
    }
}
