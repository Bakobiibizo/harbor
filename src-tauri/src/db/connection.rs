use rusqlite::{Connection, Result as SqliteResult};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing::info;

const MIGRATION_001: &str = include_str!("migrations/001_initial.sql");
const MIGRATION_002: &str = include_str!("migrations/002_schema_fixes.sql");
const MIGRATION_003: &str = include_str!("migrations/003_lamport_sync_cursor.sql");
const MIGRATION_004: &str = include_str!("migrations/004_messages_nonce.sql");

/// Database wrapper for SQLite connection management
pub struct Database {
    conn: Arc<Mutex<Connection>>,
    path: PathBuf,
}

impl Database {
    /// Create a new database connection at the given path
    pub fn new(path: PathBuf) -> SqliteResult<Self> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| {
                rusqlite::Error::InvalidPath(path.clone().into())
            })?;
        }

        let conn = Connection::open(&path)?;

        // Enable foreign keys
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
            path,
        };

        // Run migrations
        db.migrate()?;

        info!("Database initialized at {:?}", db.path);
        Ok(db)
    }

    /// Create an in-memory database (for testing)
    pub fn in_memory() -> SqliteResult<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
            path: PathBuf::from(":memory:"),
        };

        db.migrate()?;
        Ok(db)
    }

    /// Run database migrations
    fn migrate(&self) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();

        // Check current schema version
        let version: i32 = conn
            .query_row(
                "SELECT version FROM schema_version WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if version < 1 {
            info!("Running migration 001...");
            conn.execute_batch(MIGRATION_001)?;
            info!("Migration 001 complete");
        }

        if version < 2 {
            info!("Running migration 002...");
            conn.execute_batch(MIGRATION_002)?;
            info!("Migration 002 complete");
        }

        if version < 3 {
            info!("Running migration 003...");
            conn.execute_batch(MIGRATION_003)?;
            info!("Migration 003 complete");
        }

        if version < 4 {
            info!("Running migration 004...");
            conn.execute_batch(MIGRATION_004)?;
            info!("Migration 004 complete");
        }

        Ok(())
    }

    /// Execute a function with the database connection
    pub fn with_connection<F, T>(&self, f: F) -> SqliteResult<T>
    where
        F: FnOnce(&Connection) -> SqliteResult<T>,
    {
        let conn = self.conn.lock().unwrap();
        f(&conn)
    }

    /// Execute a function with a mutable database connection (for transactions)
    pub fn with_connection_mut<F, T>(&self, f: F) -> SqliteResult<T>
    where
        F: FnOnce(&mut Connection) -> SqliteResult<T>,
    {
        let mut conn = self.conn.lock().unwrap();
        f(&mut conn)
    }

    /// Get the database path
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Get the next lamport clock value for the given author and increment it
    pub fn next_lamport_clock(&self, author_peer_id: &str) -> SqliteResult<i64> {
        self.with_connection_mut(|conn| {
            let tx = conn.transaction()?;

            // Get current value (or 0 if not exists)
            let current: i64 = tx.query_row(
                "SELECT current_value FROM lamport_clocks WHERE author_peer_id = ?",
                [author_peer_id],
                |row| row.get(0),
            ).unwrap_or(0);

            let next = current + 1;

            // Upsert the new value
            tx.execute(
                "INSERT INTO lamport_clocks (author_peer_id, current_value) VALUES (?, ?)
                 ON CONFLICT(author_peer_id) DO UPDATE SET current_value = excluded.current_value",
                rusqlite::params![author_peer_id, next],
            )?;

            tx.commit()?;
            Ok(next)
        })
    }

    /// Update lamport clock for author if received value is higher
    pub fn update_lamport_clock(&self, author_peer_id: &str, received: i64) -> SqliteResult<()> {
        self.with_connection(|conn| {
            conn.execute(
                "INSERT INTO lamport_clocks (author_peer_id, current_value) VALUES (?, ?)
                 ON CONFLICT(author_peer_id) DO UPDATE SET current_value = MAX(current_value, excluded.current_value)",
                rusqlite::params![author_peer_id, received],
            )?;
            Ok(())
        })
    }

    /// Get the current lamport clock value for an author (without incrementing)
    pub fn get_lamport_clock(&self, author_peer_id: &str) -> SqliteResult<i64> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT current_value FROM lamport_clocks WHERE author_peer_id = ?",
                [author_peer_id],
                |row| row.get(0),
            ).or(Ok(0))
        })
    }

    /// Get and increment the send counter for a conversation (for nonce generation)
    pub fn next_send_counter(&self, conversation_id: &str) -> SqliteResult<u64> {
        self.with_connection_mut(|conn| {
            let tx = conn.transaction()?;

            // Get current value (or 0 if not exists)
            let current: u64 = tx.query_row(
                "SELECT send_counter FROM conversation_counters WHERE conversation_id = ?",
                [conversation_id],
                |row| row.get(0),
            ).unwrap_or(0);

            let next = current + 1;

            // Upsert the new value
            tx.execute(
                "INSERT INTO conversation_counters (conversation_id, send_counter, highest_received_counter) VALUES (?, ?, 0)
                 ON CONFLICT(conversation_id) DO UPDATE SET send_counter = excluded.send_counter",
                rusqlite::params![conversation_id, next],
            )?;

            tx.commit()?;
            Ok(next)
        })
    }

    /// Check if a nonce has been seen and record it if not
    /// Returns true if the nonce is new (not replayed), false if it's a replay
    pub fn check_and_record_nonce(
        &self,
        conversation_id: &str,
        sender_peer_id: &str,
        nonce_counter: u64,
    ) -> SqliteResult<bool> {
        self.with_connection_mut(|conn| {
            let tx = conn.transaction()?;

            // Try to insert the nonce
            let result = tx.execute(
                "INSERT INTO received_nonces (conversation_id, sender_peer_id, nonce_counter, received_at)
                 VALUES (?, ?, ?, ?)",
                rusqlite::params![
                    conversation_id,
                    sender_peer_id,
                    nonce_counter as i64,
                    chrono::Utc::now().timestamp()
                ],
            );

            match result {
                Ok(_) => {
                    // Update highest received counter
                    tx.execute(
                        "UPDATE conversation_counters
                         SET highest_received_counter = MAX(highest_received_counter, ?)
                         WHERE conversation_id = ?",
                        rusqlite::params![nonce_counter as i64, conversation_id],
                    )?;
                    tx.commit()?;
                    Ok(true) // New nonce, not a replay
                }
                Err(rusqlite::Error::SqliteFailure(err, _))
                    if err.code == rusqlite::ErrorCode::ConstraintViolation =>
                {
                    // Unique constraint violated - this nonce was already seen
                    Ok(false) // Replay detected
                }
                Err(e) => Err(e),
            }
        })
    }

    // ============================================================
    // Sync Cursor Functions (lamport-based)
    // ============================================================

    /// Get the sync cursor for a specific peer and sync type
    /// Returns a map of author_peer_id -> highest_lamport_clock
    pub fn get_sync_cursor(
        &self,
        source_peer_id: &str,
        sync_type: &str,
    ) -> SqliteResult<std::collections::HashMap<String, u64>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT author_peer_id, highest_lamport_clock FROM sync_cursors
                 WHERE source_peer_id = ? AND sync_type = ?"
            )?;

            let rows = stmt.query_map(
                rusqlite::params![source_peer_id, sync_type],
                |row| {
                    let author: String = row.get(0)?;
                    let clock: i64 = row.get(1)?;
                    Ok((author, clock as u64))
                },
            )?;

            let mut cursor = std::collections::HashMap::new();
            for row in rows {
                let (author, clock) = row?;
                cursor.insert(author, clock);
            }
            Ok(cursor)
        })
    }

    /// Update the sync cursor for a specific author
    /// Call this after successfully syncing content from an author
    pub fn update_sync_cursor(
        &self,
        source_peer_id: &str,
        sync_type: &str,
        author_peer_id: &str,
        lamport_clock: u64,
    ) -> SqliteResult<()> {
        self.with_connection(|conn| {
            conn.execute(
                "INSERT INTO sync_cursors (source_peer_id, sync_type, author_peer_id, highest_lamport_clock, last_sync_at)
                 VALUES (?, ?, ?, ?, ?)
                 ON CONFLICT(source_peer_id, sync_type, author_peer_id)
                 DO UPDATE SET
                     highest_lamport_clock = MAX(highest_lamport_clock, excluded.highest_lamport_clock),
                     last_sync_at = excluded.last_sync_at",
                rusqlite::params![
                    source_peer_id,
                    sync_type,
                    author_peer_id,
                    lamport_clock as i64,
                    chrono::Utc::now().timestamp()
                ],
            )?;
            Ok(())
        })
    }

    /// Batch update sync cursors from a response
    pub fn update_sync_cursors_batch(
        &self,
        source_peer_id: &str,
        sync_type: &str,
        cursor_updates: &std::collections::HashMap<String, u64>,
    ) -> SqliteResult<()> {
        self.with_connection_mut(|conn| {
            let tx = conn.transaction()?;
            let now = chrono::Utc::now().timestamp();

            for (author_peer_id, lamport_clock) in cursor_updates {
                tx.execute(
                    "INSERT INTO sync_cursors (source_peer_id, sync_type, author_peer_id, highest_lamport_clock, last_sync_at)
                     VALUES (?, ?, ?, ?, ?)
                     ON CONFLICT(source_peer_id, sync_type, author_peer_id)
                     DO UPDATE SET
                         highest_lamport_clock = MAX(highest_lamport_clock, excluded.highest_lamport_clock),
                         last_sync_at = excluded.last_sync_at",
                    rusqlite::params![
                        source_peer_id,
                        sync_type,
                        author_peer_id,
                        *lamport_clock as i64,
                        now
                    ],
                )?;
            }

            tx.commit()?;
            Ok(())
        })
    }

    /// Get last sync time for a peer
    pub fn get_last_sync_time(
        &self,
        source_peer_id: &str,
        sync_type: &str,
    ) -> SqliteResult<Option<i64>> {
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT MAX(last_sync_at) FROM sync_cursors
                 WHERE source_peer_id = ? AND sync_type = ?",
                rusqlite::params![source_peer_id, sync_type],
                |row| row.get(0),
            ).or(Ok(None))
        })
    }
}

impl Clone for Database {
    fn clone(&self) -> Self {
        Self {
            conn: Arc::clone(&self.conn),
            path: self.path.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_creation() {
        let db = Database::in_memory().unwrap();

        // Verify tables exist
        db.with_connection(|conn| {
            let count: i32 = conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
                [],
                |row| row.get(0),
            )?;
            assert!(count > 0, "Tables should be created");
            Ok(())
        }).unwrap();
    }

    #[test]
    fn test_lamport_clock_per_author() {
        let db = Database::in_memory().unwrap();
        let author1 = "12D3KooWAuthor1";
        let author2 = "12D3KooWAuthor2";

        // Each author starts at 0 and increments independently
        let clock1_a = db.next_lamport_clock(author1).unwrap();
        let clock1_b = db.next_lamport_clock(author1).unwrap();
        let clock2_a = db.next_lamport_clock(author2).unwrap();
        let clock1_c = db.next_lamport_clock(author1).unwrap();

        assert_eq!(clock1_a, 1);
        assert_eq!(clock1_b, 2);
        assert_eq!(clock2_a, 1); // Author 2 starts at 1
        assert_eq!(clock1_c, 3);
    }

    #[test]
    fn test_lamport_clock_update() {
        let db = Database::in_memory().unwrap();
        let author = "12D3KooWAuthor1";

        // Get initial value
        let _ = db.next_lamport_clock(author).unwrap(); // 1

        // Update with higher value from network
        db.update_lamport_clock(author, 100).unwrap();

        // Next should be 101
        let next = db.next_lamport_clock(author).unwrap();
        assert_eq!(next, 101);
    }

    #[test]
    fn test_send_counter() {
        let db = Database::in_memory().unwrap();
        let conv_id = "conversation123";

        let counter1 = db.next_send_counter(conv_id).unwrap();
        let counter2 = db.next_send_counter(conv_id).unwrap();
        let counter3 = db.next_send_counter(conv_id).unwrap();

        assert_eq!(counter1, 1);
        assert_eq!(counter2, 2);
        assert_eq!(counter3, 3);
    }

    #[test]
    fn test_nonce_replay_detection() {
        let db = Database::in_memory().unwrap();
        let conv_id = "conversation123";
        let sender = "12D3KooWSender";

        // First time seeing nonce 1 - should be accepted
        assert!(db.check_and_record_nonce(conv_id, sender, 1).unwrap());

        // Second time seeing nonce 1 - should be rejected (replay)
        assert!(!db.check_and_record_nonce(conv_id, sender, 1).unwrap());

        // Different nonce - should be accepted
        assert!(db.check_and_record_nonce(conv_id, sender, 2).unwrap());

        // Same nonce from different sender - should be accepted
        assert!(db.check_and_record_nonce(conv_id, "12D3KooWOther", 1).unwrap());
    }

    #[test]
    fn test_sync_cursor_empty() {
        let db = Database::in_memory().unwrap();

        // Initially no cursor exists
        let cursor = db.get_sync_cursor("12D3KooWPeer1", "posts").unwrap();
        assert!(cursor.is_empty());
    }

    #[test]
    fn test_sync_cursor_update_and_get() {
        let db = Database::in_memory().unwrap();
        let source = "12D3KooWPeer1";
        let author1 = "12D3KooWAuthor1";
        let author2 = "12D3KooWAuthor2";

        // Update cursor for author1
        db.update_sync_cursor(source, "posts", author1, 10).unwrap();

        // Update cursor for author2
        db.update_sync_cursor(source, "posts", author2, 5).unwrap();

        // Get cursor should return both
        let cursor = db.get_sync_cursor(source, "posts").unwrap();
        assert_eq!(cursor.len(), 2);
        assert_eq!(cursor.get(author1), Some(&10));
        assert_eq!(cursor.get(author2), Some(&5));
    }

    #[test]
    fn test_sync_cursor_only_increases() {
        let db = Database::in_memory().unwrap();
        let source = "12D3KooWPeer1";
        let author = "12D3KooWAuthor1";

        // Update to 10
        db.update_sync_cursor(source, "posts", author, 10).unwrap();

        // Try to "update" to 5 (lower) - should be ignored
        db.update_sync_cursor(source, "posts", author, 5).unwrap();

        // Cursor should still be 10
        let cursor = db.get_sync_cursor(source, "posts").unwrap();
        assert_eq!(cursor.get(author), Some(&10));

        // Update to 15 (higher) - should work
        db.update_sync_cursor(source, "posts", author, 15).unwrap();
        let cursor = db.get_sync_cursor(source, "posts").unwrap();
        assert_eq!(cursor.get(author), Some(&15));
    }

    #[test]
    fn test_sync_cursor_different_sync_types() {
        let db = Database::in_memory().unwrap();
        let source = "12D3KooWPeer1";
        let author = "12D3KooWAuthor1";

        // Update posts cursor
        db.update_sync_cursor(source, "posts", author, 10).unwrap();

        // Update permissions cursor (different type)
        db.update_sync_cursor(source, "permissions", author, 5).unwrap();

        // They should be separate
        let posts_cursor = db.get_sync_cursor(source, "posts").unwrap();
        let perms_cursor = db.get_sync_cursor(source, "permissions").unwrap();

        assert_eq!(posts_cursor.get(author), Some(&10));
        assert_eq!(perms_cursor.get(author), Some(&5));
    }

    #[test]
    fn test_sync_cursor_batch_update() {
        use std::collections::HashMap;

        let db = Database::in_memory().unwrap();
        let source = "12D3KooWPeer1";

        let mut updates = HashMap::new();
        updates.insert("12D3KooWAuthor1".to_string(), 10u64);
        updates.insert("12D3KooWAuthor2".to_string(), 20u64);
        updates.insert("12D3KooWAuthor3".to_string(), 30u64);

        db.update_sync_cursors_batch(source, "posts", &updates).unwrap();

        let cursor = db.get_sync_cursor(source, "posts").unwrap();
        assert_eq!(cursor.len(), 3);
        assert_eq!(cursor.get("12D3KooWAuthor1"), Some(&10));
        assert_eq!(cursor.get("12D3KooWAuthor2"), Some(&20));
        assert_eq!(cursor.get("12D3KooWAuthor3"), Some(&30));
    }
}
