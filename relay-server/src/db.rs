//! Relay server SQLite database for community board data

use crate::resource_limits::DEFAULT_MAX_STORAGE_BYTES;
use rusqlite::{
    params, Connection, OptionalExtension, Result as SqliteResult, TransactionBehavior,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tracing::info;

pub const RELAY_INTEGER_RANGE: &str = "RELAY_INTEGER_RANGE";

fn u64_to_sql_i64(value: u64, field: &str) -> SqliteResult<i64> {
    i64::try_from(value).map_err(|_| {
        rusqlite::Error::InvalidParameterName(format!("{RELAY_INTEGER_RANGE}:{field}:{value}"))
    })
}

fn sql_i64_to_u64(value: i64, column: usize) -> SqliteResult<u64> {
    u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(column, value))
}

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
    registration_timestamp INTEGER NOT NULL DEFAULT 0,
    identity_state TEXT NOT NULL DEFAULT 'unverified' CHECK(identity_state IN ('verified','unverified')),
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

CREATE TABLE IF NOT EXISTS introduction_envelopes (
 request_id TEXT PRIMARY KEY,
 target_peer_id TEXT NOT NULL,
 requester_peer_id TEXT NOT NULL,
 requester_ephemeral_key BLOB NOT NULL,
 ciphertext BLOB NOT NULL,
 issued_at INTEGER NOT NULL,
 expires_at INTEGER NOT NULL,
 stored_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS introduction_target_expiry
 ON introduction_envelopes(target_peer_id, expires_at, stored_at);

CREATE TABLE IF NOT EXISTS relay_resource_limits (
    singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
    record_retention_secs INTEGER NOT NULL CHECK(record_retention_secs > 0),
    max_known_peers INTEGER NOT NULL CHECK(max_known_peers > 0),
    max_posts INTEGER NOT NULL CHECK(max_posts > 0),
    max_grants INTEGER NOT NULL CHECK(max_grants > 0),
    max_introductions INTEGER NOT NULL CHECK(max_introductions > 0),
    max_social_events INTEGER NOT NULL CHECK(max_social_events > 0)
);

CREATE TRIGGER IF NOT EXISTS relay_bound_known_peers BEFORE INSERT ON known_peers
WHEN NOT EXISTS (SELECT 1 FROM known_peers WHERE peer_id = NEW.peer_id)
 AND (SELECT COUNT(*) FROM known_peers) >=
     (SELECT max_known_peers FROM relay_resource_limits WHERE singleton = 1)
BEGIN SELECT RAISE(ABORT, 'RELAY_CAPACITY_KNOWN_PEERS'); END;

CREATE TRIGGER IF NOT EXISTS relay_bound_board_posts BEFORE INSERT ON board_posts
WHEN NOT EXISTS (SELECT 1 FROM board_posts WHERE post_id = NEW.post_id)
 AND ((SELECT COUNT(*) FROM board_posts) + (SELECT COUNT(*) FROM wall_posts)) >=
     (SELECT max_posts FROM relay_resource_limits WHERE singleton = 1)
BEGIN SELECT RAISE(ABORT, 'RELAY_CAPACITY_POSTS'); END;

CREATE TRIGGER IF NOT EXISTS relay_bound_wall_posts BEFORE INSERT ON wall_posts
WHEN NOT EXISTS (SELECT 1 FROM wall_posts WHERE post_id = NEW.post_id)
 AND ((SELECT COUNT(*) FROM board_posts) + (SELECT COUNT(*) FROM wall_posts)) >=
     (SELECT max_posts FROM relay_resource_limits WHERE singleton = 1)
BEGIN SELECT RAISE(ABORT, 'RELAY_CAPACITY_POSTS'); END;

CREATE TRIGGER IF NOT EXISTS relay_bound_grants BEFORE INSERT ON wall_read_grants
WHEN NOT EXISTS (SELECT 1 FROM wall_read_grants WHERE grant_id = NEW.grant_id)
 AND (SELECT COUNT(*) FROM wall_read_grants) >=
     (SELECT max_grants FROM relay_resource_limits WHERE singleton = 1)
BEGIN SELECT RAISE(ABORT, 'RELAY_CAPACITY_GRANTS'); END;

CREATE TRIGGER IF NOT EXISTS relay_bound_introductions BEFORE INSERT ON introduction_envelopes
WHEN NOT EXISTS (SELECT 1 FROM introduction_envelopes WHERE request_id = NEW.request_id)
 AND (SELECT COUNT(*) FROM introduction_envelopes) >=
     (SELECT max_introductions FROM relay_resource_limits WHERE singleton = 1)
BEGIN SELECT RAISE(ABORT, 'RELAY_CAPACITY_INTRODUCTIONS'); END;

CREATE TRIGGER IF NOT EXISTS relay_bound_social_events BEFORE INSERT ON wall_social_events
WHEN NOT EXISTS (SELECT 1 FROM wall_social_events WHERE event_id = NEW.event_id)
 AND (SELECT COUNT(*) FROM wall_social_events) >=
     (SELECT max_social_events FROM relay_resource_limits WHERE singleton = 1)
BEGIN SELECT RAISE(ABORT, 'RELAY_CAPACITY_SOCIAL_EVENTS'); END;
"#;

/// Relay server database
#[derive(Clone)]
pub struct RelayDatabase {
    conn: Arc<Mutex<Connection>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetentionLimits {
    pub record_retention_secs: i64,
    pub max_known_peers: u64,
    pub max_posts: u64,
    pub max_grants: u64,
    pub max_introductions: u64,
    pub max_social_events: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerRegistrationStoreError {
    Database,
    KeySubstitution,
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallPostWriteError {
    Database,
    OwnerConflict,
    StaleClock,
    Tombstoned,
}

impl WallPostWriteError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Database => "RELAY_POST_DATABASE",
            Self::OwnerConflict => "RELAY_POST_OWNER_CONFLICT",
            Self::StaleClock => "RELAY_POST_STALE_CLOCK",
            Self::Tombstoned => "RELAY_POST_TOMBSTONED",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallPostDeleteOutcome {
    Applied,
    AlreadyApplied,
}

pub struct WallPostMediaWrite<'a> {
    pub media_hash: &'a str,
    pub media_type: &'a str,
    pub mime_type: &'a str,
    pub file_name: &'a str,
    pub file_size: i64,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub duration_seconds: Option<i32>,
    pub sort_order: i32,
    pub signature: &'a [u8],
}

impl PeerRegistrationStoreError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Database => "RELAY_PEER_DATABASE",
            Self::KeySubstitution => "RELAY_PEER_KEY_SUBSTITUTION",
            Self::Stale => "RELAY_PEER_REGISTRATION_STALE",
        }
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WallSocialEventWriteError {
    Database,
    MissingPost,
    Unauthorized,
    ActorIdentity,
    EventConflict,
}

impl WallSocialEventWriteError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Database => "RELAY_SOCIAL_DATABASE",
            Self::MissingPost => "RELAY_SOCIAL_POST_NOT_FOUND",
            Self::Unauthorized => "RELAY_SOCIAL_UNAUTHORIZED",
            Self::ActorIdentity => "RELAY_SOCIAL_ACTOR_IDENTITY",
            Self::EventConflict => "RELAY_SOCIAL_EVENT_CONFLICT",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WallSocialEventWriteOutcome {
    Inserted,
    Duplicate,
}

impl RelayDatabase {
    pub fn with_connection<T>(&self, operation: impl FnOnce(&mut Connection) -> T) -> T {
        let mut connection = self.conn.lock().expect("relay database lock poisoned");
        operation(&mut connection)
    }
    /// Open or create the database at the given path
    #[allow(dead_code)] // The binary uses an explicit budget; library consumers and tests use the validated default.
    pub fn open(path: &str) -> SqliteResult<Self> {
        Self::open_with_max_bytes(path, DEFAULT_MAX_STORAGE_BYTES)
    }

    /// Open or create a database with a hard SQLite page budget.
    pub fn open_with_max_bytes(path: &str, max_storage_bytes: u64) -> SqliteResult<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        Self::configure_storage_budget(&conn, max_storage_bytes)?;
        conn.execute_batch(SCHEMA)?;

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        db.ensure_wall_post_tombstone_columns()?;
        db.ensure_wall_media_columns()?;
        db.ensure_known_peer_binding_columns()?;

        // Create default "General" board if none exist
        db.ensure_default_board()?;

        info!(
            "Relay database initialized at {} with max_storage_bytes={}",
            path, max_storage_bytes
        );
        Ok(db)
    }

    /// Installs the validated cardinality model and prunes expired records.
    /// The values live in SQLite so trigger enforcement remains atomic and is
    /// restored before any write after a process restart.
    pub fn configure_retention(&self, limits: RetentionLimits, at: i64) -> SqliteResult<()> {
        if limits.record_retention_secs <= 0
            || limits.max_known_peers == 0
            || limits.max_posts == 0
            || limits.max_grants == 0
            || limits.max_introductions == 0
            || limits.max_social_events == 0
        {
            return Err(rusqlite::Error::InvalidParameterName(
                "relay retention limits must be positive".into(),
            ));
        }
        let mut conn = self.conn.lock().expect("relay database lock poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO relay_resource_limits
             (singleton, record_retention_secs, max_known_peers, max_posts, max_grants,
              max_introductions, max_social_events)
             VALUES (1, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(singleton) DO UPDATE SET
              record_retention_secs=excluded.record_retention_secs,
              max_known_peers=excluded.max_known_peers,
              max_posts=excluded.max_posts,
              max_grants=excluded.max_grants,
              max_introductions=excluded.max_introductions,
              max_social_events=excluded.max_social_events",
            params![
                limits.record_retention_secs,
                limits.max_known_peers,
                limits.max_posts,
                limits.max_grants,
                limits.max_introductions,
                limits.max_social_events,
            ],
        )?;
        Self::prune_connection(&tx, at, limits.record_retention_secs)?;
        for (name, count, maximum) in [
            (
                "known_peers",
                tx.query_row("SELECT COUNT(*) FROM known_peers", [], |row| {
                    row.get::<_, u64>(0)
                })?,
                limits.max_known_peers,
            ),
            (
                "posts",
                tx.query_row(
                    "SELECT (SELECT COUNT(*) FROM board_posts) + (SELECT COUNT(*) FROM wall_posts)",
                    [],
                    |row| row.get::<_, u64>(0),
                )?,
                limits.max_posts,
            ),
            (
                "grants",
                tx.query_row("SELECT COUNT(*) FROM wall_read_grants", [], |row| {
                    row.get::<_, u64>(0)
                })?,
                limits.max_grants,
            ),
            (
                "introductions",
                tx.query_row("SELECT COUNT(*) FROM introduction_envelopes", [], |row| {
                    row.get::<_, u64>(0)
                })?,
                limits.max_introductions,
            ),
            (
                "social_events",
                tx.query_row("SELECT COUNT(*) FROM wall_social_events", [], |row| {
                    row.get::<_, u64>(0)
                })?,
                limits.max_social_events,
            ),
        ] {
            if count > maximum {
                return Err(rusqlite::Error::InvalidParameterName(format!(
                    "existing {name} cardinality {count} exceeds configured maximum {maximum}"
                )));
            }
        }
        tx.commit()
    }

    pub fn enforce_retention(&self, at: i64) -> SqliteResult<usize> {
        let mut conn = self.conn.lock().expect("relay database lock poisoned");
        let retention: Option<i64> = conn
            .query_row(
                "SELECT record_retention_secs FROM relay_resource_limits WHERE singleton=1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        let Some(retention) = retention else {
            return Ok(0);
        };
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let removed = Self::prune_connection(&tx, at, retention)?;
        tx.commit()?;
        Ok(removed)
    }

    fn prune_connection(conn: &Connection, at: i64, retention_secs: i64) -> SqliteResult<usize> {
        let cutoff = at.saturating_sub(retention_secs);
        let mut removed = 0;
        removed += conn.execute(
            "DELETE FROM introduction_envelopes WHERE expires_at <= ? OR stored_at < ?",
            params![at, cutoff],
        )?;
        removed += conn.execute(
            "DELETE FROM wall_social_events WHERE stored_at < ?",
            [cutoff],
        )?;
        removed += conn.execute("DELETE FROM board_posts WHERE created_at < ?", [cutoff])?;
        removed += conn.execute("DELETE FROM wall_posts WHERE stored_at < ?", [cutoff])?;
        removed += conn.execute(
            "DELETE FROM wall_read_grants
             WHERE (expires_at IS NOT NULL AND expires_at <= ?)
                OR (revoked_at IS NOT NULL AND revoked_at < ?)",
            params![at, cutoff],
        )?;
        removed += conn.execute(
            "DELETE FROM known_peers
             WHERE identity_state='unverified' AND last_seen_at < ?
               AND NOT EXISTS (
                 SELECT 1 FROM relay_name_claims c
                 WHERE c.peer_id=known_peers.peer_id AND c.status='active'
               )",
            [cutoff],
        )?;
        Ok(removed)
    }

    fn configure_storage_budget(conn: &Connection, max_storage_bytes: u64) -> SqliteResult<()> {
        let page_size: u64 = conn.pragma_query_value(None, "page_size", |row| row.get(0))?;
        let max_pages = max_storage_bytes / page_size;
        if max_pages == 0 || max_pages > i64::MAX as u64 {
            return Err(rusqlite::Error::InvalidParameterName(format!(
                "max_storage_bytes={max_storage_bytes} cannot produce a finite SQLite page budget"
            )));
        }

        let current_pages: u64 = conn.pragma_query_value(None, "page_count", |row| row.get(0))?;
        if current_pages > max_pages {
            return Err(rusqlite::Error::InvalidParameterName(format!(
                "existing relay database uses {} bytes, above max_storage_bytes={max_storage_bytes}",
                current_pages.saturating_mul(page_size)
            )));
        }

        conn.pragma_update(None, "max_page_count", max_pages as i64)?;
        let effective_max_pages: u64 =
            conn.pragma_query_value(None, "max_page_count", |row| row.get(0))?;
        if effective_max_pages > max_pages {
            return Err(rusqlite::Error::InvalidParameterName(format!(
                "SQLite refused max_storage_bytes={max_storage_bytes}"
            )));
        }

        let journal_limit = (max_storage_bytes / 16).clamp(1_048_576, 67_108_864);
        conn.pragma_update(None, "journal_size_limit", journal_limit as i64)?;
        Ok(())
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

    fn ensure_known_peer_binding_columns(&self) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let has_registration_timestamp: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('known_peers') WHERE name = 'registration_timestamp'",
            [],
            |row| row.get(0),
        )?;
        if !has_registration_timestamp {
            conn.execute(
                "ALTER TABLE known_peers ADD COLUMN registration_timestamp INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
        }
        let has_identity_state: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('known_peers') WHERE name = 'identity_state'",
            [],
            |row| row.get(0),
        )?;
        if !has_identity_state {
            conn.execute(
                "ALTER TABLE known_peers ADD COLUMN identity_state TEXT NOT NULL DEFAULT 'unverified' CHECK(identity_state IN ('verified','unverified'))",
                [],
            )?;
        }
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
        let lamport_clock = u64_to_sql_i64(lamport_clock, "lamport_clock")?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO board_posts (post_id, board_id, author_peer_id, content_type, content_text, lamport_clock, created_at, signature)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![post_id, board_id, author_peer_id, content_type, content_text, lamport_clock, created_at, signature],
        )?;
        Ok(())
    }

    /// Compatibility entry point for the timestamp-only new-since protocol.
    /// New code should use a compound cursor.
    pub fn get_board_posts(
        &self,
        board_id: &str,
        after_timestamp: Option<i64>,
        limit: u32,
    ) -> SqliteResult<Vec<PostRow>> {
        let cursor = after_timestamp.map(|created_at| BoardPostCursor {
            created_at,
            post_id: String::new(),
        });
        if cursor.is_some() {
            self.get_board_posts_newer(board_id, cursor.as_ref(), limit)
        } else {
            self.get_board_posts_older(board_id, None, limit)
        }
    }

    /// Return the page older than `before` in stable newest-first order.
    pub fn get_board_posts_older(
        &self,
        board_id: &str,
        before: Option<&BoardPostCursor>,
        limit: u32,
    ) -> SqliteResult<Vec<PostRow>> {
        let conn = self.conn.lock().unwrap();
        let mut posts = Vec::new();

        if let Some(before) = before {
            let mut stmt = conn.prepare(
                "SELECT bp.post_id, bp.board_id, bp.author_peer_id, bp.content_type, bp.content_text,
                        bp.lamport_clock, bp.created_at, bp.deleted_at, bp.signature,
                        kp.display_name
                 FROM board_posts bp
                 LEFT JOIN known_peers kp ON bp.author_peer_id = kp.peer_id
                 WHERE bp.board_id = ?
                   AND (bp.created_at < ? OR (bp.created_at = ? AND bp.post_id < ?))
                 ORDER BY bp.created_at DESC, bp.post_id DESC
                 LIMIT ?",
            )?;
            let mut rows = stmt.query(params![
                board_id,
                before.created_at,
                before.created_at,
                before.post_id,
                limit
            ])?;
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
                 ORDER BY bp.created_at DESC, bp.post_id DESC
                 LIMIT ?",
            )?;
            let mut rows = stmt.query(params![board_id, limit])?;
            while let Some(row) = rows.next()? {
                posts.push(Self::row_to_post(row)?);
            }
        }
        Ok(posts)
    }

    /// Return the page newer than `after` in stable oldest-first order.
    pub fn get_board_posts_newer(
        &self,
        board_id: &str,
        after: Option<&BoardPostCursor>,
        limit: u32,
    ) -> SqliteResult<Vec<PostRow>> {
        let conn = self.conn.lock().unwrap();
        let mut posts = Vec::new();
        if let Some(after) = after {
            let mut stmt = conn.prepare(
                "SELECT bp.post_id, bp.board_id, bp.author_peer_id, bp.content_type, bp.content_text,
                        bp.lamport_clock, bp.created_at, bp.deleted_at, bp.signature,
                        kp.display_name
                 FROM board_posts bp
                 LEFT JOIN known_peers kp ON bp.author_peer_id = kp.peer_id
                 WHERE bp.board_id = ?
                   AND (bp.created_at > ? OR (bp.created_at = ? AND bp.post_id > ?))
                 ORDER BY bp.created_at ASC, bp.post_id ASC
                 LIMIT ?",
            )?;
            let mut rows = stmt.query(params![
                board_id,
                after.created_at,
                after.created_at,
                after.post_id,
                limit
            ])?;
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
                 ORDER BY bp.created_at ASC, bp.post_id ASC
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
            lamport_clock: sql_i64_to_u64(row.get::<_, i64>(5)?, 5)?,
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
        registration_timestamp: i64,
        identity_state: &str,
        server_now: i64,
    ) -> Result<(), PeerRegistrationStoreError> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|_| PeerRegistrationStoreError::Database)?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| PeerRegistrationStoreError::Database)?;
        let existing = tx
            .query_row(
                "SELECT public_key, registration_timestamp FROM known_peers WHERE peer_id = ?",
                [peer_id],
                |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()
            .map_err(|_| PeerRegistrationStoreError::Database)?;
        if let Some((stored_key, stored_timestamp)) = existing {
            if stored_key != public_key {
                return Err(PeerRegistrationStoreError::KeySubstitution);
            }
            if registration_timestamp <= stored_timestamp {
                return Err(PeerRegistrationStoreError::Stale);
            }
        }
        tx.execute(
            "INSERT INTO known_peers (peer_id, public_key, display_name, registration_timestamp, identity_state, first_seen_at, last_seen_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(peer_id) DO UPDATE SET
                 display_name = excluded.display_name,
                 registration_timestamp = excluded.registration_timestamp,
                 identity_state = excluded.identity_state,
                 last_seen_at = excluded.last_seen_at",
            params![
                peer_id,
                public_key,
                display_name,
                registration_timestamp,
                identity_state,
                server_now,
                server_now
            ],
        )
        .map_err(|_| PeerRegistrationStoreError::Database)?;
        tx.commit()
            .map_err(|_| PeerRegistrationStoreError::Database)?;
        Ok(())
    }

    pub fn peer_identity_state(&self, peer_id: &str, now: i64) -> SqliteResult<String> {
        let conn = self.conn.lock().unwrap();
        let has_active_claim: bool = conn.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM relay_name_claims
                WHERE peer_id = ? AND status = 'active' AND not_before <= ? AND not_after >= ?
            )",
            params![peer_id, now, now],
            |row| row.get(0),
        )?;
        Ok(if has_active_claim {
            "verified".to_string()
        } else {
            "unverified".to_string()
        })
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
        let lamport_clock = u64_to_sql_i64(lamport_clock, "grant_lamport_clock")?;
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
                lamport_clock,
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
        let lamport_clock = u64_to_sql_i64(lamport_clock, "grant_lamport_clock")?;
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "UPDATE wall_read_grants SET revoked_at = ?, lamport_clock = ?
             WHERE grant_id = ? AND issuer_peer_id = ? AND lamport_clock < ?
               AND (revoked_at IS NULL OR revoked_at < ?)",
            params![
                revoked_at,
                lamport_clock,
                grant_id,
                issuer_peer_id,
                lamport_clock,
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
               AND scope_json IS NULL
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
        match last_clock {
            Some(clock) => sql_i64_to_u64(clock, 0),
            None => Ok(0),
        }
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
        let new_clock = u64_to_sql_i64(new_clock, "lamport_clock")?;
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "INSERT INTO author_lamport_clocks (author_peer_id, last_seen_clock, updated_at)
             VALUES (?, ?, ?)
             ON CONFLICT(author_peer_id) DO UPDATE SET
                 last_seen_clock = excluded.last_seen_clock,
                 updated_at = excluded.updated_at",
            params![author_peer_id, new_clock, now],
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
        let lamport_clock_i64 =
            i64::try_from(lamport_clock).map_err(|_| RELAY_INTEGER_RANGE.to_string())?;
        let conn = self.conn.lock().unwrap();

        conn.execute_batch("BEGIN IMMEDIATE")
            .map_err(|e| format!("Failed to begin transaction: {}", e))?;

        // Step 1: Read the current high-water mark for this author.
        let last_seen_clock_i64 = conn
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
            .unwrap_or(0);
        let last_seen_clock = u64::try_from(last_seen_clock_i64).map_err(|_| {
            let _ = conn.execute_batch("ROLLBACK");
            RELAY_INTEGER_RANGE.to_string()
        })?;

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
            params![post_id, board_id, author_peer_id, content_type, content_text, lamport_clock_i64, created_at, signature],
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
            params![author_peer_id, lamport_clock_i64, now],
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

    /// Atomically create or advance an author-owned wall post and replace its
    /// complete signed media manifest. Existing IDs owned by another peer,
    /// stale revisions, and tombstone resurrection all fail before mutation.
    #[allow(clippy::too_many_arguments)]
    pub fn write_wall_post_with_media(
        &self,
        post_id: &str,
        author_peer_id: &str,
        content_type: &str,
        content_text: Option<&str>,
        visibility: &str,
        lamport_clock: i64,
        created_at: i64,
        signature: &[u8],
        media_items: &[WallPostMediaWrite<'_>],
        server_now: i64,
    ) -> Result<(), WallPostWriteError> {
        let mut conn = self.conn.lock().map_err(|_| WallPostWriteError::Database)?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| WallPostWriteError::Database)?;
        let existing = tx
            .query_row(
                "SELECT author_peer_id, lamport_clock, deleted_at FROM wall_posts WHERE post_id = ?",
                [post_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|_| WallPostWriteError::Database)?;

        if let Some((stored_author, stored_clock, deleted_at)) = existing {
            if stored_author != author_peer_id {
                return Err(WallPostWriteError::OwnerConflict);
            }
            if deleted_at.is_some() {
                return Err(WallPostWriteError::Tombstoned);
            }
            if lamport_clock <= stored_clock {
                return Err(WallPostWriteError::StaleClock);
            }
            tx.execute(
                "UPDATE wall_posts
                 SET content_type = ?, content_text = ?, visibility = ?, lamport_clock = ?,
                     created_at = ?, signature = ?, stored_at = ?
                 WHERE post_id = ? AND author_peer_id = ? AND deleted_at IS NULL",
                params![
                    content_type,
                    content_text,
                    visibility,
                    lamport_clock,
                    created_at,
                    signature,
                    server_now,
                    post_id,
                    author_peer_id
                ],
            )
            .map_err(|_| WallPostWriteError::Database)?;
            tx.execute("DELETE FROM wall_post_media WHERE post_id = ?", [post_id])
                .map_err(|_| WallPostWriteError::Database)?;
        } else {
            tx.execute(
                "INSERT INTO wall_posts
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
                    server_now
                ],
            )
            .map_err(|_| WallPostWriteError::Database)?;
        }

        for item in media_items {
            tx.execute(
                "INSERT INTO wall_post_media
                    (post_id, media_hash, media_type, mime_type, file_name, file_size,
                     width, height, duration_seconds, sort_order, signature)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    post_id,
                    item.media_hash,
                    item.media_type,
                    item.mime_type,
                    item.file_name,
                    item.file_size,
                    item.width,
                    item.height,
                    item.duration_seconds,
                    item.sort_order,
                    item.signature
                ],
            )
            .map_err(|_| WallPostWriteError::Database)?;
        }
        tx.commit().map_err(|_| WallPostWriteError::Database)
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

    /// Fetch complete media manifests for a page of posts in one query.
    pub fn get_wall_post_media_batch(
        &self,
        post_ids: &[String],
    ) -> SqliteResult<Vec<(String, Vec<WallPostMediaRow>)>> {
        if post_ids.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.conn.lock().unwrap();
        let placeholders = (0..post_ids.len())
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        let query = format!(
            "SELECT post_id, media_hash, media_type, mime_type, file_name, file_size,
                    width, height, duration_seconds, sort_order, signature
             FROM wall_post_media
             WHERE post_id IN ({})
             ORDER BY post_id, sort_order ASC",
            placeholders
        );
        let mut statement = conn.prepare(&query)?;
        let rows = statement.query_map(rusqlite::params_from_iter(post_ids.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                WallPostMediaRow {
                    media_hash: row.get(1)?,
                    media_type: row.get(2)?,
                    mime_type: row.get(3)?,
                    file_name: row.get(4)?,
                    file_size: row.get(5)?,
                    width: row.get(6)?,
                    height: row.get(7)?,
                    duration_seconds: row.get(8)?,
                    sort_order: row.get(9)?,
                    signature: row.get(10)?,
                },
            ))
        })?;
        let mut grouped = std::collections::BTreeMap::<String, Vec<WallPostMediaRow>>::new();
        for row in rows {
            let (post_id, item) = row?;
            grouped.entry(post_id).or_default().push(item);
        }
        Ok(grouped.into_iter().collect())
    }

    #[cfg(test)]
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

    /// Atomically checks post visibility, actor identity, duplicate integrity,
    /// and storage capacity before admitting a validated social event.
    pub fn insert_authorized_wall_social_event(
        &self,
        event: &WallSocialEventRow,
        expected_author_name: Option<&str>,
        server_now: i64,
    ) -> Result<WallSocialEventWriteOutcome, WallSocialEventWriteError> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|_| WallSocialEventWriteError::Database)?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| WallSocialEventWriteError::Database)?;

        let existing = tx
            .query_row(
                "SELECT event_type,post_id,actor_peer_id,author_name,comment_id,content,
                        reaction_type,timestamp,payload_cbor,signature
                 FROM wall_social_events WHERE event_id=?",
                [&event.event_id],
                |row| {
                    Ok(WallSocialEventRow {
                        event_id: event.event_id.clone(),
                        event_type: row.get(0)?,
                        post_id: row.get(1)?,
                        actor_peer_id: row.get(2)?,
                        author_name: row.get(3)?,
                        comment_id: row.get(4)?,
                        content: row.get(5)?,
                        reaction_type: row.get(6)?,
                        timestamp: row.get(7)?,
                        payload_cbor: row.get(8)?,
                        signature: row.get(9)?,
                    })
                },
            )
            .optional()
            .map_err(|_| WallSocialEventWriteError::Database)?;
        let post: Option<(String, String, Option<i64>)> = tx
            .query_row(
                "SELECT author_peer_id,visibility,deleted_at FROM wall_posts WHERE post_id=?",
                [&event.post_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(|_| WallSocialEventWriteError::Database)?;
        let Some((post_author, visibility, deleted_at)) = post else {
            return Err(WallSocialEventWriteError::MissingPost);
        };
        if deleted_at.is_some() {
            return Err(WallSocialEventWriteError::MissingPost);
        }

        let actor: Option<(String, String)> = tx
            .query_row(
                "SELECT display_name,identity_state FROM known_peers WHERE peer_id=?",
                [&event.actor_peer_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|_| WallSocialEventWriteError::Database)?;
        let Some((display_name, identity_state)) = actor else {
            return Err(WallSocialEventWriteError::ActorIdentity);
        };
        let active_claim: Option<(String, String)> = tx
            .query_row(
                "SELECT local_name,relay FROM relay_name_claims
                 WHERE peer_id=? AND status='active' AND not_before<=? AND not_after>=?
                 ORDER BY sequence DESC LIMIT 1",
                params![event.actor_peer_id, server_now, server_now],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|_| WallSocialEventWriteError::Database)?;
        let canonical_actor_name = match (identity_state.as_str(), active_claim) {
            ("verified", Some((local_name, relay))) => format!("@{local_name}@{relay}"),
            ("unverified", None) => display_name,
            _ => return Err(WallSocialEventWriteError::ActorIdentity),
        };
        if expected_author_name.is_some_and(|name| name != canonical_actor_name) {
            return Err(WallSocialEventWriteError::ActorIdentity);
        }

        let authorized = post_author == event.actor_peer_id
            || visibility == "public"
            || (visibility == "contacts"
                && tx
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM wall_read_grants
                         WHERE issuer_peer_id=? AND subject_peer_id=?
                           AND capability IN ('wall_read','wall:read')
                           AND scope_json IS NULL AND revoked_at IS NULL
                           AND (expires_at IS NULL OR expires_at>=?))",
                        params![post_author, event.actor_peer_id, server_now],
                        |row| row.get::<_, bool>(0),
                    )
                    .map_err(|_| WallSocialEventWriteError::Database)?);
        if !authorized {
            return Err(WallSocialEventWriteError::Unauthorized);
        }
        if event.event_type == "comment_delete" {
            let owns_comment: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM wall_social_events
                     WHERE event_type='comment_create' AND post_id=? AND actor_peer_id=?
                       AND comment_id=? AND timestamp<=?)",
                    params![
                        event.post_id,
                        event.actor_peer_id,
                        event.comment_id,
                        event.timestamp
                    ],
                    |row| row.get(0),
                )
                .map_err(|_| WallSocialEventWriteError::Database)?;
            if !owns_comment {
                return Err(WallSocialEventWriteError::Unauthorized);
            }
        }
        if event.event_type == "reaction_remove" {
            let owns_reaction: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM wall_social_events
                     WHERE event_type='reaction_add' AND post_id=? AND actor_peer_id=?
                       AND reaction_type=? AND timestamp<=?)",
                    params![
                        event.post_id,
                        event.actor_peer_id,
                        event.reaction_type,
                        event.timestamp
                    ],
                    |row| row.get(0),
                )
                .map_err(|_| WallSocialEventWriteError::Database)?;
            if !owns_reaction {
                return Err(WallSocialEventWriteError::Unauthorized);
            }
        }
        if let Some(existing) = existing {
            return if existing.event_type == event.event_type
                && existing.post_id == event.post_id
                && existing.actor_peer_id == event.actor_peer_id
                && existing.author_name == event.author_name
                && existing.comment_id == event.comment_id
                && existing.content == event.content
                && existing.reaction_type == event.reaction_type
                && existing.timestamp == event.timestamp
                && existing.payload_cbor == event.payload_cbor
                && existing.signature == event.signature
            {
                Ok(WallSocialEventWriteOutcome::Duplicate)
            } else {
                Err(WallSocialEventWriteError::EventConflict)
            };
        }

        tx.execute(
            "INSERT INTO wall_social_events
             (event_id,event_type,post_id,actor_peer_id,author_name,comment_id,content,
              reaction_type,timestamp,payload_cbor,signature,stored_at)
             VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",
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
                server_now,
            ],
        )
        .map_err(|_| WallSocialEventWriteError::Database)?;
        tx.commit()
            .map_err(|_| WallSocialEventWriteError::Database)?;
        Ok(WallSocialEventWriteOutcome::Inserted)
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

    /// Persist a wall-post tombstone atomically with ownership, revision, and
    /// attachment cleanup checks.
    pub fn tombstone_wall_post(
        &self,
        post_id: &str,
        author_peer_id: &str,
        lamport_clock: i64,
        deleted_at: i64,
        signature: &[u8],
        server_now: i64,
    ) -> Result<WallPostDeleteOutcome, WallPostWriteError> {
        let mut conn = self.conn.lock().map_err(|_| WallPostWriteError::Database)?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| WallPostWriteError::Database)?;
        let existing: Option<(String, i64, Option<i64>, Vec<u8>)> = tx
            .query_row(
                "SELECT author_peer_id, lamport_clock, deleted_at, signature FROM wall_posts WHERE post_id = ?",
                [post_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(|_| WallPostWriteError::Database)?;

        if let Some((stored_author, stored_clock, stored_deleted_at, stored_signature)) = existing {
            if stored_author != author_peer_id {
                return Err(WallPostWriteError::OwnerConflict);
            }
            if stored_clock > lamport_clock
                || (stored_clock == lamport_clock
                    && (stored_deleted_at != Some(deleted_at) || stored_signature != signature))
            {
                return Err(WallPostWriteError::StaleClock);
            }
            if stored_clock == lamport_clock
                && stored_deleted_at == Some(deleted_at)
                && stored_signature == signature
            {
                return Ok(WallPostDeleteOutcome::AlreadyApplied);
            }
            tx.execute(
                "UPDATE wall_posts
                 SET lamport_clock = ?, deleted_at = ?, signature = ?, stored_at = ?
                 WHERE post_id = ? AND author_peer_id = ?",
                params![
                    lamport_clock,
                    deleted_at,
                    signature,
                    server_now,
                    post_id,
                    author_peer_id
                ],
            )
            .map_err(|_| WallPostWriteError::Database)?;
            tx.execute("DELETE FROM wall_post_media WHERE post_id = ?", [post_id])
                .map_err(|_| WallPostWriteError::Database)?;
            tx.commit().map_err(|_| WallPostWriteError::Database)?;
            return Ok(WallPostDeleteOutcome::Applied);
        }

        tx.execute(
            "INSERT INTO wall_posts
                (post_id, author_peer_id, content_type, content_text, visibility,
                 lamport_clock, created_at, signature, stored_at, deleted_at)
             VALUES (?, ?, 'tombstone', NULL, 'contacts', ?, ?, ?, ?, ?)",
            params![
                post_id,
                author_peer_id,
                lamport_clock,
                deleted_at,
                signature,
                server_now,
                deleted_at,
            ],
        )
        .map_err(|_| WallPostWriteError::Database)?;
        tx.commit().map_err(|_| WallPostWriteError::Database)?;
        Ok(WallPostDeleteOutcome::Applied)
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

/// Stable board pagination position. The post id disambiguates equal timestamps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardPostCursor {
    pub created_at: i64,
    pub post_id: String,
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

#[cfg(test)]
mod storage_budget_tests {
    use super::*;

    fn retention_limits() -> RetentionLimits {
        RetentionLimits {
            record_retention_secs: 10,
            max_known_peers: 1,
            max_posts: 2,
            max_grants: 1,
            max_introductions: 1,
            max_social_events: 1,
        }
    }

    #[test]
    fn sqlite_page_and_journal_budgets_are_finite_and_inspectable() {
        let database = RelayDatabase::open_with_max_bytes(":memory:", 16_777_216).unwrap();
        database.with_connection(|connection| {
            let page_size: u64 = connection
                .pragma_query_value(None, "page_size", |row| row.get(0))
                .unwrap();
            let max_pages: u64 = connection
                .pragma_query_value(None, "max_page_count", |row| row.get(0))
                .unwrap();
            let journal_limit: i64 = connection
                .pragma_query_value(None, "journal_size_limit", |row| row.get(0))
                .unwrap();
            assert!(max_pages.saturating_mul(page_size) <= 16_777_216);
            assert!((1_048_576..=67_108_864).contains(&(journal_limit as u64)));
        });
    }

    #[test]
    fn existing_database_over_budget_fails_closed() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("relay.db");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch("CREATE TABLE oversized (value BLOB); INSERT INTO oversized VALUES (zeroblob(1048576));")
            .unwrap();
        drop(connection);

        assert!(RelayDatabase::open_with_max_bytes(path.to_str().unwrap(), 262_144).is_err());
    }

    #[test]
    fn persistent_collections_have_atomic_hard_cardinality_bounds() {
        let database = RelayDatabase::open(":memory:").unwrap();
        database
            .configure_retention(retention_limits(), 100)
            .unwrap();
        database.with_connection(|connection| {
            connection.execute("INSERT INTO boards(board_id,name,created_at,is_default) VALUES('test-board','Test',100,0)", []).unwrap();
            connection.execute("INSERT INTO known_peers(peer_id,public_key,display_name,registration_timestamp,identity_state,first_seen_at,last_seen_at) VALUES('p1',X'01','one',100,'unverified',100,100)", []).unwrap();
            assert!(connection.execute("INSERT INTO known_peers(peer_id,public_key,display_name,registration_timestamp,identity_state,first_seen_at,last_seen_at) VALUES('p2',X'02','two',100,'unverified',100,100)", []).is_err());

            connection.execute("INSERT INTO board_posts(post_id,board_id,author_peer_id,content_type,lamport_clock,created_at,signature) VALUES('b1','test-board','p1','text',1,100,X'01')", []).unwrap();
            connection.execute("INSERT INTO wall_posts(post_id,author_peer_id,content_type,visibility,lamport_clock,created_at,signature,stored_at) VALUES('w1','p1','text','public',1,100,X'01',100)", []).unwrap();
            assert!(connection.execute("INSERT INTO wall_posts(post_id,author_peer_id,content_type,visibility,lamport_clock,created_at,signature,stored_at) VALUES('w2','p1','text','public',2,100,X'01',100)", []).is_err());

            connection.execute("INSERT INTO wall_read_grants VALUES('g1','p1','p2','wall_read',NULL,1,100,NULL,X'01',NULL)", []).unwrap();
            assert!(connection.execute("INSERT INTO wall_read_grants VALUES('g2','p1','p3','wall_read',NULL,2,100,NULL,X'01',NULL)", []).is_err());

            connection.execute("INSERT INTO introduction_envelopes VALUES('i1','p1','p2',X'01',X'02',100,200,100)", []).unwrap();
            assert!(connection.execute("INSERT INTO introduction_envelopes VALUES('i2','p1','p3',X'01',X'02',100,200,100)", []).is_err());

            connection.execute("INSERT INTO wall_social_events(event_id,event_type,post_id,actor_peer_id,timestamp,payload_cbor,signature,stored_at) VALUES('e1','reaction','w1','p1',100,X'01',X'02',100)", []).unwrap();
            assert!(connection.execute("INSERT INTO wall_social_events(event_id,event_type,post_id,actor_peer_id,timestamp,payload_cbor,signature,stored_at) VALUES('e2','reaction','w1','p2',100,X'01',X'02',100)", []).is_err());
        });
    }

    #[test]
    fn expiry_prunes_all_retained_record_classes_deterministically() {
        let database = RelayDatabase::open(":memory:").unwrap();
        let mut limits = retention_limits();
        limits.max_known_peers = 10;
        limits.max_posts = 10;
        limits.max_grants = 10;
        limits.max_introductions = 10;
        limits.max_social_events = 10;
        database.configure_retention(limits, 100).unwrap();
        database.with_connection(|connection| {
            connection.execute("INSERT INTO boards(board_id,name,created_at,is_default) VALUES('test-board','Test',100,0)", []).unwrap();
            connection.execute("INSERT INTO known_peers(peer_id,public_key,display_name,registration_timestamp,identity_state,first_seen_at,last_seen_at) VALUES('old-peer',X'01','old',1,'unverified',1,1)", []).unwrap();
            connection.execute("INSERT INTO board_posts(post_id,board_id,author_peer_id,content_type,lamport_clock,created_at,signature) VALUES('old-board','test-board','old-peer','text',1,1,X'01')", []).unwrap();
            connection.execute("INSERT INTO wall_posts(post_id,author_peer_id,content_type,visibility,lamport_clock,created_at,signature,stored_at) VALUES('old-wall','old-peer','text','public',1,1,X'01',1)", []).unwrap();
            connection.execute("INSERT INTO wall_read_grants VALUES('old-grant','old-peer','subject','wall_read',NULL,1,1,5,X'01',NULL)", []).unwrap();
            connection.execute("INSERT INTO introduction_envelopes VALUES('old-intro','old-peer','subject',X'01',X'02',1,5,1)", []).unwrap();
            connection.execute("INSERT INTO wall_social_events(event_id,event_type,post_id,actor_peer_id,timestamp,payload_cbor,signature,stored_at) VALUES('old-event','reaction','old-wall','old-peer',1,X'01',X'02',1)", []).unwrap();
        });
        let removed = database.enforce_retention(100).unwrap();
        assert!(removed >= 6);
        database.with_connection(|connection| {
            for table in [
                "known_peers",
                "board_posts",
                "wall_posts",
                "wall_read_grants",
                "introduction_envelopes",
                "wall_social_events",
            ] {
                let count: i64 = connection
                    .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                        row.get(0)
                    })
                    .unwrap();
                assert_eq!(count, 0, "expired rows remain in {table}");
            }
        });
    }

    #[test]
    fn restart_preserves_capacity_enforcement_and_prunes_expired_storage() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("relay.db");
        {
            let database = RelayDatabase::open(path.to_str().unwrap()).unwrap();
            database
                .configure_retention(retention_limits(), 100)
                .unwrap();
            database.with_connection(|connection| {
                connection.execute("INSERT INTO introduction_envelopes VALUES('expired','p1','p2',X'01',X'02',1,5,1)", []).unwrap();
            });
        }
        let reopened = RelayDatabase::open(path.to_str().unwrap()).unwrap();
        reopened.enforce_retention(100).unwrap();
        reopened.with_connection(|connection| {
            let count: i64 = connection.query_row("SELECT COUNT(*) FROM introduction_envelopes", [], |row| row.get(0)).unwrap();
            assert_eq!(count, 0);
            connection.execute("INSERT INTO introduction_envelopes VALUES('one','p1','p2',X'01',X'02',100,200,100)", []).unwrap();
            assert!(connection.execute("INSERT INTO introduction_envelopes VALUES('two','p1','p3',X'01',X'02',100,200,100)", []).is_err());
        });
    }

    #[test]
    fn lowering_a_limit_below_live_cardinality_fails_closed() {
        let database = RelayDatabase::open(":memory:").unwrap();
        let mut initial = retention_limits();
        initial.max_introductions = 2;
        database.configure_retention(initial, 100).unwrap();
        database.with_connection(|connection| {
            connection.execute("INSERT INTO introduction_envelopes VALUES('one','p1','p2',X'01',X'02',100,200,100)", []).unwrap();
            connection.execute("INSERT INTO introduction_envelopes VALUES('two','p1','p3',X'01',X'02',100,200,100)", []).unwrap();
        });
        assert!(database
            .configure_retention(retention_limits(), 100)
            .is_err());
    }
}

#[cfg(test)]
mod pagination_integer_tests {
    use super::*;

    fn insert_fixture_posts(database: &RelayDatabase) {
        database.with_connection(|connection| {
            connection
                .execute(
                    "INSERT OR IGNORE INTO boards(board_id,name,created_at,is_default) VALUES('general','General test',100,0)",
                    [],
                )
                .unwrap();
        });
        for (index, (post_id, created_at)) in [
            ("post-a", 100),
            ("post-b", 100),
            ("post-c", 100),
            ("post-d", 99),
            ("post-e", 99),
            ("post-f", 98),
        ]
        .into_iter()
        .enumerate()
        {
            database
                .insert_post(
                    post_id,
                    "general",
                    "author",
                    "text",
                    None,
                    u64::try_from(index + 1).unwrap(),
                    created_at,
                    &[1],
                )
                .unwrap();
        }
    }

    #[test]
    fn older_compound_cursor_has_no_duplicates_or_gaps_across_restart() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("relay.db");
        let first = RelayDatabase::open(path.to_str().unwrap()).unwrap();
        insert_fixture_posts(&first);

        let page_one = first.get_board_posts_older("general", None, 2).unwrap();
        assert_eq!(
            page_one
                .iter()
                .map(|post| post.post_id.as_str())
                .collect::<Vec<_>>(),
            ["post-c", "post-b"]
        );
        let mut cursor = BoardPostCursor {
            created_at: page_one.last().unwrap().created_at,
            post_id: page_one.last().unwrap().post_id.clone(),
        };
        let mut ids = page_one
            .into_iter()
            .map(|post| post.post_id)
            .collect::<Vec<_>>();
        drop(first);

        let reopened = RelayDatabase::open(path.to_str().unwrap()).unwrap();
        loop {
            let page = reopened
                .get_board_posts_older("general", Some(&cursor), 2)
                .unwrap();
            if page.is_empty() {
                break;
            }
            cursor = BoardPostCursor {
                created_at: page.last().unwrap().created_at,
                post_id: page.last().unwrap().post_id.clone(),
            };
            ids.extend(page.into_iter().map(|post| post.post_id));
        }

        assert_eq!(
            ids,
            ["post-c", "post-b", "post-a", "post-e", "post-d", "post-f"]
        );
        let unique = ids.iter().collect::<std::collections::HashSet<_>>();
        assert_eq!(unique.len(), ids.len());
    }

    #[test]
    fn newer_compound_cursor_traverses_equal_timestamps_once() {
        let database = RelayDatabase::open(":memory:").unwrap();
        insert_fixture_posts(&database);
        let mut cursor = None;
        let mut ids = Vec::new();
        loop {
            let page = database
                .get_board_posts_newer("general", cursor.as_ref(), 2)
                .unwrap();
            if page.is_empty() {
                break;
            }
            let last = page.last().unwrap();
            cursor = Some(BoardPostCursor {
                created_at: last.created_at,
                post_id: last.post_id.clone(),
            });
            ids.extend(page.into_iter().map(|post| post.post_id));
        }
        assert_eq!(
            ids,
            ["post-f", "post-d", "post-e", "post-a", "post-b", "post-c"]
        );
    }

    #[test]
    fn sqlite_integer_boundaries_are_checked() {
        assert_eq!(u64_to_sql_i64(0, "clock").unwrap(), 0);
        assert_eq!(u64_to_sql_i64(i64::MAX as u64, "clock").unwrap(), i64::MAX);
        assert!(u64_to_sql_i64(i64::MAX as u64 + 1, "clock").is_err());
        assert!(u64_to_sql_i64(u64::MAX, "clock").is_err());
        assert_eq!(sql_i64_to_u64(0, 0).unwrap(), 0);
        assert_eq!(sql_i64_to_u64(i64::MAX, 0).unwrap(), i64::MAX as u64);
        assert!(sql_i64_to_u64(-1, 0).is_err());
    }

    #[test]
    fn negative_database_clock_is_rejected_instead_of_wrapping() {
        let database = RelayDatabase::open(":memory:").unwrap();
        database.with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO boards(board_id,name,created_at,is_default) VALUES('general','General test',100,0)",
                    [],
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO board_posts(post_id,board_id,author_peer_id,content_type,lamport_clock,created_at,signature) VALUES('negative','general','author','text',-1,100,X'01')",
                    [],
                )
                .unwrap();
        });
        assert!(database.get_board_posts_older("general", None, 10).is_err());
    }
}
