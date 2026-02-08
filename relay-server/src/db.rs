//! Relay server SQLite database for community board data

use rusqlite::{params, Connection, OptionalExtension, Result as SqliteResult};
use std::sync::{Arc, Mutex};
use tracing::info;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS boards (
    board_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    created_by_peer_id TEXT,
    created_at INTEGER NOT NULL,
    is_default INTEGER DEFAULT 0
);

CREATE TABLE IF NOT EXISTS board_posts (
    post_id TEXT PRIMARY KEY,
    board_id TEXT NOT NULL,
    author_peer_id TEXT NOT NULL,
    content_type TEXT NOT NULL DEFAULT 'text',
    content_text TEXT,
    lamport_clock INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    deleted_at INTEGER,
    signature BLOB NOT NULL,
    FOREIGN KEY (board_id) REFERENCES boards(board_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_board_posts_board_time
    ON board_posts(board_id, created_at DESC);

CREATE TABLE IF NOT EXISTS known_peers (
    peer_id TEXT PRIMARY KEY,
    public_key BLOB NOT NULL,
    display_name TEXT NOT NULL,
    first_seen_at INTEGER NOT NULL,
    last_seen_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS banned_peers (
    peer_id TEXT PRIMARY KEY,
    reason TEXT,
    banned_at INTEGER NOT NULL,
    banned_by TEXT
);

CREATE TABLE IF NOT EXISTS author_lamport_clocks (
    author_peer_id TEXT PRIMARY KEY,
    last_seen_clock INTEGER NOT NULL DEFAULT 0,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (author_peer_id) REFERENCES known_peers(peer_id) ON DELETE CASCADE
);
"#;

/// Relay server database
#[derive(Clone)]
pub struct RelayDatabase {
    conn: Arc<Mutex<Connection>>,
}

impl RelayDatabase {
    /// Open or create the database at the given path
    pub fn open(path: &str) -> SqliteResult<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        conn.execute_batch(SCHEMA)?;

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        // Create default "General" board if none exist
        db.ensure_default_board()?;

        info!("Relay database initialized at {}", path);
        Ok(db)
    }

    fn ensure_default_board(&self) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM boards WHERE is_default = 1",
            [],
            |row| row.get(0),
        )?;

        if count == 0 {
            let now = chrono::Utc::now().timestamp();
            conn.execute(
                "INSERT INTO boards (board_id, name, description, created_at, is_default)
                 VALUES (?, ?, ?, ?, 1)",
                params![
                    uuid::Uuid::new_v4().to_string(),
                    "General",
                    "General discussion",
                    now,
                ],
            )?;
            info!("Created default 'General' board");
        }
        Ok(())
    }

    // ========== Board Operations ==========

    pub fn list_boards(&self) -> SqliteResult<Vec<BoardRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT board_id, name, description, is_default FROM boards ORDER BY is_default DESC, name ASC",
        )?;
        let mut boards = Vec::new();
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            boards.push(BoardRow {
                board_id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                is_default: row.get::<_, i32>(3)? != 0,
            });
        }
        Ok(boards)
    }

    // ========== Post Operations ==========

    pub fn insert_post(
        &self,
        post_id: &str,
        board_id: &str,
        author_peer_id: &str,
        content_type: &str,
        content_text: Option<&str>,
        lamport_clock: u64,
        created_at: i64,
        signature: &[u8],
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO board_posts (post_id, board_id, author_peer_id, content_type, content_text, lamport_clock, created_at, signature)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![post_id, board_id, author_peer_id, content_type, content_text, lamport_clock as i64, created_at, signature],
        )?;
        Ok(())
    }

    pub fn get_board_posts(
        &self,
        board_id: &str,
        after_timestamp: Option<i64>,
        limit: u32,
    ) -> SqliteResult<Vec<PostRow>> {
        let conn = self.conn.lock().unwrap();
        let mut posts = Vec::new();

        if let Some(after) = after_timestamp {
            let mut stmt = conn.prepare(
                "SELECT bp.post_id, bp.board_id, bp.author_peer_id, bp.content_type, bp.content_text,
                        bp.lamport_clock, bp.created_at, bp.deleted_at, bp.signature,
                        kp.display_name
                 FROM board_posts bp
                 LEFT JOIN known_peers kp ON bp.author_peer_id = kp.peer_id
                 WHERE bp.board_id = ? AND bp.created_at > ?
                 ORDER BY bp.created_at DESC
                 LIMIT ?",
            )?;
            let mut rows = stmt.query(params![board_id, after, limit])?;
            while let Some(row) = rows.next()? {
                posts.push(Self::row_to_post(row)?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT bp.post_id, bp.board_id, bp.author_peer_id, bp.content_type, bp.content_text,
                        bp.lamport_clock, bp.created_at, bp.deleted_at, bp.signature,
                        kp.display_name
                 FROM board_posts bp
                 LEFT JOIN known_peers kp ON bp.author_peer_id = kp.peer_id
                 WHERE bp.board_id = ?
                 ORDER BY bp.created_at DESC
                 LIMIT ?",
            )?;
            let mut rows = stmt.query(params![board_id, limit])?;
            while let Some(row) = rows.next()? {
                posts.push(Self::row_to_post(row)?);
            }
        }
        Ok(posts)
    }

    fn row_to_post(row: &rusqlite::Row) -> SqliteResult<PostRow> {
        Ok(PostRow {
            post_id: row.get(0)?,
            board_id: row.get(1)?,
            author_peer_id: row.get(2)?,
            content_type: row.get(3)?,
            content_text: row.get(4)?,
            lamport_clock: row.get::<_, i64>(5)? as u64,
            created_at: row.get(6)?,
            deleted_at: row.get(7)?,
            signature: row.get(8)?,
            author_display_name: row.get(9)?,
        })
    }

    pub fn delete_post(&self, post_id: &str, author_peer_id: &str) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().timestamp();
        let rows = conn.execute(
            "UPDATE board_posts SET deleted_at = ? WHERE post_id = ? AND author_peer_id = ? AND deleted_at IS NULL",
            params![now, post_id, author_peer_id],
        )?;
        Ok(rows > 0)
    }

    // ========== Peer Operations ==========

    pub fn register_peer(
        &self,
        peer_id: &str,
        public_key: &[u8],
        display_name: &str,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "INSERT INTO known_peers (peer_id, public_key, display_name, first_seen_at, last_seen_at)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(peer_id) DO UPDATE SET
                 display_name = excluded.display_name,
                 last_seen_at = excluded.last_seen_at",
            params![peer_id, public_key, display_name, now, now],
        )?;
        Ok(())
    }

    /// Retrieve the stored public key for a registered peer
    pub fn get_peer_public_key(&self, peer_id: &str) -> SqliteResult<Option<Vec<u8>>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT public_key FROM known_peers WHERE peer_id = ?",
        )?;
        let mut rows = stmt.query([peer_id])?;
        match rows.next()? {
            Some(row) => Ok(Some(row.get(0)?)),
            None => Ok(None),
        }
    }

    pub fn is_peer_known(&self, peer_id: &str) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM known_peers WHERE peer_id = ?",
            [peer_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn is_peer_banned(&self, peer_id: &str) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM banned_peers WHERE peer_id = ?",
            [peer_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Get the highest lamport clock value ever seen for a given author peer.
    ///
    /// This reads from the dedicated `author_lamport_clocks` table, which is
    /// monotonically updated and never decreases -- even when posts are deleted.
    /// Returns 0 if no clock entry exists for this author yet.
    pub fn get_last_lamport_clock(&self, author_peer_id: &str) -> SqliteResult<u64> {
        let conn = self.conn.lock().unwrap();
        let last_clock: Option<i64> = conn
            .query_row(
                "SELECT last_seen_clock FROM author_lamport_clocks WHERE author_peer_id = ?",
                [author_peer_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(last_clock.unwrap_or(0) as u64)
    }

    /// Record a new lamport clock value for an author.
    ///
    /// The caller must ensure `new_clock` is strictly greater than the
    /// previously stored value. This method performs an upsert so that
    /// the first post from a new author creates the tracking row.
    pub fn update_lamport_clock(
        &self,
        author_peer_id: &str,
        new_clock: u64,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "INSERT INTO author_lamport_clocks (author_peer_id, last_seen_clock, updated_at)
             VALUES (?, ?, ?)
             ON CONFLICT(author_peer_id) DO UPDATE SET
                 last_seen_clock = excluded.last_seen_clock,
                 updated_at = excluded.updated_at",
            params![author_peer_id, new_clock as i64, now],
        )?;
        Ok(())
    }

    pub fn board_exists(&self, board_id: &str) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM boards WHERE board_id = ?",
            [board_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }
}

/// A board row from the database
#[derive(Debug, Clone)]
pub struct BoardRow {
    pub board_id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_default: bool,
}

/// A post row from the database
#[derive(Debug, Clone)]
pub struct PostRow {
    pub post_id: String,
    pub board_id: String,
    pub author_peer_id: String,
    pub content_type: String,
    pub content_text: Option<String>,
    pub lamport_clock: u64,
    pub created_at: i64,
    pub deleted_at: Option<i64>,
    pub signature: Vec<u8>,
    pub author_display_name: Option<String>,
}
