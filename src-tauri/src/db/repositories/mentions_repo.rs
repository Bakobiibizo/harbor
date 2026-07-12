use crate::db::Database;
use crate::models::NameClaim;
use rusqlite::{params, OptionalExtension, Result};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone)]
pub struct StoredMention {
    pub mention_id: String,
    pub post_id: String,
    pub qualified_name: String,
    pub intent: String,
    pub sender_peer_id: String,
    pub preview: String,
    pub status: String,
    pub created_at: i64,
}
#[derive(Debug, Clone)]
pub struct QueuedMentionEnvelope {
    pub mention_id: String,
    pub target: String,
    pub ephemeral_public_key: Vec<u8>,
    pub ciphertext: Vec<u8>,
    pub expires_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn review_is_single_use_and_persists_ciphertext() {
        let db = Database::in_memory().unwrap();
        let repo = MentionsRepository::new(&db);
        repo.insert(
            "m",
            "p",
            "@bugs@harbor.social",
            "repost-request",
            "sender",
            None,
            None,
            "preview",
            b"cipher",
            b"ephemeral",
            b"sig",
            1,
        )
        .unwrap();
        assert_eq!(repo.pending("recipient").unwrap().len(), 1);
        assert!(repo.review("m", "accepted", 2).unwrap());
        assert!(!repo.review("m", "declined", 3).unwrap());
        assert!(repo.pending("recipient").unwrap().is_empty());
    }
}
pub struct MentionsRepository<'a> {
    db: &'a Database,
}
impl<'a> MentionsRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }
    pub fn resolve_claim(&self, name: &str) -> Result<Option<(String, String, Vec<u8>)>> {
        self.db.with_connection(|c| c.query_row("SELECT peer_id,claim_cbor FROM relay_name_claims WHERE qualified_name=? AND status='active' AND not_after>=strftime('%s','now') ORDER BY sequence DESC LIMIT 1",[name],|r|{let peer:String=r.get(0)?;let bytes:Vec<u8>=r.get(1)?;let claim:NameClaim=ciborium::de::from_reader(bytes.as_slice()).map_err(|e|rusqlite::Error::FromSqlConversionFailure(bytes.len(),rusqlite::types::Type::Blob,Box::new(e)))?;Ok((peer,hex::encode(Sha256::digest(&bytes)),claim.request.x25519_public_key))}).optional())
    }
    #[allow(clippy::too_many_arguments)]
    pub fn insert(
        &self,
        id: &str,
        post: &str,
        name: &str,
        intent: &str,
        sender: &str,
        authorized: Option<&str>,
        digest: Option<&str>,
        preview: &str,
        cipher: &[u8],
        ephemeral: &[u8],
        signature: &[u8],
        at: i64,
    ) -> Result<()> {
        self.db.with_connection(|c|c.execute("INSERT INTO private_mentions(mention_id,post_id,qualified_name,intent,sender_peer_id,authorized_peer_id,claim_digest,preview,envelope_ciphertext,ephemeral_public_key,signature,created_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",params![id,post,name,intent,sender,authorized,digest,preview,cipher,ephemeral,signature,at]).map(|_|()))
    }
    pub fn enqueue_outbound(
        &self,
        id: &str,
        target: &str,
        ephemeral: &[u8],
        cipher: &[u8],
        expires: i64,
    ) -> Result<()> {
        self.db.with_connection(|c| {
            c.execute(
                "INSERT INTO private_mention_outbox VALUES(?,?,?,?,?,'queued')",
                params![id, target, ephemeral, cipher, expires],
            )
            .map(|_| ())
        })
    }
    pub fn queued_outbound(&self, now: i64, limit: u32) -> Result<Vec<QueuedMentionEnvelope>> {
        self.db.with_connection(|c| { let mut s=c.prepare("SELECT mention_id,target,ephemeral_public_key,ciphertext,expires_at FROM private_mention_outbox WHERE delivery_status='queued' AND expires_at>? ORDER BY rowid LIMIT ?")?; let rows=s.query_map(params![now,limit],|r|Ok(QueuedMentionEnvelope{mention_id:r.get(0)?,target:r.get(1)?,ephemeral_public_key:r.get(2)?,ciphertext:r.get(3)?,expires_at:r.get(4)?}))?.collect(); rows })
    }
    pub fn mark_delivered(&self, mention_id: &str) -> Result<bool> {
        self.db.with_connection(|c|c.execute("UPDATE private_mention_outbox SET delivery_status='delivered' WHERE mention_id=? AND delivery_status='queued'",[mention_id]).map(|n|n==1))
    }
    pub fn block_sender(&self, peer_id: &str, at: i64) -> Result<()> {
        self.db.with_connection(|c| c.execute("INSERT INTO private_mention_blocks VALUES(?,?) ON CONFLICT(sender_peer_id) DO UPDATE SET blocked_at=excluded.blocked_at",params![peer_id,at]).map(|_| ()))
    }
    pub fn pending(&self, exclude_sender: &str) -> Result<Vec<StoredMention>> {
        self.db.with_connection(|c|{let mut s=c.prepare("SELECT mention_id,post_id,qualified_name,intent,sender_peer_id,preview,status,created_at FROM private_mentions WHERE status='pending' AND sender_peer_id != ? ORDER BY created_at DESC")?;let rows=s.query_map([exclude_sender],|r|Ok(StoredMention{mention_id:r.get(0)?,post_id:r.get(1)?,qualified_name:r.get(2)?,intent:r.get(3)?,sender_peer_id:r.get(4)?,preview:r.get(5)?,status:r.get(6)?,created_at:r.get(7)?}))?.collect();rows})
    }
    pub fn review(&self, id: &str, status: &str, at: i64) -> Result<bool> {
        self.db.with_connection(|c|c.execute("UPDATE private_mentions SET status=?,reviewed_at=? WHERE mention_id=? AND status='pending'",params![status,at,id]).map(|n|n==1))
    }
    pub fn intent(&self, id: &str) -> Result<Option<String>> {
        self.db.with_connection(|c| {
            c.query_row(
                "SELECT intent FROM private_mentions WHERE mention_id=?",
                [id],
                |r| r.get(0),
            )
            .optional()
        })
    }
    pub fn get(&self, id: &str) -> Result<Option<StoredMention>> {
        self.db.with_connection(|c| c.query_row("SELECT mention_id,post_id,qualified_name,intent,sender_peer_id,preview,status,created_at FROM private_mentions WHERE mention_id=?",[id],|r|Ok(StoredMention{mention_id:r.get(0)?,post_id:r.get(1)?,qualified_name:r.get(2)?,intent:r.get(3)?,sender_peer_id:r.get(4)?,preview:r.get(5)?,status:r.get(6)?,created_at:r.get(7)?})).optional())
    }
}
