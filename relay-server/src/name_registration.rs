use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use libp2p::{identity, PeerId};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use thiserror::Error;

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
        || (r.issued_at - now).abs() > MAX_SKEW
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
    let existing:Option<(String,i64,String)>=tx.query_row("SELECT peer_id,sequence,status FROM relay_name_claims WHERE relay=? AND local_name=? ORDER BY sequence DESC LIMIT 1",params![relay,r.local_name],|x|Ok((x.get(0)?,x.get(1)?,x.get(2)?))).optional().map_err(|_|RegistrationError::Database)?;
    if let Some((peer, seq, status)) = existing {
        if status == "retired" || peer != r.peer_id {
            return Err(RegistrationError::Unavailable);
        }
        if r.sequence as i64 <= seq {
            return Err(RegistrationError::Replay);
        }
    }
    let mut claim = NameClaim {
        request: r.clone(),
        user_signature: signed.user_signature,
        status: "active".into(),
        not_before: now,
        not_after: now + CLAIM_LIFETIME,
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
            r.sequence as i64,
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
