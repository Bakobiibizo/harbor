//! Permissions repository for managing capability grants and revocations

use crate::db::Database;
use rusqlite::{params, OptionalExtension, Result as SqliteResult};

/// Capability types that can be granted
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    /// Can send/receive direct messages
    Chat,
    /// Can view wall posts
    WallRead,
    /// Can initiate voice calls
    Call,
}

impl Capability {
    pub fn as_str(&self) -> &'static str {
        match self {
            Capability::Chat => "chat",
            Capability::WallRead => "wall_read",
            Capability::Call => "call",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "chat" => Some(Capability::Chat),
            "wall_read" => Some(Capability::WallRead),
            "call" => Some(Capability::Call),
            _ => None,
        }
    }
}

/// A permission event (request, grant, or revoke)
#[derive(Debug, Clone)]
pub struct PermissionEvent {
    pub id: i64,
    pub event_id: String,
    pub event_type: String, // "request", "grant", "revoke"
    pub entity_id: String,  // request_id or grant_id
    pub author_peer_id: String,
    pub issuer_peer_id: Option<String>,
    pub subject_peer_id: String,
    pub capability: String,
    pub scope_json: Option<String>,
    pub lamport_clock: i64,
    pub issued_at: Option<i64>,
    pub expires_at: Option<i64>,
    pub payload_cbor: Vec<u8>,
    pub signature: Vec<u8>,
    pub received_at: i64,
}

/// Current permission state (materialized from events)
#[derive(Debug, Clone)]
pub struct Permission {
    pub id: i64,
    pub grant_id: String,
    pub issuer_peer_id: String,
    pub subject_peer_id: String,
    pub capability: String,
    pub issued_at: i64,
    pub expires_at: Option<i64>,
    pub revoked_at: Option<i64>,
    pub payload_cbor: Vec<u8>,
    pub signature: Vec<u8>,
}

impl Permission {
    /// Check if this permission is currently valid
    pub fn is_valid(&self) -> bool {
        // Not revoked
        if self.revoked_at.is_some() {
            return false;
        }

        // Not expired
        if let Some(expires_at) = self.expires_at {
            let now = chrono::Utc::now().timestamp();
            if now > expires_at {
                return false;
            }
        }

        true
    }
}

/// Data for creating a new permission grant
#[derive(Debug, Clone)]
pub struct GrantData {
    pub grant_id: String,
    pub issuer_peer_id: String,
    pub subject_peer_id: String,
    pub capability: String,
    pub scope_json: Option<String>,
    pub lamport_clock: i64,
    pub issued_at: i64,
    pub expires_at: Option<i64>,
    pub payload_cbor: Vec<u8>,
    pub signature: Vec<u8>,
}

/// Repository for permission operations
pub struct PermissionsRepository;

impl PermissionsRepository {
    // ============================================================
    // Event Storage (append-only log)
    // ============================================================

    /// Record a permission event (request, grant, or revoke)
    pub fn record_event(
        db: &Database,
        event_id: &str,
        event_type: &str,
        entity_id: &str,
        author_peer_id: &str,
        issuer_peer_id: Option<&str>,
        subject_peer_id: &str,
        capability: &str,
        scope_json: Option<&str>,
        lamport_clock: i64,
        issued_at: Option<i64>,
        expires_at: Option<i64>,
        payload_cbor: &[u8],
        signature: &[u8],
    ) -> SqliteResult<i64> {
        db.with_connection(|conn| {
            let now = chrono::Utc::now().timestamp();
            conn.execute(
                "INSERT INTO permission_events
                 (event_id, event_type, entity_id, author_peer_id, issuer_peer_id, subject_peer_id,
                  capability, scope_json, lamport_clock, issued_at, expires_at, payload_cbor, signature, received_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    event_id, event_type, entity_id, author_peer_id, issuer_peer_id, subject_peer_id,
                    capability, scope_json, lamport_clock, issued_at, expires_at, payload_cbor, signature, now
                ],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    /// Check if event already exists (for deduplication)
    pub fn event_exists(db: &Database, event_id: &str) -> SqliteResult<bool> {
        db.with_connection(|conn| {
            let count: i32 = conn.query_row(
                "SELECT COUNT(*) FROM permission_events WHERE event_id = ?",
                [event_id],
                |row| row.get(0),
            )?;
            Ok(count > 0)
        })
    }

    // ============================================================
    // Materialized Permission State
    // ============================================================

    /// Create or update a permission grant in the materialized view
    pub fn upsert_grant(db: &Database, grant: &GrantData) -> SqliteResult<()> {
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO permissions_current
                 (grant_id, issuer_peer_id, subject_peer_id, capability, issued_at, expires_at, payload_cbor, signature)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(grant_id) DO UPDATE SET
                     expires_at = excluded.expires_at,
                     payload_cbor = excluded.payload_cbor,
                     signature = excluded.signature",
                params![
                    grant.grant_id,
                    grant.issuer_peer_id,
                    grant.subject_peer_id,
                    grant.capability,
                    grant.issued_at,
                    grant.expires_at,
                    grant.payload_cbor,
                    grant.signature
                ],
            )?;
            Ok(())
        })
    }

    /// Mark a grant as revoked
    pub fn revoke_grant(db: &Database, grant_id: &str, revoked_at: i64) -> SqliteResult<bool> {
        db.with_connection(|conn| {
            let rows = conn.execute(
                "UPDATE permissions_current SET revoked_at = ? WHERE grant_id = ?",
                params![revoked_at, grant_id],
            )?;
            Ok(rows > 0)
        })
    }

    /// Get a permission by grant ID
    pub fn get_by_grant_id(db: &Database, grant_id: &str) -> SqliteResult<Option<Permission>> {
        db.with_connection(|conn| {
            conn.query_row(
                "SELECT id, grant_id, issuer_peer_id, subject_peer_id, capability,
                        issued_at, expires_at, revoked_at, payload_cbor, signature
                 FROM permissions_current WHERE grant_id = ?",
                [grant_id],
                |row| {
                    Ok(Permission {
                        id: row.get(0)?,
                        grant_id: row.get(1)?,
                        issuer_peer_id: row.get(2)?,
                        subject_peer_id: row.get(3)?,
                        capability: row.get(4)?,
                        issued_at: row.get(5)?,
                        expires_at: row.get(6)?,
                        revoked_at: row.get(7)?,
                        payload_cbor: row.get(8)?,
                        signature: row.get(9)?,
                    })
                },
            )
            .optional()
        })
    }

    /// Get all valid permissions granted TO a peer (they are the subject)
    pub fn get_permissions_for_subject(
        db: &Database,
        subject_peer_id: &str,
    ) -> SqliteResult<Vec<Permission>> {
        db.with_connection(|conn| {
            let now = chrono::Utc::now().timestamp();
            let mut stmt = conn.prepare(
                "SELECT id, grant_id, issuer_peer_id, subject_peer_id, capability,
                        issued_at, expires_at, revoked_at, payload_cbor, signature
                 FROM permissions_current
                 WHERE subject_peer_id = ?
                   AND revoked_at IS NULL
                   AND (expires_at IS NULL OR expires_at > ?)",
            )?;

            let perms = stmt.query_map(params![subject_peer_id, now], |row| {
                Ok(Permission {
                    id: row.get(0)?,
                    grant_id: row.get(1)?,
                    issuer_peer_id: row.get(2)?,
                    subject_peer_id: row.get(3)?,
                    capability: row.get(4)?,
                    issued_at: row.get(5)?,
                    expires_at: row.get(6)?,
                    revoked_at: row.get(7)?,
                    payload_cbor: row.get(8)?,
                    signature: row.get(9)?,
                })
            })?;

            perms.collect()
        })
    }

    /// Get all valid permissions granted BY a peer (they are the issuer)
    pub fn get_permissions_by_issuer(
        db: &Database,
        issuer_peer_id: &str,
    ) -> SqliteResult<Vec<Permission>> {
        db.with_connection(|conn| {
            let now = chrono::Utc::now().timestamp();
            let mut stmt = conn.prepare(
                "SELECT id, grant_id, issuer_peer_id, subject_peer_id, capability,
                        issued_at, expires_at, revoked_at, payload_cbor, signature
                 FROM permissions_current
                 WHERE issuer_peer_id = ?
                   AND revoked_at IS NULL
                   AND (expires_at IS NULL OR expires_at > ?)",
            )?;

            let perms = stmt.query_map(params![issuer_peer_id, now], |row| {
                Ok(Permission {
                    id: row.get(0)?,
                    grant_id: row.get(1)?,
                    issuer_peer_id: row.get(2)?,
                    subject_peer_id: row.get(3)?,
                    capability: row.get(4)?,
                    issued_at: row.get(5)?,
                    expires_at: row.get(6)?,
                    revoked_at: row.get(7)?,
                    payload_cbor: row.get(8)?,
                    signature: row.get(9)?,
                })
            })?;

            perms.collect()
        })
    }

    /// Check if a peer has a specific capability granted by another peer
    pub fn has_capability(
        db: &Database,
        issuer_peer_id: &str,
        subject_peer_id: &str,
        capability: &str,
    ) -> SqliteResult<bool> {
        db.with_connection(|conn| {
            let now = chrono::Utc::now().timestamp();
            let count: i32 = conn.query_row(
                "SELECT COUNT(*) FROM permissions_current
                 WHERE issuer_peer_id = ?
                   AND subject_peer_id = ?
                   AND capability = ?
                   AND revoked_at IS NULL
                   AND (expires_at IS NULL OR expires_at > ?)",
                params![issuer_peer_id, subject_peer_id, capability, now],
                |row| row.get(0),
            )?;
            Ok(count > 0)
        })
    }

    /// Get the grant for a specific capability (if it exists and is valid)
    pub fn get_capability_grant(
        db: &Database,
        issuer_peer_id: &str,
        subject_peer_id: &str,
        capability: &str,
    ) -> SqliteResult<Option<Permission>> {
        db.with_connection(|conn| {
            let now = chrono::Utc::now().timestamp();
            conn.query_row(
                "SELECT id, grant_id, issuer_peer_id, subject_peer_id, capability,
                        issued_at, expires_at, revoked_at, payload_cbor, signature
                 FROM permissions_current
                 WHERE issuer_peer_id = ?
                   AND subject_peer_id = ?
                   AND capability = ?
                   AND revoked_at IS NULL
                   AND (expires_at IS NULL OR expires_at > ?)
                 ORDER BY issued_at DESC
                 LIMIT 1",
                params![issuer_peer_id, subject_peer_id, capability, now],
                |row| {
                    Ok(Permission {
                        id: row.get(0)?,
                        grant_id: row.get(1)?,
                        issuer_peer_id: row.get(2)?,
                        subject_peer_id: row.get(3)?,
                        capability: row.get(4)?,
                        issued_at: row.get(5)?,
                        expires_at: row.get(6)?,
                        revoked_at: row.get(7)?,
                        payload_cbor: row.get(8)?,
                        signature: row.get(9)?,
                    })
                },
            )
            .optional()
        })
    }

    /// Get all peers who can chat with us (we granted them chat capability)
    pub fn get_chat_contacts(db: &Database, our_peer_id: &str) -> SqliteResult<Vec<String>> {
        db.with_connection(|conn| {
            let now = chrono::Utc::now().timestamp();
            let mut stmt = conn.prepare(
                "SELECT DISTINCT subject_peer_id FROM permissions_current
                 WHERE issuer_peer_id = ?
                   AND capability = 'chat'
                   AND revoked_at IS NULL
                   AND (expires_at IS NULL OR expires_at > ?)",
            )?;

            let peers = stmt.query_map(params![our_peer_id, now], |row| row.get(0))?;
            peers.collect()
        })
    }

    /// Get all peers who have granted us a capability
    pub fn get_peers_who_granted_capability(
        db: &Database,
        our_peer_id: &str,
        capability: &str,
    ) -> SqliteResult<Vec<String>> {
        db.with_connection(|conn| {
            let now = chrono::Utc::now().timestamp();
            let mut stmt = conn.prepare(
                "SELECT DISTINCT issuer_peer_id FROM permissions_current
                 WHERE subject_peer_id = ?
                   AND capability = ?
                   AND revoked_at IS NULL
                   AND (expires_at IS NULL OR expires_at > ?)",
            )?;

            let peers = stmt.query_map(params![our_peer_id, capability, now], |row| row.get(0))?;
            peers.collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_conversion() {
        assert_eq!(Capability::Chat.as_str(), "chat");
        assert_eq!(Capability::from_str("chat"), Some(Capability::Chat));
        assert_eq!(Capability::from_str("invalid"), None);
    }

    #[test]
    fn test_grant_and_check_capability() {
        let db = Database::in_memory().unwrap();

        let grant = GrantData {
            grant_id: "grant-123".to_string(),
            issuer_peer_id: "12D3KooWIssuer".to_string(),
            subject_peer_id: "12D3KooWSubject".to_string(),
            capability: "chat".to_string(),
            scope_json: None,
            lamport_clock: 1,
            issued_at: chrono::Utc::now().timestamp(),
            expires_at: None,
            payload_cbor: vec![1, 2, 3],
            signature: vec![4, 5, 6],
        };

        PermissionsRepository::upsert_grant(&db, &grant).unwrap();

        // Check capability exists
        assert!(PermissionsRepository::has_capability(
            &db,
            "12D3KooWIssuer",
            "12D3KooWSubject",
            "chat"
        )
        .unwrap());

        // Check different capability doesn't exist
        assert!(!PermissionsRepository::has_capability(
            &db,
            "12D3KooWIssuer",
            "12D3KooWSubject",
            "call"
        )
        .unwrap());
    }

    #[test]
    fn test_revoke_grant() {
        let db = Database::in_memory().unwrap();

        let grant = GrantData {
            grant_id: "grant-123".to_string(),
            issuer_peer_id: "12D3KooWIssuer".to_string(),
            subject_peer_id: "12D3KooWSubject".to_string(),
            capability: "chat".to_string(),
            scope_json: None,
            lamport_clock: 1,
            issued_at: chrono::Utc::now().timestamp(),
            expires_at: None,
            payload_cbor: vec![1, 2, 3],
            signature: vec![4, 5, 6],
        };

        PermissionsRepository::upsert_grant(&db, &grant).unwrap();

        // Capability exists before revocation
        assert!(PermissionsRepository::has_capability(
            &db,
            "12D3KooWIssuer",
            "12D3KooWSubject",
            "chat"
        )
        .unwrap());

        // Revoke
        let now = chrono::Utc::now().timestamp();
        PermissionsRepository::revoke_grant(&db, "grant-123", now).unwrap();

        // Capability no longer valid
        assert!(!PermissionsRepository::has_capability(
            &db,
            "12D3KooWIssuer",
            "12D3KooWSubject",
            "chat"
        )
        .unwrap());
    }

    #[test]
    fn test_expired_permission() {
        let db = Database::in_memory().unwrap();

        // Create a grant that expired in the past
        let grant = GrantData {
            grant_id: "grant-expired".to_string(),
            issuer_peer_id: "12D3KooWIssuer".to_string(),
            subject_peer_id: "12D3KooWSubject".to_string(),
            capability: "chat".to_string(),
            scope_json: None,
            lamport_clock: 1,
            issued_at: chrono::Utc::now().timestamp() - 3600, // 1 hour ago
            expires_at: Some(chrono::Utc::now().timestamp() - 1800), // 30 min ago
            payload_cbor: vec![1, 2, 3],
            signature: vec![4, 5, 6],
        };

        PermissionsRepository::upsert_grant(&db, &grant).unwrap();

        // Expired permission should not be valid
        assert!(!PermissionsRepository::has_capability(
            &db,
            "12D3KooWIssuer",
            "12D3KooWSubject",
            "chat"
        )
        .unwrap());
    }

    #[test]
    fn test_get_permissions_by_issuer() {
        let db = Database::in_memory().unwrap();

        // Grant multiple permissions
        for (i, cap) in ["chat", "wall_read", "call"].iter().enumerate() {
            let grant = GrantData {
                grant_id: format!("grant-{}", i),
                issuer_peer_id: "12D3KooWIssuer".to_string(),
                subject_peer_id: format!("12D3KooWSubject{}", i),
                capability: cap.to_string(),
                scope_json: None,
                lamport_clock: i as i64,
                issued_at: chrono::Utc::now().timestamp(),
                expires_at: None,
                payload_cbor: vec![1, 2, 3],
                signature: vec![4, 5, 6],
            };
            PermissionsRepository::upsert_grant(&db, &grant).unwrap();
        }

        let perms =
            PermissionsRepository::get_permissions_by_issuer(&db, "12D3KooWIssuer").unwrap();
        assert_eq!(perms.len(), 3);
    }
}
