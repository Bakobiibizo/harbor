use crate::db::Database;
use rusqlite::{params, OptionalExtension, Result};

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
        self.db.with_connection(|c| { let existing:Option<Vec<u8>>=c.query_row("SELECT public_key FROM relay_trust_keys WHERE relay=? AND key_id=?",params![relay,key_id],|r|r.get(0)).optional()?;if existing.as_deref().is_some_and(|v|v!=key){return Err(rusqlite::Error::InvalidQuery)}c.execute("INSERT OR IGNORE INTO relay_trust_keys(relay,key_id,public_key,not_before,not_after) VALUES(?,?,?,?,?)", params![relay,key_id,key,not_before,not_after]).map(|_| ())})
    }
    pub fn trusted_key(&self, relay: &str, key_id: &str, now: i64) -> Result<Option<Vec<u8>>> {
        self.db.with_connection(|c| c.query_row("SELECT public_key FROM relay_trust_keys WHERE relay=? AND key_id=? AND retired_at IS NULL AND not_before<=? AND (not_after IS NULL OR not_after>=?)", params![relay,key_id,now,now], |r| r.get(0)).optional())
    }
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
