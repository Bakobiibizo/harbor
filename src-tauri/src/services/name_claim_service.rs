use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use libp2p::{identity, PeerId};
use thiserror::Error;
use x25519_dalek::{PublicKey as X25519Public, StaticSecret};

use crate::{
    db::repositories::RelayNamesRepository,
    models::{domain, NameClaim, QualifiedRelayName, PROTOCOL_VERSION},
    services::signing::canonical_cbor,
};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ClaimVerificationError {
    #[error("claim encoding or canonical name is invalid")]
    InvalidEncoding,
    #[error("claim is outside its validity window")]
    Expired,
    #[error("claim is not active")]
    Inactive,
    #[error("relay key is not trusted")]
    UntrustedRelay,
    #[error("public key does not derive the claimed peer id")]
    PeerIdMismatch,
    #[error("user signature is invalid")]
    InvalidUserSignature,
    #[error("relay signature is invalid")]
    InvalidRelaySignature,
    #[error("claim sequence is stale")]
    Superseded,
    #[error("database error")]
    Database,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedNameClaim {
    pub qualified_name: QualifiedRelayName,
    pub peer_id: PeerId,
    pub sequence: u64,
    pub not_after: i64,
}

pub fn user_signing_bytes(claim: &NameClaim) -> Result<Vec<u8>, ClaimVerificationError> {
    canonical_cbor(&claim.request).map_err(|_| ClaimVerificationError::InvalidEncoding)
}
pub fn relay_signing_bytes(claim: &NameClaim) -> Result<Vec<u8>, ClaimVerificationError> {
    #[derive(serde::Serialize)]
    struct RelaySigned<'a> {
        request: &'a crate::models::NameClaimRequest,
        user_signature: &'a [u8],
        status: &'a str,
        not_before: i64,
        not_after: i64,
        relay_key_id: &'a str,
    }
    canonical_cbor(&RelaySigned {
        request: &claim.request,
        user_signature: &claim.user_signature,
        status: &claim.status,
        not_before: claim.not_before,
        not_after: claim.not_after,
        relay_key_id: &claim.relay_key_id,
    })
    .map_err(|_| ClaimVerificationError::InvalidEncoding)
}

pub fn verify_and_cache(
    repo: &RelayNamesRepository<'_>,
    claim: &NameClaim,
    now: i64,
) -> Result<VerifiedNameClaim, ClaimVerificationError> {
    if claim.request.domain != domain::NAME_CLAIM_REQUEST
        || claim.request.version != PROTOCOL_VERSION
        || claim.request.sequence == 0
        || claim.request.nonce.len() < 16
    {
        return Err(ClaimVerificationError::InvalidEncoding);
    }
    let x_raw: [u8; 32] = claim
        .request
        .x25519_public_key
        .as_slice()
        .try_into()
        .map_err(|_| ClaimVerificationError::InvalidEncoding)?;
    if !StaticSecret::from([1u8; 32])
        .diffie_hellman(&X25519Public::from(x_raw))
        .was_contributory()
    {
        return Err(ClaimVerificationError::InvalidEncoding);
    }
    let qualified: QualifiedRelayName =
        format!("@{}@{}", claim.request.local_name, claim.request.relay)
            .parse()
            .map_err(|_| ClaimVerificationError::InvalidEncoding)?;
    if claim.status != "active" {
        return Err(ClaimVerificationError::Inactive);
    }
    if claim.not_before > now
        || claim.not_after < now
        || claim.not_after <= claim.not_before
        || claim.request.issued_at > now
    {
        return Err(ClaimVerificationError::Expired);
    }
    let raw: [u8; 32] = claim
        .request
        .ed25519_public_key
        .as_slice()
        .try_into()
        .map_err(|_| ClaimVerificationError::InvalidEncoding)?;
    let user_key =
        VerifyingKey::from_bytes(&raw).map_err(|_| ClaimVerificationError::InvalidEncoding)?;
    let lp_key = identity::ed25519::PublicKey::try_from_bytes(&raw)
        .map_err(|_| ClaimVerificationError::InvalidEncoding)?;
    let peer = PeerId::from_public_key(&identity::PublicKey::from(lp_key));
    if peer.to_string() != claim.request.peer_id {
        return Err(ClaimVerificationError::PeerIdMismatch);
    }
    let user_sig = Signature::from_slice(&claim.user_signature)
        .map_err(|_| ClaimVerificationError::InvalidUserSignature)?;
    user_key
        .verify(&user_signing_bytes(claim)?, &user_sig)
        .map_err(|_| ClaimVerificationError::InvalidUserSignature)?;
    let relay_bytes = repo
        .trusted_key(&claim.request.relay, &claim.relay_key_id, now)
        .map_err(|_| ClaimVerificationError::Database)?
        .ok_or(ClaimVerificationError::UntrustedRelay)?;
    let relay_raw: [u8; 32] = relay_bytes
        .as_slice()
        .try_into()
        .map_err(|_| ClaimVerificationError::UntrustedRelay)?;
    let relay_key =
        VerifyingKey::from_bytes(&relay_raw).map_err(|_| ClaimVerificationError::UntrustedRelay)?;
    let relay_sig = Signature::from_slice(&claim.relay_signature)
        .map_err(|_| ClaimVerificationError::InvalidRelaySignature)?;
    relay_key
        .verify(&relay_signing_bytes(claim)?, &relay_sig)
        .map_err(|_| ClaimVerificationError::InvalidRelaySignature)?;
    let mut encoded = Vec::new();
    ciborium::ser::into_writer(claim, &mut encoded)
        .map_err(|_| ClaimVerificationError::InvalidEncoding)?;
    repo.cache_verified(
        &qualified.to_string(),
        qualified.local().as_str(),
        qualified.relay().as_str(),
        &claim.request.peer_id,
        claim.request.sequence,
        &encoded,
        claim.not_before,
        claim.not_after,
        &claim.relay_key_id,
        now,
    )
    .map_err(|e| {
        if matches!(e, rusqlite::Error::InvalidQuery) {
            ClaimVerificationError::Superseded
        } else {
            ClaimVerificationError::Database
        }
    })?;
    Ok(VerifiedNameClaim {
        qualified_name: qualified,
        peer_id: peer,
        sequence: claim.request.sequence,
        not_after: claim.not_after,
    })
}
