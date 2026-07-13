use crate::db::Database;
use rusqlite::{params, OptionalExtension, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContactRequestRecord {
    pub request_id: String,
    pub peer_id: String,
    pub direction: String,
    pub display_name: Option<String>,
    pub public_key: Option<Vec<u8>>,
    pub x25519_public: Option<Vec<u8>>,
    pub avatar_hash: Option<String>,
    pub bio: Option<String>,
    pub status: String,
    pub pending_action: Option<String>,
    pub error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

pub struct ContactRequestsRepository<'a> {
    db: &'a Database,
}

impl<'a> ContactRequestsRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn upsert(
        &self,
        request_id: &str,
        peer_id: &str,
        direction: &str,
        display_name: Option<&str>,
        public_key: Option<&[u8]>,
        x25519_public: Option<&[u8]>,
        avatar_hash: Option<&str>,
        bio: Option<&str>,
        status: &str,
        pending_action: Option<&str>,
        error: Option<&str>,
        at: i64,
    ) -> Result<()> {
        self.db.with_connection(|connection| {
            connection.execute(
                "INSERT INTO contact_requests(
                    request_id,peer_id,direction,display_name,public_key,x25519_public,
                    avatar_hash,bio,status,pending_action,error,created_at,updated_at
                 ) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?)
                 ON CONFLICT(peer_id,direction) DO UPDATE SET
                    request_id=excluded.request_id,
                    display_name=CASE WHEN contact_requests.request_id=excluded.request_id
                      THEN COALESCE(excluded.display_name,contact_requests.display_name)
                      ELSE excluded.display_name END,
                    public_key=CASE WHEN contact_requests.request_id=excluded.request_id
                      THEN COALESCE(excluded.public_key,contact_requests.public_key)
                      ELSE excluded.public_key END,
                    x25519_public=CASE WHEN contact_requests.request_id=excluded.request_id
                      THEN COALESCE(excluded.x25519_public,contact_requests.x25519_public)
                      ELSE excluded.x25519_public END,
                    avatar_hash=CASE WHEN contact_requests.request_id=excluded.request_id
                      THEN COALESCE(excluded.avatar_hash,contact_requests.avatar_hash)
                      ELSE excluded.avatar_hash END,
                    bio=CASE WHEN contact_requests.request_id=excluded.request_id
                      THEN COALESCE(excluded.bio,contact_requests.bio) ELSE excluded.bio END,
                    status=CASE
                      WHEN contact_requests.request_id=excluded.request_id
                           AND contact_requests.status IN ('accepted','declined','revoked')
                      THEN contact_requests.status ELSE excluded.status END,
                    pending_action=excluded.pending_action,
                    error=excluded.error,
                    created_at=CASE WHEN contact_requests.request_id=excluded.request_id
                                    THEN contact_requests.created_at ELSE excluded.created_at END,
                    updated_at=excluded.updated_at
                 WHERE contact_requests.request_id=excluded.request_id
                    OR excluded.created_at>contact_requests.created_at",
                params![
                    request_id,
                    peer_id,
                    direction,
                    display_name,
                    public_key,
                    x25519_public,
                    avatar_hash,
                    bio,
                    status,
                    pending_action,
                    error,
                    at,
                    at
                ],
            )?;
            Ok(())
        })
    }

    pub fn update_status(
        &self,
        request_id: &str,
        status: &str,
        pending_action: Option<&str>,
        error: Option<&str>,
        at: i64,
    ) -> Result<bool> {
        self.db.with_connection(|connection| {
            connection
                .execute(
                    "UPDATE contact_requests SET status=?,pending_action=?,error=?,updated_at=?
                     WHERE request_id=?",
                    params![status, pending_action, error, at, request_id],
                )
                .map(|changed| changed > 0)
        })
    }

    pub fn get(&self, request_id: &str) -> Result<Option<ContactRequestRecord>> {
        self.db.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT request_id,peer_id,direction,display_name,public_key,x25519_public,
                            avatar_hash,bio,status,pending_action,error,created_at,updated_at
                     FROM contact_requests WHERE request_id=?",
                    [request_id],
                    map_record,
                )
                .optional()
        })
    }

    pub fn for_peer(&self, peer_id: &str, direction: &str) -> Result<Option<ContactRequestRecord>> {
        self.db.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT request_id,peer_id,direction,display_name,public_key,x25519_public,
                            avatar_hash,bio,status,pending_action,error,created_at,updated_at
                     FROM contact_requests WHERE peer_id=? AND direction=?",
                    params![peer_id, direction],
                    map_record,
                )
                .optional()
        })
    }

    pub fn list(&self) -> Result<Vec<ContactRequestRecord>> {
        self.db.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT request_id,peer_id,direction,display_name,public_key,x25519_public,
                        avatar_hash,bio,status,pending_action,error,created_at,updated_at
                 FROM contact_requests ORDER BY updated_at DESC",
            )?;
            let rows = statement.query_map([], map_record)?.collect();
            rows
        })
    }

    pub fn revoke_peer(&self, peer_id: &str, at: i64) -> Result<usize> {
        self.db.with_connection(|connection| {
            connection.execute(
                "UPDATE contact_requests
                 SET status='revoked',pending_action='revoked',error=NULL,updated_at=?
                 WHERE peer_id=?",
                params![at, peer_id],
            )
        })
    }
}

fn map_record(row: &rusqlite::Row<'_>) -> Result<ContactRequestRecord> {
    Ok(ContactRequestRecord {
        request_id: row.get(0)?,
        peer_id: row.get(1)?,
        direction: row.get(2)?,
        display_name: row.get(3)?,
        public_key: row.get(4)?,
        x25519_public: row.get(5)?,
        avatar_hash: row.get(6)?,
        bio: row.get(7)?,
        status: row.get(8)?,
        pending_action: row.get(9)?,
        error: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_and_replay_are_durable_and_terminal_safe() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("contacts.db");
        {
            let db = Database::new(path.clone()).unwrap();
            let repo = ContactRequestsRepository::new(&db);
            repo.upsert(
                "r1",
                "peer-a",
                "incoming",
                Some("Alice"),
                Some(&[1; 32]),
                Some(&[2; 32]),
                None,
                None,
                "review",
                None,
                None,
                10,
            )
            .unwrap();
            assert!(repo
                .update_status("r1", "declined", None, None, 11)
                .unwrap());
            repo.upsert(
                "r1",
                "peer-a",
                "incoming",
                Some("Alice"),
                None,
                None,
                None,
                None,
                "review",
                None,
                None,
                12,
            )
            .unwrap();
            assert_eq!(repo.get("r1").unwrap().unwrap().status, "declined");
            repo.upsert(
                "r2",
                "peer-a",
                "incoming",
                Some("Alice"),
                Some(&[3; 32]),
                Some(&[4; 32]),
                None,
                None,
                "review",
                None,
                None,
                20,
            )
            .unwrap();
            repo.upsert(
                "r1",
                "peer-a",
                "incoming",
                Some("Alice"),
                None,
                None,
                None,
                None,
                "review",
                None,
                None,
                12,
            )
            .unwrap();
            assert_eq!(
                repo.for_peer("peer-a", "incoming")
                    .unwrap()
                    .unwrap()
                    .request_id,
                "r2"
            );
        }
        let reopened = Database::new(path).unwrap();
        let request = ContactRequestsRepository::new(&reopened)
            .get("r2")
            .unwrap()
            .unwrap();
        assert_eq!(request.status, "review");
        assert_eq!(request.public_key.unwrap(), vec![3; 32]);
    }
}
