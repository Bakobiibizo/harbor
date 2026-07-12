use crate::db::Database;
use rusqlite::{params, OptionalExtension, Result};

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
            b"sig",
            1,
        )
        .unwrap();
        assert_eq!(repo.pending().unwrap().len(), 1);
        assert!(repo.review("m", "accepted", 2).unwrap());
        assert!(!repo.review("m", "declined", 3).unwrap());
        assert!(repo.pending().unwrap().is_empty());
    }
}
pub struct MentionsRepository<'a> {
    db: &'a Database,
}
impl<'a> MentionsRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }
    pub fn resolve_claim(&self, name: &str) -> Result<Option<(String, String)>> {
        self.db.with_connection(|c| c.query_row("SELECT peer_id,hex(claim_cbor) FROM relay_name_claims WHERE qualified_name=? AND status='active' AND not_after>=strftime('%s','now') ORDER BY sequence DESC LIMIT 1",[name],|r|Ok((r.get(0)?,r.get::<_,String>(1)?.to_lowercase()))).optional())
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
        signature: &[u8],
        at: i64,
    ) -> Result<()> {
        self.db.with_connection(|c|c.execute("INSERT INTO private_mentions(mention_id,post_id,qualified_name,intent,sender_peer_id,authorized_peer_id,claim_digest,preview,envelope_ciphertext,signature,created_at) VALUES(?,?,?,?,?,?,?,?,?,?,?)",params![id,post,name,intent,sender,authorized,digest,preview,cipher,signature,at]).map(|_|()))
    }
    pub fn pending(&self) -> Result<Vec<StoredMention>> {
        self.db.with_connection(|c|{let mut s=c.prepare("SELECT mention_id,post_id,qualified_name,intent,sender_peer_id,preview,status,created_at FROM private_mentions WHERE status='pending' ORDER BY created_at DESC")?;let rows=s.query_map([],|r|Ok(StoredMention{mention_id:r.get(0)?,post_id:r.get(1)?,qualified_name:r.get(2)?,intent:r.get(3)?,sender_peer_id:r.get(4)?,preview:r.get(5)?,status:r.get(6)?,created_at:r.get(7)?}))?.collect();rows})
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
