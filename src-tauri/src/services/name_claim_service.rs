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

/// Load and cryptographically verify the active relay name for a peer.
///
/// `Ok(None)` means that no active claim exists. Repository, decoding, trust,
/// signature, and peer-binding failures remain errors so presentation callers
/// cannot silently turn a broken trust lookup into an ordinary unverified name.
pub fn verified_name_claim(
    repo: &RelayNamesRepository<'_>,
    peer_id: &str,
    now: i64,
) -> Result<Option<(NameClaim, VerifiedNameClaim)>, ClaimVerificationError> {
    let Some(encoded) = repo
        .active_for_peer(peer_id, now)
        .map_err(|_| ClaimVerificationError::Database)?
    else {
        return Ok(None);
    };
    let claim: NameClaim = ciborium::de::from_reader(encoded.as_slice())
        .map_err(|_| ClaimVerificationError::InvalidEncoding)?;
    let verified = verify_and_cache(repo, &claim, now)?;
    Ok(Some((claim, verified)))
}

pub fn verified_qualified_name(
    repo: &RelayNamesRepository<'_>,
    peer_id: &str,
    now: i64,
) -> Result<Option<String>, ClaimVerificationError> {
    Ok(verified_name_claim(repo, peer_id, now)?
        .map(|(_, verified)| verified.qualified_name.to_string()))
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

#[cfg(test)]
mod adversarial_tests {
    use super::*;
    use crate::{
        db::Database,
        models::{NameClaim, NameClaimRequest},
    };
    use ed25519_dalek::{Signer, SigningKey};
    fn claim(user: &SigningKey, relay: &SigningKey) -> NameClaim {
        let raw = user.verifying_key().to_bytes();
        let lp = identity::ed25519::PublicKey::try_from_bytes(&raw).unwrap();
        let request = NameClaimRequest {
            domain: domain::NAME_CLAIM_REQUEST.into(),
            version: PROTOCOL_VERSION,
            local_name: "alice".into(),
            relay: "relay.test".into(),
            peer_id: PeerId::from_public_key(&identity::PublicKey::from(lp)).to_string(),
            ed25519_public_key: raw.to_vec(),
            x25519_public_key: x25519_dalek::PublicKey::from(&x25519_dalek::StaticSecret::from(
                [8; 32],
            ))
            .to_bytes()
            .to_vec(),
            sequence: 1,
            issued_at: 100,
            nonce: vec![1; 16],
        };
        let mut c = NameClaim {
            request,
            user_signature: vec![],
            status: "active".into(),
            not_before: 100,
            not_after: 300,
            relay_key_id: "k1".into(),
            relay_signature: vec![],
        };
        c.user_signature = user
            .sign(&user_signing_bytes(&c).unwrap())
            .to_bytes()
            .to_vec();
        c.relay_signature = relay
            .sign(&relay_signing_bytes(&c).unwrap())
            .to_bytes()
            .to_vec();
        c
    }
    #[test]
    fn every_forged_claim_field_is_rejected() {
        let user = SigningKey::from_bytes(&[3; 32]);
        let relay = SigningKey::from_bytes(&[4; 32]);
        let original = claim(&user, &relay);
        for mutation in 0..8 {
            let db = Database::in_memory().unwrap();
            let repo = RelayNamesRepository::new(&db);
            repo.pin_key(
                "relay.test",
                "k1",
                &relay.verifying_key().to_bytes(),
                0,
                Some(400),
            )
            .unwrap();
            let mut c = original.clone();
            match mutation {
                0 => c.request.peer_id = "12D3KooWForged".into(),
                1 => c.request.ed25519_public_key[0] ^= 1,
                2 => c.user_signature[0] ^= 1,
                3 => c.relay_signature[0] ^= 1,
                4 => c.request.relay = "other.test".into(),
                5 => c.not_after = 99,
                6 => c.request.sequence = 0,
                _ => c.relay_key_id = "unknown".into(),
            }
            assert!(
                verify_and_cache(&repo, &c, 200).is_err(),
                "mutation {mutation}"
            );
            let count: i64 = db
                .with_connection(|x| {
                    x.query_row("SELECT COUNT(*) FROM relay_name_claims", [], |r| r.get(0))
                })
                .unwrap();
            assert_eq!(count, 0);
        }
    }

    #[test]
    fn verified_name_lookup_distinguishes_absence_from_repository_failure() {
        let db = Database::in_memory().unwrap();
        let repo = RelayNamesRepository::new(&db);

        assert_eq!(
            verified_qualified_name(&repo, "missing-peer", 200),
            Ok(None)
        );

        db.with_connection(|connection| {
            connection.execute("DROP TABLE relay_name_claims", [])?;
            Ok(())
        })
        .unwrap();
        assert_eq!(
            verified_qualified_name(&repo, "missing-peer", 200),
            Err(ClaimVerificationError::Database)
        );
    }

    #[test]
    fn verified_name_lookup_rejects_a_tampered_cached_claim() {
        let user = SigningKey::from_bytes(&[3; 32]);
        let relay = SigningKey::from_bytes(&[4; 32]);
        let original = claim(&user, &relay);
        let db = Database::in_memory().unwrap();
        let repo = RelayNamesRepository::new(&db);
        repo.pin_key(
            "relay.test",
            "k1",
            &relay.verifying_key().to_bytes(),
            0,
            Some(400),
        )
        .unwrap();
        verify_and_cache(&repo, &original, 200).unwrap();
        assert_eq!(
            verified_qualified_name(&repo, &original.request.peer_id, 200).unwrap(),
            Some("@alice@relay.test".into())
        );

        let mut tampered = original.clone();
        tampered.request.local_name = "mallory".into();
        let mut encoded = Vec::new();
        ciborium::ser::into_writer(&tampered, &mut encoded).unwrap();
        db.with_connection(|connection| {
            connection.execute(
                "UPDATE relay_name_claims SET claim_cbor=? WHERE peer_id=?",
                rusqlite::params![encoded, original.request.peer_id],
            )?;
            Ok(())
        })
        .unwrap();

        assert_eq!(
            verified_qualified_name(&repo, &original.request.peer_id, 200),
            Err(ClaimVerificationError::InvalidUserSignature)
        );
    }
}
