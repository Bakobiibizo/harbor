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

CREATE TABLE IF NOT EXISTS relay_signing_keys (
    key_id TEXT PRIMARY KEY, public_key BLOB NOT NULL, not_before INTEGER NOT NULL,
    not_after INTEGER, retired_at INTEGER
);
CREATE TABLE IF NOT EXISTS relay_name_claims (
    local_name TEXT NOT NULL, relay TEXT NOT NULL, peer_id TEXT NOT NULL,
    sequence INTEGER NOT NULL CHECK(sequence > 0), claim_cbor BLOB NOT NULL,
    not_before INTEGER NOT NULL, not_after INTEGER NOT NULL,
    relay_key_id TEXT NOT NULL, status TEXT NOT NULL CHECK(status IN ('active','retired')),
    created_at INTEGER NOT NULL, retired_at INTEGER,
    PRIMARY KEY(relay, local_name, sequence)
);
CREATE UNIQUE INDEX IF NOT EXISTS relay_name_claims_one_active
 ON relay_name_claims(relay,local_name) WHERE status='active';
CREATE TABLE IF NOT EXISTS relay_name_nonces (
    peer_id TEXT NOT NULL, nonce BLOB NOT NULL, used_at INTEGER NOT NULL,
    PRIMARY KEY(peer_id,nonce)
);

CREATE TABLE IF NOT EXISTS author_lamport_clocks (
    author_peer_id TEXT PRIMARY KEY,
    last_seen_clock INTEGER NOT NULL DEFAULT 0,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (author_peer_id) REFERENCES known_peers(peer_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS wall_posts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    post_id TEXT UNIQUE NOT NULL,
    author_peer_id TEXT NOT NULL,
    content_type TEXT NOT NULL,
    content_text TEXT,
    visibility TEXT NOT NULL DEFAULT 'contacts',
    lamport_clock INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    signature BLOB NOT NULL,
    stored_at INTEGER NOT NULL,
    deleted_at INTEGER
);

CREATE INDEX IF NOT EXISTS idx_wall_posts_author
    ON wall_posts(author_peer_id, lamport_clock DESC);

CREATE TABLE IF NOT EXISTS wall_post_media (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    post_id TEXT NOT NULL,
    media_hash TEXT NOT NULL,
    media_type TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    file_name TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    width INTEGER,
    height INTEGER,
    duration_seconds INTEGER,
    sort_order INTEGER DEFAULT 0,
    signature BLOB NOT NULL DEFAULT X'',
    FOREIGN KEY (post_id) REFERENCES wall_posts(post_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_wall_post_media_post
    ON wall_post_media(post_id);

CREATE TABLE IF NOT EXISTS wall_read_grants (
    grant_id TEXT PRIMARY KEY,
    issuer_peer_id TEXT NOT NULL,
    subject_peer_id TEXT NOT NULL,
    capability TEXT NOT NULL,
    scope_json TEXT,
    lamport_clock INTEGER NOT NULL,
    issued_at INTEGER NOT NULL,
    expires_at INTEGER,
    signature BLOB NOT NULL,
    revoked_at INTEGER
);

CREATE INDEX IF NOT EXISTS idx_wall_read_grants_lookup
    ON wall_read_grants(issuer_peer_id, subject_peer_id, capability, revoked_at);

CREATE TABLE IF NOT EXISTS wall_social_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT NOT NULL UNIQUE,
    event_type TEXT NOT NULL,
    post_id TEXT NOT NULL,
    actor_peer_id TEXT NOT NULL,
    author_name TEXT,
    comment_id TEXT,
    content TEXT,
    reaction_type TEXT,
    timestamp INTEGER NOT NULL,
    payload_cbor BLOB NOT NULL,
    signature BLOB NOT NULL,
    stored_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_wall_social_events_post
    ON wall_social_events(post_id, timestamp);
"#;

/// Relay server database
#[derive(Clone)]
pub struct RelayDatabase {
    conn: Arc<Mutex<Connection>>,
}

#[derive(Debug, Clone)]
pub struct WallSocialEventRow {
    pub event_id: String,
    pub event_type: String,
    pub post_id: String,
    pub actor_peer_id: String,
    pub author_name: Option<String>,
    pub comment_id: Option<String>,
    pub content: Option<String>,
    pub reaction_type: Option<String>,
    pub timestamp: i64,
    pub payload_cbor: Vec<u8>,
    pub signature: Vec<u8>,
}

impl RelayDatabase {
    pub fn with_connection<T>(&self, operation: impl FnOnce(&mut Connection) -> T) -> T {
        let mut connection = self.conn.lock().expect("relay database lock poisoned");
        operation(&mut connection)
    }
    /// Open or create the database at the given path
    pub fn open(path: &str) -> SqliteResult<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        conn.execute_batch(SCHEMA)?;

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        db.ensure_wall_post_tombstone_columns()?;
        db.ensure_wall_media_columns()?;

        // Create default "General" board if none exist
        db.ensure_default_board()?;

        info!("Relay database initialized at {}", path);
        Ok(db)
    }

    fn ensure_wall_post_tombstone_columns(&self) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let has_deleted_at: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('wall_posts') WHERE name = 'deleted_at'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|count| count > 0)
            .unwrap_or(false);
        if !has_deleted_at {
            conn.execute("ALTER TABLE wall_posts ADD COLUMN deleted_at INTEGER", [])?;
        }
        Ok(())
    }

    fn ensure_wall_media_columns(&self) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let has_duration: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('wall_post_media') WHERE name = 'duration_seconds'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|count| count > 0)
            .unwrap_or(false);
        if !has_duration {
            conn.execute(
                "ALTER TABLE wall_post_media ADD COLUMN duration_seconds INTEGER",
                [],
            )?;
        }

        let has_signature: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('wall_post_media') WHERE name = 'signature'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|count| count > 0)
            .unwrap_or(false);
        if !has_signature {
            conn.execute(
                "ALTER TABLE wall_post_media ADD COLUMN signature BLOB NOT NULL DEFAULT X''",
                [],
            )?;
        }

        conn.execute(
            "DELETE FROM wall_post_media WHERE id NOT IN (SELECT MIN(id) FROM wall_post_media GROUP BY post_id, media_hash)",
            [],
        )?;
        conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_wall_post_media_post_hash ON wall_post_media(post_id, media_hash)",
            [],
        )?;
        Ok(())
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

    /// Insert a post without lamport clock validation.
    ///
    /// Prefer `insert_post_with_clock_validation` for the normal submit-post
    /// path, which atomically validates and advances the clock. This bare
    /// insert is retained for administrative or testing scenarios.
    #[allow(dead_code)]
    #[allow(clippy::too_many_arguments)] // Database insert mirrors the board_posts schema columns.
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
        let mut stmt = conn.prepare("SELECT public_key FROM known_peers WHERE peer_id = ?")?;
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

    #[allow(clippy::too_many_arguments)]
    pub fn upsert_wall_read_grant(
        &self,
        grant_id: &str,
        issuer_peer_id: &str,
        subject_peer_id: &str,
        capability: &str,
        scope_json: Option<&str>,
        lamport_clock: u64,
        issued_at: i64,
        expires_at: Option<i64>,
        signature: &[u8],
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO wall_read_grants
                (grant_id, issuer_peer_id, subject_peer_id, capability, scope_json,
                 lamport_clock, issued_at, expires_at, signature, revoked_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)
             ON CONFLICT(grant_id) DO UPDATE SET
                 issuer_peer_id = excluded.issuer_peer_id,
                 subject_peer_id = excluded.subject_peer_id,
                 capability = excluded.capability,
                 scope_json = excluded.scope_json,
                 lamport_clock = excluded.lamport_clock,
                 issued_at = excluded.issued_at,
                 expires_at = excluded.expires_at,
                 signature = excluded.signature,
                 revoked_at = NULL
             WHERE excluded.lamport_clock > wall_read_grants.lamport_clock",
            params![
                grant_id,
                issuer_peer_id,
                subject_peer_id,
                capability,
                scope_json,
                lamport_clock as i64,
                issued_at,
                expires_at,
                signature
            ],
        )?;
        Ok(())
    }

    pub fn revoke_wall_read_grant(
        &self,
        grant_id: &str,
        issuer_peer_id: &str,
        lamport_clock: u64,
        revoked_at: i64,
    ) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "UPDATE wall_read_grants SET revoked_at = ?, lamport_clock = ?
             WHERE grant_id = ? AND issuer_peer_id = ? AND lamport_clock < ?
               AND (revoked_at IS NULL OR revoked_at < ?)",
            params![
                revoked_at,
                lamport_clock as i64,
                grant_id,
                issuer_peer_id,
                lamport_clock as i64,
                revoked_at
            ],
        )?;
        Ok(rows > 0)
    }

    pub fn has_active_wall_read_grant(
        &self,
        issuer_peer_id: &str,
        subject_peer_id: &str,
        now: i64,
    ) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM wall_read_grants
             WHERE issuer_peer_id = ?
               AND subject_peer_id = ?
               AND capability IN ('wall_read', 'wall:read')
               AND revoked_at IS NULL
               AND (expires_at IS NULL OR expires_at > ?)",
            params![issuer_peer_id, subject_peer_id, now],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Get the highest lamport clock value ever seen for a given author peer.
    ///
    /// This reads from the dedicated `author_lamport_clocks` table, which is
    /// monotonically updated and never decreases -- even when posts are deleted.
    /// Returns 0 if no clock entry exists for this author yet.
    ///
    /// Note: the normal submit-post path uses `insert_post_with_clock_validation`
    /// which reads the clock inside its own transaction. This standalone reader
    /// is retained for diagnostics and testing.
    #[allow(dead_code)]
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
    ///
    /// Note: the normal submit-post path uses `insert_post_with_clock_validation`
    /// which writes the clock inside its own transaction. This standalone writer
    /// is retained for administrative use and testing.
    #[allow(dead_code)]
    pub fn update_lamport_clock(&self, author_peer_id: &str, new_clock: u64) -> SqliteResult<()> {
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

    /// Atomically validate, insert a post, and advance the author's lamport clock.
    ///
    /// This method performs the following steps inside a single database
    /// transaction (and a single Mutex acquisition), eliminating TOCTOU
    /// races that could occur if the caller performed these steps separately:
    ///
    /// 1. Read the author's last seen lamport clock.
    /// 2. Reject the post if `lamport_clock <= last_seen_clock`.
    /// 3. Insert the post row.
    /// 4. Upsert the new high-water mark for the author's lamport clock.
    ///
    /// Returns `Ok(())` on success, or an error string on validation failure
    /// / database error.
    #[allow(clippy::too_many_arguments)] // Transaction arguments match the signed board post fields.
    pub fn insert_post_with_clock_validation(
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
        let conn = self.conn.lock().unwrap();

        conn.execute_batch("BEGIN IMMEDIATE")
            .map_err(|e| format!("Failed to begin transaction: {}", e))?;

        // Step 1: Read the current high-water mark for this author.
        let last_seen_clock: u64 = conn
            .query_row(
                "SELECT last_seen_clock FROM author_lamport_clocks WHERE author_peer_id = ?",
                [author_peer_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|e| {
                let _ = conn.execute_batch("ROLLBACK");
                format!("Failed to query lamport clock: {}", e)
            })?
            .unwrap_or(0) as u64;

        // Step 2: Validate strictly increasing clock.
        if lamport_clock <= last_seen_clock {
            let _ = conn.execute_batch("ROLLBACK");
            return Err(format!(
                "Stale lamport clock: received {} but last seen was {}. Clock must be strictly increasing.",
                lamport_clock, last_seen_clock
            ));
        }

        // Step 3: Insert the post.
        conn.execute(
            "INSERT INTO board_posts (post_id, board_id, author_peer_id, content_type, content_text, lamport_clock, created_at, signature)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![post_id, board_id, author_peer_id, content_type, content_text, lamport_clock as i64, created_at, signature],
        )
        .map_err(|e| {
            let _ = conn.execute_batch("ROLLBACK");
            format!("Failed to insert post: {}", e)
        })?;

        // Step 4: Upsert the new lamport clock high-water mark.
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "INSERT INTO author_lamport_clocks (author_peer_id, last_seen_clock, updated_at)
             VALUES (?, ?, ?)
             ON CONFLICT(author_peer_id) DO UPDATE SET
                 last_seen_clock = excluded.last_seen_clock,
                 updated_at = excluded.updated_at",
            params![author_peer_id, lamport_clock as i64, now],
        )
        .map_err(|e| {
            let _ = conn.execute_batch("ROLLBACK");
            format!("Failed to update lamport clock: {}", e)
        })?;

        conn.execute_batch("COMMIT")
            .map_err(|e| format!("Failed to commit transaction: {}", e))?;

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

    // ========== Wall Post Operations ==========

    /// Insert a wall post into relay storage.
    ///
    /// Uses INSERT OR REPLACE so that re-submitting the same post_id
    /// (e.g. after a retry) is idempotent.
    #[allow(clippy::too_many_arguments)] // Database insert mirrors the wall_posts schema columns.
    pub fn insert_wall_post(
        &self,
        post_id: &str,
        author_peer_id: &str,
        content_type: &str,
        content_text: Option<&str>,
        visibility: &str,
        lamport_clock: i64,
        created_at: i64,
        signature: &[u8],
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().timestamp();
        let existing_deleted_at: Option<Option<i64>> = conn
            .query_row(
                "SELECT deleted_at FROM wall_posts WHERE post_id = ? AND author_peer_id = ?",
                params![post_id, author_peer_id],
                |row| row.get(0),
            )
            .optional()?;
        if matches!(existing_deleted_at, Some(Some(_))) {
            return Ok(());
        }

        conn.execute(
            "INSERT OR REPLACE INTO wall_posts
                (post_id, author_peer_id, content_type, content_text, visibility,
                 lamport_clock, created_at, signature, stored_at, deleted_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)",
            params![
                post_id,
                author_peer_id,
                content_type,
                content_text,
                visibility,
                lamport_clock,
                created_at,
                signature,
                now,
            ],
        )?;
        Ok(())
    }

    /// Retrieve wall posts for a given author, optionally filtered by
    /// lamport clock. Returns posts with `lamport_clock > since_lamport_clock`,
    /// ordered oldest-first so callers can advance durable cursors without
    /// duplicating/skipping pages.
    pub fn get_wall_posts(
        &self,
        author_peer_id: &str,
        since_lamport_clock: i64,
        limit: u32,
        include_contacts_only: bool,
    ) -> SqliteResult<Vec<WallPostRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT post_id, author_peer_id, content_type, content_text, visibility,
                    lamport_clock, created_at, deleted_at, signature, stored_at
             FROM wall_posts
             WHERE author_peer_id = ? AND lamport_clock > ?
               AND (? OR visibility = 'public')
             ORDER BY lamport_clock ASC
             LIMIT ?",
        )?;

        let mut posts = Vec::new();
        let mut rows = stmt.query(params![
            author_peer_id,
            since_lamport_clock,
            include_contacts_only,
            limit
        ])?;
        while let Some(row) = rows.next()? {
            posts.push(WallPostRow {
                post_id: row.get(0)?,
                author_peer_id: row.get(1)?,
                content_type: row.get(2)?,
                content_text: row.get(3)?,
                visibility: row.get(4)?,
                lamport_clock: row.get(5)?,
                created_at: row.get(6)?,
                deleted_at: row.get(7)?,
                signature: row.get(8)?,
                stored_at: row.get(9)?,
            });
        }
        Ok(posts)
    }

    /// Insert media metadata for a wall post.
    /// Uses INSERT OR IGNORE so re-submitting the same post_id+media_hash is idempotent.
    #[allow(clippy::too_many_arguments)] // Database insert mirrors the wall_post_media schema columns.
    pub fn insert_wall_post_media(
        &self,
        post_id: &str,
        media_hash: &str,
        media_type: &str,
        mime_type: &str,
        file_name: &str,
        file_size: i64,
        width: Option<i32>,
        height: Option<i32>,
        duration_seconds: Option<i32>,
        sort_order: i32,
        signature: &[u8],
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO wall_post_media
                (post_id, media_hash, media_type, mime_type, file_name, file_size, width, height, duration_seconds, sort_order, signature)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(post_id, media_hash) DO UPDATE SET
                media_type = excluded.media_type,
                mime_type = excluded.mime_type,
                file_name = excluded.file_name,
                file_size = excluded.file_size,
                width = excluded.width,
                height = excluded.height,
                duration_seconds = excluded.duration_seconds,
                sort_order = excluded.sort_order,
                signature = excluded.signature",
            params![post_id, media_hash, media_type, mime_type, file_name, file_size, width, height, duration_seconds, sort_order, signature],
        )?;
        Ok(())
    }

    /// Get media metadata for a wall post.
    pub fn get_wall_post_media(&self, post_id: &str) -> SqliteResult<Vec<WallPostMediaRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT media_hash, media_type, mime_type, file_name, file_size, width, height, duration_seconds, sort_order, signature
             FROM wall_post_media
             WHERE post_id = ?
             ORDER BY sort_order ASC",
        )?;

        let mut items = Vec::new();
        let mut rows = stmt.query([post_id])?;
        while let Some(row) = rows.next()? {
            items.push(WallPostMediaRow {
                media_hash: row.get(0)?,
                media_type: row.get(1)?,
                mime_type: row.get(2)?,
                file_name: row.get(3)?,
                file_size: row.get(4)?,
                width: row.get(5)?,
                height: row.get(6)?,
                duration_seconds: row.get(7)?,
                sort_order: row.get(8)?,
                signature: row.get(9)?,
            });
        }
        Ok(items)
    }

    pub fn insert_wall_social_event(&self, event: &WallSocialEventRow) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();
        let inserted = conn.execute(
            "INSERT OR IGNORE INTO wall_social_events
             (event_id, event_type, post_id, actor_peer_id, author_name, comment_id, content,
              reaction_type, timestamp, payload_cbor, signature, stored_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                event.event_id,
                event.event_type,
                event.post_id,
                event.actor_peer_id,
                event.author_name,
                event.comment_id,
                event.content,
                event.reaction_type,
                event.timestamp,
                event.payload_cbor,
                event.signature,
                chrono::Utc::now().timestamp()
            ],
        )?;
        Ok(inserted > 0)
    }

    pub fn get_wall_social_events(
        &self,
        author_peer_id: &str,
        post_ids: &[String],
        after_timestamp: i64,
        limit: u32,
        can_read_contacts: bool,
    ) -> SqliteResult<Vec<WallSocialEventRow>> {
        if post_ids.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.conn.lock().unwrap();
        let placeholders = (0..post_ids.len())
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        let visibility_filter = if can_read_contacts {
            ""
        } else {
            " AND p.visibility = 'public'"
        };
        let query = format!(
            "SELECT e.event_id, e.event_type, e.post_id, e.actor_peer_id, e.author_name,
                    e.comment_id, e.content, e.reaction_type, e.timestamp, e.payload_cbor, e.signature
             FROM wall_social_events e JOIN wall_posts p ON p.post_id = e.post_id
             WHERE p.author_peer_id = ? AND e.post_id IN ({}) AND e.timestamp > ?{}
             ORDER BY e.timestamp ASC, e.id ASC LIMIT ?",
            placeholders, visibility_filter
        );
        let mut params_vec: Vec<&dyn rusqlite::ToSql> = Vec::new();
        params_vec.push(&author_peer_id);
        for post_id in post_ids {
            params_vec.push(post_id);
        }
        params_vec.push(&after_timestamp);
        let limit_i64 = limit as i64;
        params_vec.push(&limit_i64);
        let mut stmt = conn.prepare(&query)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params_vec), |row| {
            Ok(WallSocialEventRow {
                event_id: row.get(0)?,
                event_type: row.get(1)?,
                post_id: row.get(2)?,
                actor_peer_id: row.get(3)?,
                author_name: row.get(4)?,
                comment_id: row.get(5)?,
                content: row.get(6)?,
                reaction_type: row.get(7)?,
                timestamp: row.get(8)?,
                payload_cbor: row.get(9)?,
                signature: row.get(10)?,
            })
        })?;
        rows.collect()
    }

    /// Persist a wall post delete tombstone. Returns true if a row existed or was created.
    pub fn tombstone_wall_post(
        &self,
        post_id: &str,
        author_peer_id: &str,
        lamport_clock: u64,
        deleted_at: i64,
        signature: &[u8],
    ) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();
        let existing: Option<(String, Option<String>, String, i64)> = conn
            .query_row(
                "SELECT content_type, content_text, visibility, lamport_clock FROM wall_posts WHERE post_id = ? AND author_peer_id = ?",
                params![post_id, author_peer_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;

        if let Some((content_type, content_text, visibility, existing_lamport)) = existing {
            if existing_lamport > lamport_clock as i64 {
                return Ok(true);
            }
            conn.execute(
                "UPDATE wall_posts
                 SET content_type = ?, content_text = ?, lamport_clock = ?, deleted_at = ?, signature = ?, stored_at = ?
                 WHERE post_id = ? AND author_peer_id = ?",
                params![content_type, content_text, lamport_clock as i64, deleted_at, signature, chrono::Utc::now().timestamp(), post_id, author_peer_id],
            )?;
            let _ = visibility;
            return Ok(true);
        }

        conn.execute(
            "INSERT INTO wall_posts
                (post_id, author_peer_id, content_type, content_text, visibility,
                 lamport_clock, created_at, signature, stored_at, deleted_at)
             VALUES (?, ?, 'tombstone', NULL, 'contacts', ?, ?, ?, ?, ?)",
            params![
                post_id,
                author_peer_id,
                lamport_clock as i64,
                deleted_at,
                signature,
                chrono::Utc::now().timestamp(),
                deleted_at,
            ],
        )?;
        Ok(true)
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

/// A wall post row from the database
#[derive(Debug, Clone)]
pub struct WallPostRow {
    pub post_id: String,
    pub author_peer_id: String,
    pub content_type: String,
    pub content_text: Option<String>,
    pub visibility: String,
    pub lamport_clock: i64,
    pub created_at: i64,
    pub deleted_at: Option<i64>,
    pub signature: Vec<u8>,
    pub stored_at: i64,
}

/// A wall post media metadata row from the database
#[derive(Debug, Clone)]
pub struct WallPostMediaRow {
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
