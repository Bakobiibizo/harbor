//! Contact repository for managing peer contacts

use crate::db::Database;
use rusqlite::{params, OptionalExtension, Result as SqliteResult};

/// Represents a contact in the database
#[derive(Debug, Clone)]
pub struct Contact {
    pub id: i64,
    pub peer_id: String,
    pub public_key: Vec<u8>,
    pub x25519_public: Vec<u8>,
    pub display_name: String,
    pub avatar_hash: Option<String>,
    pub bio: Option<String>,
    pub is_blocked: bool,
    pub trust_level: i32,
    pub last_seen_at: Option<i64>,
    pub added_at: i64,
    pub updated_at: i64,
}

/// Contact data for creating or updating contacts
#[derive(Debug, Clone)]
pub struct ContactData {
    pub peer_id: String,
    pub public_key: Vec<u8>,
    pub x25519_public: Vec<u8>,
    pub display_name: String,
    pub avatar_hash: Option<String>,
    pub bio: Option<String>,
}

/// Repository for contact operations
pub struct ContactsRepository;

impl ContactsRepository {
    /// Add a new contact
    pub fn add_contact(db: &Database, contact: &ContactData) -> SqliteResult<i64> {
        db.with_connection(|conn| {
            let now = chrono::Utc::now().timestamp();
            conn.execute(
                "INSERT INTO contacts (peer_id, public_key, x25519_public, display_name, avatar_hash, bio, added_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    contact.peer_id,
                    contact.public_key,
                    contact.x25519_public,
                    contact.display_name,
                    contact.avatar_hash,
                    contact.bio,
                    now,
                    now
                ],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    /// Get a contact by peer ID
    pub fn get_by_peer_id(db: &Database, peer_id: &str) -> SqliteResult<Option<Contact>> {
        db.with_connection(|conn| {
            conn.query_row(
                "SELECT id, peer_id, public_key, x25519_public, display_name, avatar_hash, bio,
                        is_blocked, trust_level, last_seen_at, added_at, updated_at
                 FROM contacts WHERE peer_id = ?",
                [peer_id],
                |row| {
                    Ok(Contact {
                        id: row.get(0)?,
                        peer_id: row.get(1)?,
                        public_key: row.get(2)?,
                        x25519_public: row.get(3)?,
                        display_name: row.get(4)?,
                        avatar_hash: row.get(5)?,
                        bio: row.get(6)?,
                        is_blocked: row.get::<_, i32>(7)? != 0,
                        trust_level: row.get(8)?,
                        last_seen_at: row.get(9)?,
                        added_at: row.get(10)?,
                        updated_at: row.get(11)?,
                    })
                },
            )
            .optional()
        })
    }

    /// Get all contacts
    pub fn get_all(db: &Database) -> SqliteResult<Vec<Contact>> {
        db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, peer_id, public_key, x25519_public, display_name, avatar_hash, bio,
                        is_blocked, trust_level, last_seen_at, added_at, updated_at
                 FROM contacts
                 ORDER BY display_name ASC",
            )?;

            let contacts = stmt.query_map([], |row| {
                Ok(Contact {
                    id: row.get(0)?,
                    peer_id: row.get(1)?,
                    public_key: row.get(2)?,
                    x25519_public: row.get(3)?,
                    display_name: row.get(4)?,
                    avatar_hash: row.get(5)?,
                    bio: row.get(6)?,
                    is_blocked: row.get::<_, i32>(7)? != 0,
                    trust_level: row.get(8)?,
                    last_seen_at: row.get(9)?,
                    added_at: row.get(10)?,
                    updated_at: row.get(11)?,
                })
            })?;

            contacts.collect()
        })
    }

    /// Get all non-blocked contacts
    pub fn get_active(db: &Database) -> SqliteResult<Vec<Contact>> {
        db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, peer_id, public_key, x25519_public, display_name, avatar_hash, bio,
                        is_blocked, trust_level, last_seen_at, added_at, updated_at
                 FROM contacts
                 WHERE is_blocked = 0
                 ORDER BY display_name ASC",
            )?;

            let contacts = stmt.query_map([], |row| {
                Ok(Contact {
                    id: row.get(0)?,
                    peer_id: row.get(1)?,
                    public_key: row.get(2)?,
                    x25519_public: row.get(3)?,
                    display_name: row.get(4)?,
                    avatar_hash: row.get(5)?,
                    bio: row.get(6)?,
                    is_blocked: row.get::<_, i32>(7)? != 0,
                    trust_level: row.get(8)?,
                    last_seen_at: row.get(9)?,
                    added_at: row.get(10)?,
                    updated_at: row.get(11)?,
                })
            })?;

            contacts.collect()
        })
    }

    /// Update contact info (from identity exchange)
    pub fn update_contact_info(
        db: &Database,
        peer_id: &str,
        display_name: &str,
        avatar_hash: Option<&str>,
        bio: Option<&str>,
    ) -> SqliteResult<bool> {
        db.with_connection(|conn| {
            let now = chrono::Utc::now().timestamp();
            let rows = conn.execute(
                "UPDATE contacts SET display_name = ?, avatar_hash = ?, bio = ?, updated_at = ?
                 WHERE peer_id = ?",
                params![display_name, avatar_hash, bio, now, peer_id],
            )?;
            Ok(rows > 0)
        })
    }

    /// Update last seen timestamp
    pub fn update_last_seen(db: &Database, peer_id: &str) -> SqliteResult<bool> {
        db.with_connection(|conn| {
            let now = chrono::Utc::now().timestamp();
            let rows = conn.execute(
                "UPDATE contacts SET last_seen_at = ?, updated_at = ? WHERE peer_id = ?",
                params![now, now, peer_id],
            )?;
            Ok(rows > 0)
        })
    }

    /// Block a contact
    pub fn block_contact(db: &Database, peer_id: &str) -> SqliteResult<bool> {
        db.with_connection(|conn| {
            let now = chrono::Utc::now().timestamp();
            let rows = conn.execute(
                "UPDATE contacts SET is_blocked = 1, updated_at = ? WHERE peer_id = ?",
                params![now, peer_id],
            )?;
            Ok(rows > 0)
        })
    }

    /// Unblock a contact
    pub fn unblock_contact(db: &Database, peer_id: &str) -> SqliteResult<bool> {
        db.with_connection(|conn| {
            let now = chrono::Utc::now().timestamp();
            let rows = conn.execute(
                "UPDATE contacts SET is_blocked = 0, updated_at = ? WHERE peer_id = ?",
                params![now, peer_id],
            )?;
            Ok(rows > 0)
        })
    }

    /// Update trust level
    pub fn set_trust_level(db: &Database, peer_id: &str, trust_level: i32) -> SqliteResult<bool> {
        db.with_connection(|conn| {
            let now = chrono::Utc::now().timestamp();
            let rows = conn.execute(
                "UPDATE contacts SET trust_level = ?, updated_at = ? WHERE peer_id = ?",
                params![trust_level, now, peer_id],
            )?;
            Ok(rows > 0)
        })
    }

    /// Remove a contact
    pub fn remove_contact(db: &Database, peer_id: &str) -> SqliteResult<bool> {
        db.with_connection(|conn| {
            let rows = conn.execute("DELETE FROM contacts WHERE peer_id = ?", [peer_id])?;
            Ok(rows > 0)
        })
    }

    /// Check if peer is a contact
    pub fn is_contact(db: &Database, peer_id: &str) -> SqliteResult<bool> {
        db.with_connection(|conn| {
            let count: i32 = conn.query_row(
                "SELECT COUNT(*) FROM contacts WHERE peer_id = ?",
                [peer_id],
                |row| row.get(0),
            )?;
            Ok(count > 0)
        })
    }

    /// Check if peer is blocked
    pub fn is_blocked(db: &Database, peer_id: &str) -> SqliteResult<bool> {
        db.with_connection(|conn| {
            let blocked: Option<i32> = conn
                .query_row(
                    "SELECT is_blocked FROM contacts WHERE peer_id = ?",
                    [peer_id],
                    |row| row.get(0),
                )
                .optional()?;
            Ok(blocked.unwrap_or(0) != 0)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_get_contact() {
        let db = Database::in_memory().unwrap();

        let contact_data = ContactData {
            peer_id: "12D3KooWTest".to_string(),
            public_key: vec![1, 2, 3, 4],
            x25519_public: vec![5, 6, 7, 8],
            display_name: "Test User".to_string(),
            avatar_hash: None,
            bio: Some("Hello!".to_string()),
        };

        let id = ContactsRepository::add_contact(&db, &contact_data).unwrap();
        assert!(id > 0);

        let contact = ContactsRepository::get_by_peer_id(&db, "12D3KooWTest")
            .unwrap()
            .expect("Contact should exist");

        assert_eq!(contact.peer_id, "12D3KooWTest");
        assert_eq!(contact.display_name, "Test User");
        assert_eq!(contact.bio, Some("Hello!".to_string()));
        assert!(!contact.is_blocked);
    }

    #[test]
    fn test_block_unblock_contact() {
        let db = Database::in_memory().unwrap();

        let contact_data = ContactData {
            peer_id: "12D3KooWTest".to_string(),
            public_key: vec![1, 2, 3, 4],
            x25519_public: vec![5, 6, 7, 8],
            display_name: "Test User".to_string(),
            avatar_hash: None,
            bio: None,
        };

        ContactsRepository::add_contact(&db, &contact_data).unwrap();

        // Initially not blocked
        assert!(!ContactsRepository::is_blocked(&db, "12D3KooWTest").unwrap());

        // Block
        ContactsRepository::block_contact(&db, "12D3KooWTest").unwrap();
        assert!(ContactsRepository::is_blocked(&db, "12D3KooWTest").unwrap());

        // Unblock
        ContactsRepository::unblock_contact(&db, "12D3KooWTest").unwrap();
        assert!(!ContactsRepository::is_blocked(&db, "12D3KooWTest").unwrap());
    }

    #[test]
    fn test_get_active_contacts() {
        let db = Database::in_memory().unwrap();

        // Add two contacts
        ContactsRepository::add_contact(
            &db,
            &ContactData {
                peer_id: "12D3KooWActive".to_string(),
                public_key: vec![1],
                x25519_public: vec![2],
                display_name: "Active".to_string(),
                avatar_hash: None,
                bio: None,
            },
        )
        .unwrap();

        ContactsRepository::add_contact(
            &db,
            &ContactData {
                peer_id: "12D3KooWBlocked".to_string(),
                public_key: vec![3],
                x25519_public: vec![4],
                display_name: "Blocked".to_string(),
                avatar_hash: None,
                bio: None,
            },
        )
        .unwrap();

        // Block one
        ContactsRepository::block_contact(&db, "12D3KooWBlocked").unwrap();

        // Get active should only return non-blocked
        let active = ContactsRepository::get_active(&db).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].peer_id, "12D3KooWActive");

        // Get all should return both
        let all = ContactsRepository::get_all(&db).unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_remove_contact() {
        let db = Database::in_memory().unwrap();

        ContactsRepository::add_contact(
            &db,
            &ContactData {
                peer_id: "12D3KooWTest".to_string(),
                public_key: vec![1],
                x25519_public: vec![2],
                display_name: "Test".to_string(),
                avatar_hash: None,
                bio: None,
            },
        )
        .unwrap();

        assert!(ContactsRepository::is_contact(&db, "12D3KooWTest").unwrap());

        ContactsRepository::remove_contact(&db, "12D3KooWTest").unwrap();

        assert!(!ContactsRepository::is_contact(&db, "12D3KooWTest").unwrap());
    }
}
