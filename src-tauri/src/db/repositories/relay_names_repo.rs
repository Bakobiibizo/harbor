use crate::db::Database;
use crate::models::RelayKeyRotation;
use rusqlite::{params, OptionalExtension, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayKeyPin {
    pub key_id: String,
    pub public_key: Vec<u8>,
    pub sequence: u64,
    pub not_before: i64,
    pub not_after: Option<i64>,
}

pub struct RelayNamesRepository<'a> {
    db: &'a Database,
}
impl<'a> RelayNamesRepository<'a> {
    pub fn active_for_peer(&self, peer_id: &str, now: i64) -> Result<Option<Vec<u8>>> {
        self.db.with_connection(|c| c.query_row("SELECT claim_cbor FROM relay_name_claims WHERE peer_id=? AND status='active' AND not_after>=? ORDER BY sequence DESC LIMIT 1", params![peer_id,now], |r|r.get(0)).optional())
    }
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }
    pub fn pin_key(
        &self,
        relay: &str,
        key_id: &str,
        key: &[u8],
        not_before: i64,
        not_after: Option<i64>,
    ) -> Result<()> {
        self.db.with_connection(|connection| {
            let active: Option<(String, Vec<u8>)> = connection
                .query_row(
                    "SELECT key_id, public_key FROM relay_trust_keys
                     WHERE relay=? AND retired_at IS NULL ORDER BY sequence DESC LIMIT 1",
                    [relay],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            if active.as_ref().is_some_and(|(active_id, active_key)| {
                active_id != key_id || active_key.as_slice() != key
            }) {
                return Err(rusqlite::Error::InvalidQuery);
            }
            connection
                .execute(
                    "INSERT OR IGNORE INTO relay_trust_keys(
                        relay,key_id,public_key,not_before,not_after
                     ) VALUES(?,?,?,?,?)",
                    params![relay, key_id, key, not_before, not_after],
                )
                .map(|_| ())
        })
    }
    pub fn trusted_key(&self, relay: &str, key_id: &str, now: i64) -> Result<Option<Vec<u8>>> {
        self.db.with_connection(|c| c.query_row("SELECT public_key FROM relay_trust_keys WHERE relay=? AND key_id=? AND retired_at IS NULL AND not_before<=? AND (not_after IS NULL OR not_after>=?)", params![relay,key_id,now,now], |r| r.get(0)).optional())
    }
    pub fn active_pin(&self, relay: &str, key_id: &str, now: i64) -> Result<Option<RelayKeyPin>> {
        self.db.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT key_id, public_key, sequence, not_before, not_after
                     FROM relay_trust_keys
                     WHERE relay=? AND key_id=? AND retired_at IS NULL
                       AND not_before<=? AND (not_after IS NULL OR not_after>=?)",
                    params![relay, key_id, now, now],
                    |row| {
                        Ok(RelayKeyPin {
                            key_id: row.get(0)?,
                            public_key: row.get(1)?,
                            sequence: row.get::<_, i64>(2)? as u64,
                            not_before: row.get(3)?,
                            not_after: row.get(4)?,
                        })
                    },
                )
                .optional()
        })
    }
    pub fn apply_verified_rotation(
        &self,
        rotation: &RelayKeyRotation,
        encoded: &[u8],
        at: i64,
    ) -> Result<bool> {
        self.db.with_connection_mut(|connection| {
            let transaction = connection.transaction()?;
            let previous: Option<(i64, Vec<u8>)> = transaction
                .query_row(
                    "SELECT sequence, public_key FROM relay_trust_keys
                     WHERE relay=? AND key_id=? AND retired_at IS NULL",
                    params![rotation.relay, rotation.previous_key_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            let Some((previous_sequence, _)) = previous else {
                return Ok(false);
            };
            if rotation.sequence != previous_sequence as u64 + 1 {
                return Ok(false);
            }
            let replacement: Option<Vec<u8>> = transaction
                .query_row(
                    "SELECT public_key FROM relay_trust_keys WHERE relay=? AND key_id=?",
                    params![rotation.relay, rotation.next_key_id],
                    |row| row.get(0),
                )
                .optional()?;
            if replacement
                .as_deref()
                .is_some_and(|key| key != rotation.next_public_key)
            {
                return Ok(false);
            }
            transaction.execute(
                "UPDATE relay_trust_keys
                 SET retired_at=?, compromise_from=COALESCE(?, compromise_from)
                 WHERE relay=? AND key_id=? AND retired_at IS NULL",
                params![
                    at,
                    rotation.compromise_from,
                    rotation.relay,
                    rotation.previous_key_id
                ],
            )?;
            transaction.execute(
                "INSERT INTO relay_trust_keys(
                    relay,key_id,public_key,not_before,not_after,retired_at,
                    sequence,compromise_from,rotation_cbor
                 ) VALUES(?,?,?,?,?,NULL,?,NULL,?)
                 ON CONFLICT(relay,key_id) DO UPDATE SET
                    public_key=excluded.public_key,
                    not_before=excluded.not_before,
                    not_after=excluded.not_after,
                    retired_at=NULL,
                    sequence=excluded.sequence,
                    compromise_from=NULL,
                    rotation_cbor=excluded.rotation_cbor",
                params![
                    rotation.relay,
                    rotation.next_key_id,
                    rotation.next_public_key,
                    rotation.not_before,
                    rotation.not_after,
                    rotation.sequence as i64,
                    encoded
                ],
            )?;
            if let Some(compromise_from) = rotation.compromise_from {
                transaction.execute(
                    "UPDATE relay_name_claims SET status='retired', retired_at=?
                     WHERE relay=? AND relay_key_id=? AND status='active' AND not_before>=?",
                    params![
                        at,
                        rotation.relay,
                        rotation.previous_key_id,
                        compromise_from
                    ],
                )?;
            }
            transaction.commit()?;
            Ok(true)
        })
    }
    // The claim is persisted as a single normalized row whose columns mirror the
    // signed protocol object. Keeping the parameters explicit prevents accidental
    // omission when the SQL schema changes.
    #[allow(clippy::too_many_arguments)]
    pub fn cache_verified(
        &self,
        qualified: &str,
        local: &str,
        relay: &str,
        peer: &str,
        sequence: u64,
        cbor: &[u8],
        not_before: i64,
        not_after: i64,
        key_id: &str,
        now: i64,
    ) -> Result<()> {
        self.db.with_connection_mut(|c| { let tx=c.transaction()?; let current: Option<i64>=tx.query_row("SELECT MAX(sequence) FROM relay_name_claims WHERE relay=? AND local_name=?", params![relay,local], |r| r.get(0)).optional()?.flatten(); if current.is_some_and(|v| sequence as i64 <= v) { return Err(rusqlite::Error::InvalidQuery); } tx.execute("UPDATE relay_name_claims SET status='retired',retired_at=? WHERE relay=? AND local_name=? AND status='active'",params![now,relay,local])?; tx.execute("INSERT INTO relay_name_claims VALUES(?,?,?,?,?,?,?,?,?,'active',?,NULL)",params![qualified,local,relay,peer,sequence as i64,cbor,not_before,not_after,key_id,now])?; tx.commit() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relay_key_pin_survives_reopen_and_rejects_substitution() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("harbor.db");

        {
            let db = Database::new(path.clone()).unwrap();
            let repo = RelayNamesRepository::new(&db);
            repo.pin_key("relay.test", "2026-01", &[7; 32], 100, Some(300))
                .unwrap();
            assert!(repo
                .pin_key("relay.test", "2026-01", &[8; 32], 100, Some(300))
                .is_err());
        }

        let reopened = Database::new(path).unwrap();
        let repo = RelayNamesRepository::new(&reopened);
        assert_eq!(
            repo.trusted_key("relay.test", "2026-01", 200).unwrap(),
            Some(vec![7; 32])
        );
        assert_eq!(
            repo.trusted_key("relay.test", "2026-01", 301).unwrap(),
            None
        );
        assert_eq!(
            repo.trusted_key("relay.test", "unknown", 200).unwrap(),
            None
        );
    }

    #[test]
    fn verified_claim_cache_is_monotonic_and_survives_reopen() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("harbor.db");

        {
            let db = Database::new(path.clone()).unwrap();
            let repo = RelayNamesRepository::new(&db);
            repo.cache_verified(
                "@alice@relay.test",
                "alice",
                "relay.test",
                "peer-alice",
                1,
                &[1, 2, 3],
                100,
                300,
                "2026-01",
                100,
            )
            .unwrap();
            assert!(repo
                .cache_verified(
                    "@alice@relay.test",
                    "alice",
                    "relay.test",
                    "peer-alice",
                    1,
                    &[9, 9, 9],
                    100,
                    300,
                    "2026-01",
                    101,
                )
                .is_err());
        }

        let reopened = Database::new(path).unwrap();
        let repo = RelayNamesRepository::new(&reopened);
        assert_eq!(
            repo.active_for_peer("peer-alice", 200).unwrap(),
            Some(vec![1, 2, 3])
        );
        assert_eq!(repo.active_for_peer("peer-alice", 301).unwrap(), None);
    }
}
