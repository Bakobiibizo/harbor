use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use libp2p::{identity, PeerId};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use x25519_dalek::{PublicKey as X25519Public, StaticSecret};

const REQUEST_DOMAIN: &str = "harbor/name-claim-request/1";
const MAX_SKEW: i64 = 300;
const CLAIM_LIFETIME: i64 = 365 * 24 * 60 * 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NameClaimRequest {
    pub domain: String,
    pub version: u16,
    pub local_name: String,
    pub relay: String,
    pub peer_id: String,
    pub ed25519_public_key: Vec<u8>,
    pub x25519_public_key: Vec<u8>,
    pub sequence: u64,
    pub issued_at: i64,
    pub nonce: Vec<u8>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedNameClaimRequest {
    pub request: NameClaimRequest,
    pub user_signature: Vec<u8>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NameClaim {
    pub request: NameClaimRequest,
    pub user_signature: Vec<u8>,
    pub status: String,
    pub not_before: i64,
    pub not_after: i64,
    pub relay_key_id: String,
    pub relay_signature: Vec<u8>,
}
#[derive(Debug, Error)]
pub enum RegistrationError {
    #[error("invalid request")]
    Invalid,
    #[error("invalid signature")]
    Signature,
    #[error("name unavailable")]
    Unavailable,
    #[error("stale or replayed request")]
    Replay,
    #[error("database error")]
    Database,
    #[error("integer out of range")]
    IntegerRange,
}

fn cbor<T: Serialize>(v: &T) -> Result<Vec<u8>, RegistrationError> {
    let mut b = Vec::new();
    ciborium::ser::into_writer(v, &mut b).map_err(|_| RegistrationError::Invalid)?;
    Ok(b)
}
fn valid_name(v: &str) -> bool {
    v.len() >= 3
        && v.len() <= 32
        && !v.starts_with('-')
        && !v.ends_with('-')
        && !v.contains("--")
        && v.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}
fn valid_relay(v: &str) -> bool {
    v.is_ascii()
        && v == v.to_ascii_lowercase()
        && !v.contains([':', '/', '@'])
        && v.split('.').count() > 1
        && v.split('.').all(|l| {
            !l.is_empty()
                && l.len() <= 63
                && !l.starts_with('-')
                && !l.ends_with('-')
                && l.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}

pub fn register(
    conn: &mut Connection,
    relay: &str,
    key_id: &str,
    relay_key: &SigningKey,
    signed: SignedNameClaimRequest,
    now: i64,
) -> Result<NameClaim, RegistrationError> {
    let r = &signed.request;
    if r.domain != REQUEST_DOMAIN
        || r.version != 1
        || r.relay != relay
        || !valid_name(&r.local_name)
        || !valid_relay(&r.relay)
        || r.sequence == 0
        || r.nonce.len() < 16
        || r.issued_at.abs_diff(now) > MAX_SKEW as u64
    {
        return Err(RegistrationError::Invalid);
    }
    let sequence = i64::try_from(r.sequence).map_err(|_| RegistrationError::IntegerRange)?;
    let not_after = now
        .checked_add(CLAIM_LIFETIME)
        .ok_or(RegistrationError::IntegerRange)?;
    let x_raw: [u8; 32] = r
        .x25519_public_key
        .as_slice()
        .try_into()
        .map_err(|_| RegistrationError::Invalid)?;
    if !StaticSecret::from([1u8; 32])
        .diffie_hellman(&X25519Public::from(x_raw))
        .was_contributory()
    {
        return Err(RegistrationError::Invalid);
    }
    let raw: [u8; 32] = r
        .ed25519_public_key
        .as_slice()
        .try_into()
        .map_err(|_| RegistrationError::Invalid)?;
    let user = VerifyingKey::from_bytes(&raw).map_err(|_| RegistrationError::Invalid)?;
    let lp = identity::ed25519::PublicKey::try_from_bytes(&raw)
        .map_err(|_| RegistrationError::Invalid)?;
    if PeerId::from_public_key(&identity::PublicKey::from(lp)).to_string() != r.peer_id {
        return Err(RegistrationError::Invalid);
    }
    let sig =
        Signature::from_slice(&signed.user_signature).map_err(|_| RegistrationError::Signature)?;
    user.verify(&cbor(r)?, &sig)
        .map_err(|_| RegistrationError::Signature)?;
    let tx = conn
        .transaction()
        .map_err(|_| RegistrationError::Database)?;
    if tx
        .query_row(
            "SELECT 1 FROM relay_name_nonces WHERE peer_id=? AND nonce=?",
            params![r.peer_id, r.nonce],
            |x| x.get::<_, i32>(0),
        )
        .optional()
        .map_err(|_| RegistrationError::Database)?
        .is_some()
    {
        return Err(RegistrationError::Replay);
    }
    let existing: Option<(String, i64, String, Vec<u8>)> = tx
        .query_row(
            "SELECT peer_id,sequence,status,claim_cbor
             FROM relay_name_claims
             WHERE relay=? AND local_name=?
             ORDER BY sequence DESC LIMIT 1",
            params![relay, r.local_name],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(|_| RegistrationError::Database)?;
    if let Some((peer, seq, status, encoded)) = existing {
        let stored_sequence = u64::try_from(seq).map_err(|_| RegistrationError::IntegerRange)?;
        if status == "retired" || peer != r.peer_id {
            return Err(RegistrationError::Unavailable);
        }
        if r.sequence == stored_sequence {
            let claim: NameClaim = ciborium::de::from_reader(encoded.as_slice())
                .map_err(|_| RegistrationError::Database)?;
            if claim.status != "active"
                || claim.not_after < now
                || claim.relay_key_id != key_id
                || claim.request.local_name != r.local_name
                || claim.request.relay != r.relay
                || claim.request.peer_id != r.peer_id
                || claim.request.ed25519_public_key != r.ed25519_public_key
                || claim.request.x25519_public_key != r.x25519_public_key
            {
                return Err(RegistrationError::Replay);
            }
            tx.execute(
                "INSERT INTO relay_name_nonces VALUES(?,?,?)",
                params![r.peer_id, r.nonce, now],
            )
            .map_err(|_| RegistrationError::Replay)?;
            tx.commit().map_err(|_| RegistrationError::Database)?;
            return Ok(claim);
        }
        if r.sequence < stored_sequence {
            return Err(RegistrationError::Replay);
        }
    }
    let mut claim = NameClaim {
        request: r.clone(),
        user_signature: signed.user_signature,
        status: "active".into(),
        not_before: now,
        not_after,
        relay_key_id: key_id.into(),
        relay_signature: vec![],
    };
    #[derive(Serialize)]
    struct RS<'a> {
        request: &'a NameClaimRequest,
        user_signature: &'a [u8],
        status: &'a str,
        not_before: i64,
        not_after: i64,
        relay_key_id: &'a str,
    }
    let bytes = cbor(&RS {
        request: &claim.request,
        user_signature: &claim.user_signature,
        status: &claim.status,
        not_before: claim.not_before,
        not_after: claim.not_after,
        relay_key_id: &claim.relay_key_id,
    })?;
    claim.relay_signature = relay_key.sign(&bytes).to_bytes().to_vec();
    let encoded = cbor(&claim)?;
    tx.execute("UPDATE relay_name_claims SET status='retired',retired_at=? WHERE relay=? AND local_name=? AND status='active'",params![now,relay,r.local_name]).map_err(|_|RegistrationError::Database)?;
    tx.execute(
        "INSERT INTO relay_name_claims VALUES(?,?,?,?,?,?,?,?, 'active',?,NULL)",
        params![
            r.local_name,
            relay,
            r.peer_id,
            sequence,
            encoded,
            claim.not_before,
            claim.not_after,
            key_id,
            now
        ],
    )
    .map_err(|e| {
        if matches!(e, rusqlite::Error::SqliteFailure(_, _)) {
            RegistrationError::Unavailable
        } else {
            RegistrationError::Database
        }
    })?;
    tx.execute(
        "INSERT INTO relay_name_nonces VALUES(?,?,?)",
        params![r.peer_id, r.nonce, now],
    )
    .map_err(|_| RegistrationError::Replay)?;
    tx.commit().map_err(|_| RegistrationError::Database)?;
    Ok(claim)
}

#[cfg(test)]
mod acceptance_tests {
    use super::*;
    use crate::db::RelayDatabase;
    use ed25519_dalek::Signer;
    use std::sync::{Arc, Barrier};

    fn signed(key: &SigningKey, name: &str, nonce: u8) -> SignedNameClaimRequest {
        signed_with(key, name, nonce, 1, 100)
    }

    fn signed_with(
        key: &SigningKey,
        name: &str,
        nonce: u8,
        sequence: u64,
        issued_at: i64,
    ) -> SignedNameClaimRequest {
        let lp =
            identity::ed25519::PublicKey::try_from_bytes(&key.verifying_key().to_bytes()).unwrap();
        let request = NameClaimRequest {
            domain: REQUEST_DOMAIN.into(),
            version: 1,
            local_name: name.into(),
            relay: "relay.test".into(),
            peer_id: PeerId::from_public_key(&identity::PublicKey::from(lp)).to_string(),
            ed25519_public_key: key.verifying_key().to_bytes().to_vec(),
            x25519_public_key: X25519Public::from(&StaticSecret::from([nonce.max(1); 32]))
                .to_bytes()
                .to_vec(),
            sequence,
            issued_at,
            nonce: vec![nonce; 32],
        };
        let user_signature = key.sign(&cbor(&request).unwrap()).to_bytes().to_vec();
        SignedNameClaimRequest {
            request,
            user_signature,
        }
    }
    fn path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("harbor-name-{}.sqlite", uuid::Uuid::new_v4()))
    }

    #[test]
    fn concurrent_collision_has_exactly_one_winner_and_persists_after_reopen() {
        let path = path();
        RelayDatabase::open(path.to_str().unwrap()).unwrap();
        let relay_key = SigningKey::from_bytes(&[9; 32]);
        let a = SigningKey::from_bytes(&[1; 32]);
        let b = SigningKey::from_bytes(&[2; 32]);
        let barrier = Arc::new(Barrier::new(2));
        let mut joins = Vec::new();
        for request in [signed(&a, "alice", 1), signed(&b, "alice", 2)] {
            let p = path.clone();
            let gate = barrier.clone();
            let relay = relay_key.clone();
            joins.push(std::thread::spawn(move || {
                let mut conn = Connection::open(p).unwrap();
                conn.busy_timeout(std::time::Duration::from_secs(2))
                    .unwrap();
                gate.wait();
                register(&mut conn, "relay.test", "k1", &relay, request, 100)
            }));
        }
        let results: Vec<_> = joins.into_iter().map(|j| j.join().unwrap()).collect();
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
        drop(results);
        let conn = Connection::open(&path).unwrap();
        let active:i64=conn.query_row("SELECT COUNT(*) FROM relay_name_claims WHERE relay='relay.test' AND local_name='alice' AND status='active'",[],|r|r.get(0)).unwrap();
        assert_eq!(active, 1);
        drop(conn);
        let reopened = Connection::open(&path).unwrap();
        let peer:String=reopened.query_row("SELECT peer_id FROM relay_name_claims WHERE relay='relay.test' AND local_name='alice' AND status='active'",[],|r|r.get(0)).unwrap();
        assert!(!peer.is_empty());
        drop(reopened);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn registration_nonce_replay_leaves_assignment_unchanged() {
        let path = path();
        RelayDatabase::open(path.to_str().unwrap()).unwrap();
        let relay = SigningKey::from_bytes(&[9; 32]);
        let user = SigningKey::from_bytes(&[4; 32]);
        let request = signed(&user, "alice", 7);
        let expected = request.request.peer_id.clone();
        let mut conn = Connection::open(&path).unwrap();
        assert!(register(&mut conn, "relay.test", "k1", &relay, request.clone(), 100).is_ok());
        assert!(matches!(
            register(&mut conn, "relay.test", "k1", &relay, request, 100),
            Err(RegistrationError::Replay)
        ));
        let rows:i64=conn.query_row("SELECT COUNT(*) FROM relay_name_claims WHERE relay='relay.test' AND local_name='alice'",[],|r|r.get(0)).unwrap();
        let peer:String=conn.query_row("SELECT peer_id FROM relay_name_claims WHERE relay='relay.test' AND local_name='alice' AND status='active'",[],|r|r.get(0)).unwrap();
        assert_eq!(rows, 1);
        assert_eq!(peer, expected);
        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn same_owner_can_recover_a_persisted_claim_after_losing_the_response() {
        let database = RelayDatabase::open(":memory:").unwrap();
        let relay = SigningKey::from_bytes(&[9; 32]);
        let user = SigningKey::from_bytes(&[4; 32]);

        database.with_connection(|connection| {
            let first_request = signed(&user, "alice", 1);
            let mut retry_request = first_request.clone();
            retry_request.request.nonce = vec![2; 32];
            retry_request.user_signature = user
                .sign(&cbor(&retry_request.request).unwrap())
                .to_bytes()
                .to_vec();
            let original =
                register(connection, "relay.test", "k1", &relay, first_request, 100).unwrap();

            // A fresh, signed retry represents a client that never received or
            // cached the first successful response. The relay must return the
            // persisted claim to its owner instead of stranding the identity.
            let recovered =
                register(connection, "relay.test", "k1", &relay, retry_request, 100).unwrap();

            assert_eq!(cbor(&recovered).unwrap(), cbor(&original).unwrap());
            let rows: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM relay_name_claims
                     WHERE relay='relay.test' AND local_name='alice'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(rows, 1);
        });
    }

    #[test]
    fn sequence_and_expiry_integer_boundaries_are_typed() {
        let database = RelayDatabase::open(":memory:").unwrap();
        let relay = SigningKey::from_bytes(&[9; 32]);
        let user = SigningKey::from_bytes(&[4; 32]);

        database.with_connection(|connection| {
            assert!(matches!(
                register(
                    connection,
                    "relay.test",
                    "k1",
                    &relay,
                    signed_with(&user, "zero", 1, 0, 100),
                    100,
                ),
                Err(RegistrationError::Invalid)
            ));
            assert!(register(
                connection,
                "relay.test",
                "k1",
                &relay,
                signed_with(&user, "max", 2, i64::MAX as u64, 100),
                100,
            )
            .is_ok());
            assert!(matches!(
                register(
                    connection,
                    "relay.test",
                    "k1",
                    &relay,
                    signed_with(&user, "over", 3, i64::MAX as u64 + 1, 100),
                    100,
                ),
                Err(RegistrationError::IntegerRange)
            ));
            assert!(matches!(
                register(
                    connection,
                    "relay.test",
                    "k1",
                    &relay,
                    signed_with(&user, "u64max", 4, u64::MAX, 100),
                    100,
                ),
                Err(RegistrationError::IntegerRange)
            ));
            assert!(matches!(
                register(
                    connection,
                    "relay.test",
                    "k1",
                    &relay,
                    signed_with(&user, "expiry", 5, 1, i64::MAX),
                    i64::MAX,
                ),
                Err(RegistrationError::IntegerRange)
            ));
        });
    }

    #[test]
    fn negative_stored_sequence_is_rejected_instead_of_wrapping() {
        let database = RelayDatabase::open(":memory:").unwrap();
        let relay = SigningKey::from_bytes(&[9; 32]);
        let user = SigningKey::from_bytes(&[4; 32]);
        let request = signed_with(&user, "negative", 8, 1, 100);
        database.with_connection(|connection| {
            connection.execute_batch("PRAGMA ignore_check_constraints = ON").unwrap();
            connection
                .execute(
                    "INSERT INTO relay_name_claims(local_name,relay,peer_id,sequence,claim_cbor,not_before,not_after,relay_key_id,status,created_at) VALUES(?,?,?,?,X'01',100,200,'k1','active',100)",
                    params!["negative", "relay.test", request.request.peer_id, -1i64],
                )
                .unwrap();
            assert!(matches!(
                register(connection, "relay.test", "k1", &relay, request, 100),
                Err(RegistrationError::IntegerRange)
            ));
        });
    }
}
