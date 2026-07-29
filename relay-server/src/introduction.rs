//! Opaque, authenticated introduction envelope transport.
//!
//! The relay can route encrypted envelopes, but cannot inspect their message
//! bodies or infer target existence from the submission response.

use crate::abuse::{AbuseGuard, WorkChallenge};
use crate::auth::AuthService;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

const MAX_CIPHERTEXT_BYTES: usize = 64 * 1024;
const MAX_QUEUE_PER_TARGET: i64 = 100;
const FALLBACK_MAX_QUEUE_GLOBAL: i64 = 10_000;
const MAX_TTL_SECS: i64 = 24 * 60 * 60;
const GENERIC_RETRY_AFTER: u32 = 3600;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS introduction_envelopes (
 request_id TEXT PRIMARY KEY,
 target_peer_id TEXT NOT NULL,
 requester_peer_id TEXT NOT NULL,
 requester_ephemeral_key BLOB NOT NULL,
 ciphertext BLOB NOT NULL,
 issued_at INTEGER NOT NULL,
 expires_at INTEGER NOT NULL,
 stored_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_introduction_target_expiry
 ON introduction_envelopes(target_peer_id, expires_at, stored_at);
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntroductionEnvelope {
    pub version: u8,
    pub request_id: String,
    pub target: String,
    pub requester_peer_id: String,
    pub requester_ephemeral_x25519_key: Vec<u8>,
    pub message_ciphertext: Vec<u8>,
    pub issued_at: i64,
    pub expires_at: i64,
    pub work_challenge: WorkChallenge,
    pub work_nonce: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptedResponse {
    pub status: &'static str,
    pub request_id: String,
    pub retry_after: u32,
}

impl AcceptedResponse {
    fn generic(request_id: String) -> Self {
        Self {
            status: "accepted-for-processing",
            request_id,
            retry_after: GENERIC_RETRY_AFTER,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdmissionCode {
    Queued,
    Duplicate,
    AuthRejected,
    EnvelopeInvalid,
    WorkBindingRejected,
    WorkRejected,
    TargetUnavailable,
    CapacityRejected,
    StorageFailure,
}

impl AdmissionCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Duplicate => "duplicate",
            Self::AuthRejected => "auth_rejected",
            Self::EnvelopeInvalid => "envelope_invalid",
            Self::WorkBindingRejected => "work_binding_rejected",
            Self::WorkRejected => "work_rejected",
            Self::TargetUnavailable => "target_unavailable",
            Self::CapacityRejected => "capacity_rejected",
            Self::StorageFailure => "storage_failure",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionOutcome {
    pub response: AcceptedResponse,
    pub code: AdmissionCode,
}

impl AdmissionOutcome {
    fn new(response: &AcceptedResponse, code: AdmissionCode) -> Self {
        Self {
            response: response.clone(),
            code,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedEnvelope {
    pub request_id: String,
    pub requester_peer_id: String,
    pub requester_ephemeral_x25519_key: Vec<u8>,
    pub message_ciphertext: Vec<u8>,
    pub issued_at: i64,
    pub expires_at: i64,
}

pub struct IntroductionService<'a> {
    conn: &'a Connection,
    auth: &'a AuthService,
    abuse: &'a mut AbuseGuard,
}

impl<'a> IntroductionService<'a> {
    pub fn new(
        conn: &'a Connection,
        auth: &'a AuthService,
        abuse: &'a mut AbuseGuard,
    ) -> rusqlite::Result<Self> {
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn, auth, abuse })
    }

    /// Admit an opaque envelope with an internal-only typed decision suitable
    /// for metrics and structured logs. Every decision carries the exact same
    /// peer response shape, and the admission code is never serialized.
    pub fn submit_with_outcome(
        &mut self,
        session_token: &str,
        source_network: &str,
        envelope: IntroductionEnvelope,
        at: i64,
        _known_contact_hint: bool,
    ) -> AdmissionOutcome {
        let response = AcceptedResponse::generic(envelope.request_id.clone());
        let Ok(peer_id) = self.auth.authorize(session_token, "introduce", at) else {
            return AdmissionOutcome::new(&response, AdmissionCode::AuthRejected);
        };
        if peer_id.to_string() != envelope.requester_peer_id
            || envelope.version != 1
            || uuid::Uuid::parse_str(&envelope.request_id).is_err()
            || envelope.requester_ephemeral_x25519_key.len() != 32
            || envelope.message_ciphertext.is_empty()
            || envelope.message_ciphertext.len() > MAX_CIPHERTEXT_BYTES
            || envelope.issued_at > at + 60
            || envelope.expires_at <= at
            || envelope.expires_at - envelope.issued_at > MAX_TTL_SECS
        {
            return AdmissionOutcome::new(&response, AdmissionCode::EnvelopeInvalid);
        }
        if envelope.work_challenge.requester != envelope.requester_peer_id
            || envelope.work_challenge.target != envelope.target
            || envelope.work_challenge.action != "introduce"
            || envelope.work_challenge.audience != "introduce"
            || envelope.work_challenge.relay != self.auth.relay_name()
            || envelope.work_challenge.key_id != self.auth.key_id()
        {
            return AdmissionOutcome::new(&response, AdmissionCode::WorkBindingRejected);
        }
        if self.purge_expired(at).is_err() {
            return AdmissionOutcome::new(&response, AdmissionCode::StorageFailure);
        }
        let target_peer: Option<String> = match self.conn.query_row(
            "SELECT peer_id FROM relay_name_claims WHERE ('@' || local_name || '@' || relay)=? AND status='active' AND not_before<=? AND not_after>=? ORDER BY sequence DESC LIMIT 1",
            params![envelope.target, at, at], |r| r.get(0)).optional() {
                Ok(target) => target,
                Err(_) => return AdmissionOutcome::new(&response, AdmissionCode::StorageFailure),
            };
        let known_contact = match target_peer.as_ref() {
            Some(target) => match self.conn.query_row("SELECT EXISTS(SELECT 1 FROM wall_read_grants WHERE issuer_peer_id=? AND subject_peer_id=? AND capability IN ('wall_read','wall:read') AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at>=?))",params![target,envelope.requester_peer_id,at],|r|r.get::<_,bool>(0)) {
                Ok(value) => value,
                Err(_) => return AdmissionOutcome::new(&response, AdmissionCode::StorageFailure),
            },
            None => false,
        };
        if self
            .abuse
            .check_and_record(
                &envelope.work_challenge,
                envelope.work_nonce,
                source_network,
                at,
                known_contact,
            )
            .is_err()
        {
            return AdmissionOutcome::new(&response, AdmissionCode::WorkRejected);
        }
        let Some(target_peer) = target_peer else {
            return AdmissionOutcome::new(&response, AdmissionCode::TargetUnavailable);
        };
        let target_count: i64 = match self.conn.query_row(
            "SELECT COUNT(*) FROM introduction_envelopes WHERE target_peer_id=?",
            [&target_peer],
            |r| r.get(0),
        ) {
            Ok(count) => count,
            Err(_) => return AdmissionOutcome::new(&response, AdmissionCode::StorageFailure),
        };
        let global_count: i64 =
            match self
                .conn
                .query_row("SELECT COUNT(*) FROM introduction_envelopes", [], |r| {
                    r.get(0)
                }) {
                Ok(count) => count,
                Err(_) => return AdmissionOutcome::new(&response, AdmissionCode::StorageFailure),
            };
        let has_resource_limits = self
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='relay_resource_limits')",
                [],
                |row| row.get::<_, bool>(0),
            )
            .unwrap_or(false);
        let max_global = if has_resource_limits {
            self.conn
                .query_row(
                    "SELECT max_introductions FROM relay_resource_limits WHERE singleton=1",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .optional()
                .ok()
                .flatten()
                .unwrap_or(FALLBACK_MAX_QUEUE_GLOBAL)
        } else {
            FALLBACK_MAX_QUEUE_GLOBAL
        };
        if target_count >= MAX_QUEUE_PER_TARGET || global_count >= max_global {
            return AdmissionOutcome::new(&response, AdmissionCode::CapacityRejected);
        }
        let inserted = self.conn.execute(
            "INSERT OR IGNORE INTO introduction_envelopes VALUES(?,?,?,?,?,?,?,?)",
            params![
                envelope.request_id,
                target_peer,
                envelope.requester_peer_id,
                envelope.requester_ephemeral_x25519_key,
                envelope.message_ciphertext,
                envelope.issued_at,
                envelope.expires_at,
                at
            ],
        );
        match inserted {
            Ok(1) => AdmissionOutcome::new(&response, AdmissionCode::Queued),
            Ok(_) => AdmissionOutcome::new(&response, AdmissionCode::Duplicate),
            Err(error) if error.to_string().contains("RELAY_CAPACITY_INTRODUCTIONS") => {
                AdmissionOutcome::new(&response, AdmissionCode::CapacityRejected)
            }
            Err(_) => AdmissionOutcome::new(&response, AdmissionCode::StorageFailure),
        }
    }

    /// Fetch and consume envelopes for the authenticated target identity.
    pub fn take(
        &self,
        session_token: &str,
        at: i64,
        limit: u32,
    ) -> Result<Vec<QueuedEnvelope>, String> {
        let peer = self
            .auth
            .authorize(session_token, "introductions:read", at)?
            .to_string();
        self.purge_expired(at).map_err(|error| error.to_string())?;
        let mut statement = self.conn.prepare("SELECT request_id,requester_peer_id,requester_ephemeral_key,ciphertext,issued_at,expires_at FROM introduction_envelopes WHERE target_peer_id=? ORDER BY stored_at LIMIT ?").map_err(|e|e.to_string())?;
        let rows = statement
            .query_map(params![peer, limit.clamp(1, 100)], |r| {
                Ok(QueuedEnvelope {
                    request_id: r.get(0)?,
                    requester_peer_id: r.get(1)?,
                    requester_ephemeral_x25519_key: r.get(2)?,
                    message_ciphertext: r.get(3)?,
                    issued_at: r.get(4)?,
                    expires_at: r.get(5)?,
                })
            })
            .map_err(|e| e.to_string())?;
        let values: Vec<_> = rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?;
        Ok(values)
    }
    pub fn acknowledge(
        &self,
        session_token: &str,
        request_ids: &[String],
        at: i64,
    ) -> Result<u32, String> {
        let peer = self
            .auth
            .authorize(session_token, "introductions:read", at)?
            .to_string();
        let mut removed = 0;
        for id in request_ids.iter().take(100) {
            removed += self
                .conn
                .execute(
                    "DELETE FROM introduction_envelopes WHERE request_id=? AND target_peer_id=?",
                    params![id, peer],
                )
                .map_err(|e| e.to_string())? as u32
        }
        Ok(removed)
    }

    fn purge_expired(&self, at: i64) -> rusqlite::Result<usize> {
        self.conn.execute(
            "DELETE FROM introduction_envelopes WHERE expires_at<=?",
            [at],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abuse::Limits;
    use crate::auth::AuthChallenge;
    use libp2p::identity::Keypair;

    fn challenge_bytes(value: &AuthChallenge) -> Vec<u8> {
        let mut unsigned = value.clone();
        unsigned.relay_signature.clear();
        let mut out = Vec::new();
        ciborium::ser::into_writer(&unsigned, &mut out).unwrap();
        out
    }
    fn token(auth: &mut AuthService, key: &Keypair, audience: &str, at: i64) -> String {
        let peer = key.public().to_peer_id();
        let c = auth.issue_challenge(&peer, audience, at).unwrap();
        let sig = key.sign(&challenge_bytes(&c)).unwrap();
        auth.complete(&c, &key.public().encode_protobuf(), &sig, at)
            .unwrap()
    }
    fn db(target: &str) -> Connection {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE relay_name_claims(local_name TEXT,relay TEXT,peer_id TEXT,sequence INTEGER,status TEXT,not_before INTEGER,not_after INTEGER); CREATE TABLE wall_read_grants(issuer_peer_id TEXT,subject_peer_id TEXT,capability TEXT,revoked_at INTEGER,expires_at INTEGER);").unwrap();
        c.execute(
            "INSERT INTO relay_name_claims VALUES('alice','relay.test',?,1,'active',0,9999)",
            [target],
        )
        .unwrap();
        c
    }
    fn limits() -> Limits {
        Limits {
            peer: 10,
            network: 10,
            target: 10,
            action: 10,
            global: 20,
            window_secs: 60,
        }
    }
    fn envelope(requester: String, id: String, difficulty: u8) -> IntroductionEnvelope {
        let work = WorkChallenge {
            id: id.clone(),
            relay: "relay.test".into(),
            requester: requester.clone(),
            target: "@alice@relay.test".into(),
            action: "introduce".into(),
            audience: "introduce".into(),
            expires_at: 300,
            difficulty,
            key_id: "k1".into(),
            relay_signature: vec![1],
            delivery_key: vec![7; 32],
        };
        let nonce = (0..).find(|n| work.verify(*n, 100)).unwrap();
        IntroductionEnvelope {
            version: 1,
            request_id: id,
            target: "@alice@relay.test".into(),
            requester_peer_id: requester,
            requester_ephemeral_x25519_key: vec![7; 32],
            message_ciphertext: vec![9; 48],
            issued_at: 100,
            expires_at: 200,
            work_challenge: work,
            work_nonce: nonce,
        }
    }

    #[test]
    fn authenticated_submit_and_target_fetch_are_opaque() {
        let relay = Keypair::generate_ed25519();
        let requester = Keypair::generate_ed25519();
        let target = Keypair::generate_ed25519();
        let mut auth = AuthService::new("relay.test", "k1", relay);
        let submit = token(&mut auth, &requester, "introduce", 100);
        let read = token(&mut auth, &target, "introductions:read", 100);
        let conn = db(&target.public().to_peer_id().to_string());
        let mut abuse = AbuseGuard::new(limits());
        let mut service = IntroductionService::new(&conn, &auth, &mut abuse).unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        service.abuse.remember(
            envelope(requester.public().to_peer_id().to_string(), id.clone(), 4).work_challenge,
        );
        let outcome = service.submit_with_outcome(
            &submit,
            "10.0.0.0/24",
            envelope(requester.public().to_peer_id().to_string(), id.clone(), 4),
            100,
            false,
        );
        assert_eq!(outcome.response.status, "accepted-for-processing");
        assert_eq!(outcome.code, AdmissionCode::Queued);
        let queued = service.take(&read, 101, 10).unwrap();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].message_ciphertext, vec![9; 48]);
        service.acknowledge(&read, &[id], 101).unwrap();
        assert!(service.take(&read, 101, 10).unwrap().is_empty());
    }
    #[test]
    fn unknown_invalid_and_replay_have_identical_response() {
        let relay = Keypair::generate_ed25519();
        let requester = Keypair::generate_ed25519();
        let target = Keypair::generate_ed25519();
        let mut auth = AuthService::new("relay.test", "k1", relay);
        let submit = token(&mut auth, &requester, "introduce", 100);
        let conn = db(&target.public().to_peer_id().to_string());
        let mut abuse = AbuseGuard::new(limits());
        let mut service = IntroductionService::new(&conn, &auth, &mut abuse).unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let valid = envelope(requester.public().to_peer_id().to_string(), id.clone(), 4);
        let first = service
            .submit_with_outcome(&submit, "net", valid.clone(), 100, false)
            .response;
        let replay = service
            .submit_with_outcome(&submit, "net", valid, 100, false)
            .response;
        let mut unknown = envelope(
            requester.public().to_peer_id().to_string(),
            uuid::Uuid::new_v4().to_string(),
            4,
        );
        unknown.target = "@nobody@relay.test".into();
        unknown.work_challenge.target = unknown.target.clone();
        unknown.work_nonce = (0..)
            .find(|n| unknown.work_challenge.verify(*n, 100))
            .unwrap();
        let absent = service
            .submit_with_outcome(&submit, "net", unknown, 100, false)
            .response;
        assert_eq!(first.status, replay.status);
        assert_eq!(first.status, absent.status);
    }
    #[test]
    fn expiry_and_size_are_enforced_without_oracle() {
        let relay = Keypair::generate_ed25519();
        let requester = Keypair::generate_ed25519();
        let target = Keypair::generate_ed25519();
        let mut auth = AuthService::new("relay.test", "k1", relay);
        let submit = token(&mut auth, &requester, "introduce", 100);
        let read = token(&mut auth, &target, "introductions:read", 100);
        let conn = db(&target.public().to_peer_id().to_string());
        let mut abuse = AbuseGuard::new(limits());
        let mut service = IntroductionService::new(&conn, &auth, &mut abuse).unwrap();
        let mut expired = envelope(
            requester.public().to_peer_id().to_string(),
            uuid::Uuid::new_v4().to_string(),
            0,
        );
        expired.expires_at = 99;
        service.submit_with_outcome(&submit, "net", expired, 100, false);
        let mut large = envelope(
            requester.public().to_peer_id().to_string(),
            uuid::Uuid::new_v4().to_string(),
            0,
        );
        large.message_ciphertext = vec![0; MAX_CIPHERTEXT_BYTES + 1];
        service.submit_with_outcome(&submit, "net", large, 100, false);
        assert!(service.take(&read, 101, 10).unwrap().is_empty());
    }

    #[test]
    fn every_work_binding_dimension_rejects_with_the_same_external_response() {
        let relay = Keypair::generate_ed25519();
        let requester = Keypair::generate_ed25519();
        let target = Keypair::generate_ed25519();
        let mut auth = AuthService::new("relay.test", "k1", relay);
        let submit = token(&mut auth, &requester, "introduce", 100);
        let conn = db(&target.public().to_peer_id().to_string());
        let mut abuse = AbuseGuard::new(limits());
        let mut service = IntroductionService::new(&conn, &auth, &mut abuse).unwrap();
        let requester_peer = requester.public().to_peer_id().to_string();
        let baseline = envelope(requester_peer.clone(), uuid::Uuid::new_v4().to_string(), 0);
        let expected = AcceptedResponse::generic(baseline.request_id.clone());

        for mutation in 0..5 {
            let mut candidate = baseline.clone();
            match mutation {
                0 => candidate.work_challenge.requester = "other-peer".into(),
                1 => candidate.work_challenge.target = "@other@relay.test".into(),
                2 => candidate.work_challenge.action = "fetch".into(),
                3 => candidate.work_challenge.relay = "other-relay.test".into(),
                _ => candidate.work_challenge.audience = "other".into(),
            }
            let outcome = service.submit_with_outcome(&submit, "net", candidate, 100, false);
            assert_eq!(outcome.response, expected);
            assert_eq!(outcome.code, AdmissionCode::WorkBindingRejected);
        }
    }

    #[test]
    fn storage_failure_is_typed_internally_but_oracle_resistant_externally() {
        let relay = Keypair::generate_ed25519();
        let requester = Keypair::generate_ed25519();
        let target = Keypair::generate_ed25519();
        let mut auth = AuthService::new("relay.test", "k1", relay);
        let submit = token(&mut auth, &requester, "introduce", 100);
        let conn = db(&target.public().to_peer_id().to_string());
        let mut abuse = AbuseGuard::new(limits());
        let mut service = IntroductionService::new(&conn, &auth, &mut abuse).unwrap();
        service
            .conn
            .execute("DROP TABLE relay_name_claims", [])
            .unwrap();
        let candidate = envelope(
            requester.public().to_peer_id().to_string(),
            uuid::Uuid::new_v4().to_string(),
            0,
        );
        let expected = AcceptedResponse::generic(candidate.request_id.clone());

        let outcome = service.submit_with_outcome(&submit, "net", candidate, 100, false);
        assert_eq!(outcome.response, expected);
        assert_eq!(outcome.code, AdmissionCode::StorageFailure);
    }
}
