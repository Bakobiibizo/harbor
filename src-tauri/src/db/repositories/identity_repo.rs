use crate::db::Database;
use crate::models::identity::LocalIdentity;
use rusqlite::{params, Result as SqliteResult};
use tracing::info;

/// Repository for local identity operations
pub struct IdentityRepository<'a> {
    db: &'a Database,
}

impl<'a> IdentityRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Check if a local identity exists
    pub fn exists(&self) -> SqliteResult<bool> {
        self.db.with_connection(|conn| {
            let count: i32 = conn.query_row(
                "SELECT COUNT(*) FROM local_identity WHERE id = 1",
                [],
                |row| row.get(0),
            )?;
            Ok(count > 0)
        })
    }

    /// Get the local identity
    pub fn get(&self) -> SqliteResult<Option<LocalIdentity>> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT peer_id, public_key, x25519_public, private_key_encrypted,
                        display_name, avatar_hash, bio, passphrase_hint, created_at, updated_at
                 FROM local_identity WHERE id = 1",
            )?;

            let result = stmt.query_row([], |row| {
                Ok(LocalIdentity {
                    peer_id: row.get(0)?,
                    public_key: row.get(1)?,
                    x25519_public: row.get(2)?,
                    private_key_encrypted: row.get(3)?,
                    display_name: row.get(4)?,
                    avatar_hash: row.get(5)?,
                    bio: row.get(6)?,
                    passphrase_hint: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            });

            match result {
                Ok(identity) => Ok(Some(identity)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }

    /// Create a new local identity
    pub fn create(&self, identity: &LocalIdentity) -> SqliteResult<()> {
        self.db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO local_identity
                 (id, peer_id, public_key, x25519_public, private_key_encrypted,
                  display_name, avatar_hash, bio, passphrase_hint, created_at, updated_at)
                 VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    identity.peer_id,
                    identity.public_key,
                    identity.x25519_public,
                    identity.private_key_encrypted,
                    identity.display_name,
                    identity.avatar_hash,
                    identity.bio,
                    identity.passphrase_hint,
                    identity.created_at,
                    identity.updated_at,
                ],
            )?;
            info!("Created local identity: {}", identity.peer_id);
            Ok(())
        })
    }

    /// Update display name
    pub fn update_display_name(&self, display_name: &str) -> SqliteResult<()> {
        let now = chrono::Utc::now().timestamp();
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE local_identity SET display_name = ?1, updated_at = ?2 WHERE id = 1",
                params![display_name, now],
            )?;
            Ok(())
        })
    }

    /// Update bio
    pub fn update_bio(&self, bio: Option<&str>) -> SqliteResult<()> {
        let now = chrono::Utc::now().timestamp();
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE local_identity SET bio = ?1, updated_at = ?2 WHERE id = 1",
                params![bio, now],
            )?;
            Ok(())
        })
    }

    /// Update avatar hash
    pub fn update_avatar(&self, avatar_hash: Option<&str>) -> SqliteResult<()> {
        let now = chrono::Utc::now().timestamp();
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE local_identity SET avatar_hash = ?1, updated_at = ?2 WHERE id = 1",
                params![avatar_hash, now],
            )?;
            Ok(())
        })
    }

    /// Update passphrase hint
    pub fn update_passphrase_hint(&self, hint: Option<&str>) -> SqliteResult<()> {
        let now = chrono::Utc::now().timestamp();
        self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE local_identity SET passphrase_hint = ?1, updated_at = ?2 WHERE id = 1",
                params![hint, now],
            )?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_identity() -> LocalIdentity {
        LocalIdentity {
            peer_id: "12D3KooWTestPeerId".to_string(),
            public_key: vec![1, 2, 3, 4],
            x25519_public: vec![5, 6, 7, 8],
            private_key_encrypted: vec![9, 10, 11, 12],
            display_name: "Test User".to_string(),
            avatar_hash: None,
            bio: Some("Test bio".to_string()),
            passphrase_hint: Some("My hint".to_string()),
            created_at: 1000,
            updated_at: 1000,
        }
    }

    #[test]
    fn test_identity_crud() {
        let db = Database::in_memory().unwrap();
        let repo = IdentityRepository::new(&db);

        // Initially no identity
        assert!(!repo.exists().unwrap());
        assert!(repo.get().unwrap().is_none());

        // Create identity
        let identity = create_test_identity();
        repo.create(&identity).unwrap();

        // Now exists
        assert!(repo.exists().unwrap());

        // Can retrieve
        let retrieved = repo.get().unwrap().unwrap();
        assert_eq!(retrieved.peer_id, identity.peer_id);
        assert_eq!(retrieved.display_name, identity.display_name);
    }

    #[test]
    fn test_update_display_name() {
        let db = Database::in_memory().unwrap();
        let repo = IdentityRepository::new(&db);

        repo.create(&create_test_identity()).unwrap();
        repo.update_display_name("New Name").unwrap();

        let identity = repo.get().unwrap().unwrap();
        assert_eq!(identity.display_name, "New Name");
    }
}
