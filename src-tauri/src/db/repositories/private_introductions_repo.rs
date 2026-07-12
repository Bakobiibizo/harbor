use crate::db::Database;
use crate::models::{CapabilityGrantRecord, CapabilityRevocationRecord};
use rusqlite::{params, OptionalExtension, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntroductionDecision {
    Pending,
    Approved,
    Ignored,
    Rejected,
    Blocked,
}
impl IntroductionDecision {
    fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Ignored => "ignored",
            Self::Rejected => "rejected",
            Self::Blocked => "blocked",
        }
    }
}
pub struct PrivateIntroductionsRepository<'a> {
    db: &'a Database,
}
impl<'a> PrivateIntroductionsRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }
    pub fn receive(
        &self,
        id: &str,
        peer: &str,
        name: &str,
        digest: &[u8],
        at: i64,
    ) -> Result<bool> {
        self.db.with_connection(|c| {
            c.execute(
                "INSERT OR IGNORE INTO introduction_decisions VALUES(?,?,?,?, 'pending',?,NULL)",
                params![id, peer, name, digest, at],
            )
            .map(|n| n == 1)
        })
    }
    pub fn decide(&self, id: &str, decision: IntroductionDecision, at: i64) -> Result<bool> {
        self.db.with_connection_mut(|c|{let tx=c.transaction()?;let row:Option<(String,String)>=tx.query_row("SELECT requester_peer_id,requester_name FROM introduction_decisions WHERE request_id=?",[id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;let Some((peer,name))=row else{return Ok(false)};tx.execute("UPDATE introduction_decisions SET decision=?,decided_at=? WHERE request_id=?",params![decision.as_str(),at,id])?;if decision==IntroductionDecision::Blocked{tx.execute("INSERT INTO introduction_blocks VALUES(?,?,?) ON CONFLICT(requester_peer_id) DO UPDATE SET requester_name=excluded.requester_name,blocked_at=excluded.blocked_at",params![peer,name,at])?;}tx.commit()?;Ok(true)})
    }
    pub fn is_blocked(&self, peer: &str) -> Result<bool> {
        self.db.with_connection(|c| {
            c.query_row(
                "SELECT 1 FROM introduction_blocks WHERE requester_peer_id=?",
                [peer],
                |r| r.get::<_, i32>(0),
            )
            .optional()
            .map(|v| v.is_some())
        })
    }
    pub fn apply_grant(&self, g: &CapabilityGrantRecord, at: i64) -> Result<bool> {
        self.db.with_connection(|c|{let current:Option<i64>=c.query_row("SELECT revision FROM contact_capability_state WHERE grant_id=?",[&g.grant_id],|r|r.get(0)).optional()?;if current.is_some_and(|v|g.revision as i64<=v){return Ok(false)}c.execute("INSERT INTO contact_capability_state(grant_id,issuer_peer_id,subject_peer_id,capability,revision,issued_at,expires_at,revocation_id,revoked_at,updated_at) VALUES(?,?,?,?,?,?,?,?,NULL,?) ON CONFLICT(grant_id) DO UPDATE SET issuer_peer_id=excluded.issuer_peer_id,subject_peer_id=excluded.subject_peer_id,capability=excluded.capability,revision=excluded.revision,issued_at=excluded.issued_at,expires_at=excluded.expires_at,revocation_id=excluded.revocation_id,updated_at=excluded.updated_at",params![g.grant_id,g.issuer_peer_id,g.subject_peer_id,g.capability,g.revision as i64,g.issued_at,g.expires_at,g.revocation_id,at]).map(|_|true)})
    }
    pub fn apply_revocation(&self, r: &CapabilityRevocationRecord) -> Result<bool> {
        self.db.with_connection(|c|{let current:Option<(i64,Option<i64>)>=c.query_row("SELECT revision,revoked_at FROM contact_capability_state WHERE grant_id=? AND issuer_peer_id=?",params![r.grant_id,r.issuer_peer_id],|x|Ok((x.get(0)?,x.get(1)?))).optional()?;let Some((revision,revoked))=current else{return Ok(false)};if (r.revision as i64)<revision||revoked.is_some_and(|v|v>=r.revoked_at){return Ok(false)};c.execute("UPDATE contact_capability_state SET revision=?,revoked_at=?,updated_at=? WHERE grant_id=?",params![r.revision as i64,r.revoked_at,r.revoked_at,r.grant_id]).map(|_|true)})
    }
    pub fn is_authorized(
        &self,
        issuer: &str,
        subject: &str,
        capability: &str,
        at: i64,
    ) -> Result<bool> {
        self.db.with_connection(|c|c.query_row("SELECT 1 FROM contact_capability_state WHERE issuer_peer_id=? AND subject_peer_id=? AND capability=? AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at>?) LIMIT 1",params![issuer,subject,capability,at],|r|r.get::<_,i32>(0)).optional().map(|v|v.is_some()))
    }

    pub fn capability_decision(
        &self,
        issuer: &str,
        subject: &str,
        capability: &str,
        at: i64,
    ) -> Result<Option<bool>> {
        self.db.with_connection(|c| {
            let state: Option<(Option<i64>, Option<i64>)> = c.query_row(
                "SELECT expires_at,revoked_at FROM contact_capability_state WHERE issuer_peer_id=? AND subject_peer_id=? AND capability=? ORDER BY revision DESC LIMIT 1",
                params![issuer,subject,capability], |r| Ok((r.get(0)?,r.get(1)?)))
                .optional()?;
            Ok(state.map(|(expires,revoked)| revoked.is_none() && expires.is_none_or(|value| value > at)))
        })
    }
}
