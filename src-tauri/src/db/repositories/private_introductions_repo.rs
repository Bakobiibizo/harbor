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
    pub(crate) fn apply_grant(&self, g: &CapabilityGrantRecord, at: i64) -> Result<bool> {
        self.db.with_connection(|c|{let current:Option<i64>=c.query_row("SELECT MAX(revision) FROM contact_capability_state WHERE issuer_peer_id=? AND subject_peer_id=? AND capability=?",params![g.issuer_peer_id,g.subject_peer_id,g.capability],|r|r.get(0)).optional()?.flatten();if current.is_some_and(|v|g.revision as i64<=v){return Ok(false)}c.execute("INSERT INTO contact_capability_state(grant_id,issuer_peer_id,subject_peer_id,capability,revision,issued_at,expires_at,revocation_id,revoked_at,updated_at) VALUES(?,?,?,?,?,?,?,?,NULL,?) ON CONFLICT(grant_id) DO UPDATE SET issuer_peer_id=excluded.issuer_peer_id,subject_peer_id=excluded.subject_peer_id,capability=excluded.capability,revision=excluded.revision,issued_at=excluded.issued_at,expires_at=excluded.expires_at,revocation_id=excluded.revocation_id,revoked_at=NULL,updated_at=excluded.updated_at",params![g.grant_id,g.issuer_peer_id,g.subject_peer_id,g.capability,g.revision as i64,g.issued_at,g.expires_at,g.revocation_id,at]).map(|_|true)})
    }
    pub(crate) fn apply_revocation(&self, r: &CapabilityRevocationRecord) -> Result<bool> {
        self.db.with_connection_mut(|c| { let tx=c.transaction()?; let target:Option<(String,String,String,i64)>=tx.query_row("SELECT issuer_peer_id,subject_peer_id,capability,revision FROM contact_capability_state WHERE grant_id=?",[&r.grant_id],|x|Ok((x.get(0)?,x.get(1)?,x.get(2)?,x.get(3)?))).optional()?;let Some((issuer,subject,capability,current))=target else{return Ok(false)};if issuer!=r.issuer_peer_id || (r.revision as i64)<=current{return Ok(false)};let changed=tx.execute("UPDATE contact_capability_state SET revision=?,revoked_at=?,updated_at=? WHERE issuer_peer_id=? AND subject_peer_id=? AND capability=? AND revision<?",params![r.revision as i64,r.revoked_at,r.revoked_at,issuer,subject,capability,r.revision as i64])?;tx.commit()?;Ok(changed>0) })
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
                "SELECT expires_at,revoked_at FROM contact_capability_state WHERE issuer_peer_id=? AND subject_peer_id=? AND capability=? ORDER BY revision DESC, updated_at DESC LIMIT 1",
                params![issuer,subject,capability], |r| Ok((r.get(0)?,r.get(1)?)))
                .optional()?;
            Ok(state.map(|(expires,revoked)| revoked.is_none() && expires.is_none_or(|value| value > at)))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_review_decision_and_block_survives_restart() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("harbor.db");
        let decisions = [
            IntroductionDecision::Approved,
            IntroductionDecision::Ignored,
            IntroductionDecision::Rejected,
            IntroductionDecision::Blocked,
        ];

        {
            let db = Database::new(path.clone()).unwrap();
            let repo = PrivateIntroductionsRepository::new(&db);
            for (index, decision) in decisions.iter().copied().enumerate() {
                let id = format!("request-{index}");
                let peer = format!("peer-{index}");
                assert!(repo
                    .receive(
                        &id,
                        &peer,
                        &format!("@user{index}@relay.test"),
                        &[index as u8; 32],
                        100
                    )
                    .unwrap());
                assert!(repo.decide(&id, decision, 101).unwrap());
            }
        }

        let reopened = Database::new(path).unwrap();
        for (index, decision) in decisions.iter().copied().enumerate() {
            let stored: String = reopened
                .with_connection(|connection| {
                    connection.query_row(
                        "SELECT decision FROM introduction_decisions WHERE request_id=?",
                        [format!("request-{index}")],
                        |row| row.get(0),
                    )
                })
                .unwrap();
            assert_eq!(stored, decision.as_str());
        }
        let repo = PrivateIntroductionsRepository::new(&reopened);
        assert!(repo.is_blocked("peer-3").unwrap());
        assert!(!repo.is_blocked("peer-0").unwrap());
    }

    #[test]
    fn offline_revocation_survives_restart_and_rejects_stale_grants() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("harbor.db");
        let grant = |revision| CapabilityGrantRecord {
            domain: "harbor/capability-grant/1".into(),
            version: 1,
            grant_id: "grant-1".into(),
            issuer_peer_id: "alice".into(),
            subject_peer_id: "bob".into(),
            capability: "wall:read".into(),
            revision,
            issued_at: 100 + revision as i64,
            expires_at: Some(1_000),
            revocation_id: "revoke-1".into(),
        };

        {
            let db = Database::new(path.clone()).unwrap();
            let repo = PrivateIntroductionsRepository::new(&db);
            assert!(repo.apply_grant(&grant(3), 103).unwrap());
            assert!(repo
                .is_authorized("alice", "bob", "wall:read", 104)
                .unwrap());
            assert!(repo
                .apply_revocation(&CapabilityRevocationRecord {
                    domain: "harbor/capability-revocation/1".into(),
                    version: 1,
                    grant_id: "grant-1".into(),
                    issuer_peer_id: "alice".into(),
                    revision: 5,
                    revoked_at: 105,
                    revocation_id: "revoke-1".into(),
                })
                .unwrap());
        }

        let reopened = Database::new(path).unwrap();
        let repo = PrivateIntroductionsRepository::new(&reopened);
        assert!(!repo
            .is_authorized("alice", "bob", "wall:read", 106)
            .unwrap());
        assert!(!repo.apply_grant(&grant(4), 106).unwrap());
        assert!(!repo
            .is_authorized("alice", "bob", "wall:read", 106)
            .unwrap());
        assert!(repo.apply_grant(&grant(6), 106).unwrap());
        assert!(repo
            .is_authorized("alice", "bob", "wall:read", 107)
            .unwrap());
    }
}
