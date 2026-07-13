use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use thiserror::Error;

use crate::{
    db::repositories::RelayNamesRepository,
    models::{domain, RelayHostname, SignedRelayKeyRotation, PROTOCOL_VERSION},
    services::signing::canonical_cbor,
};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RelayKeyRotationError {
    #[error("relay-key rotation record is invalid")]
    Invalid,
    #[error("the previous relay key is not trusted")]
    Untrusted,
    #[error("relay-key rotation signature is invalid")]
    Signature,
    #[error("relay-key rotation is stale or discontinuous")]
    Rollback,
    #[error("database error")]
    Database,
}

pub fn apply_signed_rotation(
    repo: &RelayNamesRepository<'_>,
    signed: &SignedRelayKeyRotation,
    now: i64,
) -> Result<bool, RelayKeyRotationError> {
    let rotation = &signed.rotation;
    if rotation.domain != domain::RELAY_KEY_ROTATION
        || rotation.version != PROTOCOL_VERSION
        || RelayHostname::parse(&rotation.relay).is_err()
        || rotation.previous_key_id.is_empty()
        || rotation.next_key_id.is_empty()
        || rotation.previous_key_id == rotation.next_key_id
        || rotation.next_public_key.len() != 32
        || rotation.sequence == 0
        || rotation.issued_at > now
        || rotation.not_before > now
        || rotation.not_after <= rotation.not_before
        || now > rotation.not_after
        || rotation
            .compromise_from
            .is_some_and(|value| value > now || value < 0)
    {
        return Err(RelayKeyRotationError::Invalid);
    }
    let previous = repo
        .active_pin(&rotation.relay, &rotation.previous_key_id, now)
        .map_err(|_| RelayKeyRotationError::Database)?
        .ok_or(RelayKeyRotationError::Untrusted)?;
    if rotation.sequence != previous.sequence + 1 || rotation.not_before < previous.not_before {
        return Err(RelayKeyRotationError::Rollback);
    }
    let previous_raw: [u8; 32] = previous
        .public_key
        .as_slice()
        .try_into()
        .map_err(|_| RelayKeyRotationError::Untrusted)?;
    let previous_key =
        VerifyingKey::from_bytes(&previous_raw).map_err(|_| RelayKeyRotationError::Untrusted)?;
    let signature = Signature::from_slice(&signed.previous_key_signature)
        .map_err(|_| RelayKeyRotationError::Signature)?;
    let bytes = canonical_cbor(rotation).map_err(|_| RelayKeyRotationError::Invalid)?;
    previous_key
        .verify(&bytes, &signature)
        .map_err(|_| RelayKeyRotationError::Signature)?;
    let _: VerifyingKey = VerifyingKey::from_bytes(
        &rotation
            .next_public_key
            .as_slice()
            .try_into()
            .map_err(|_| RelayKeyRotationError::Invalid)?,
    )
    .map_err(|_| RelayKeyRotationError::Invalid)?;
    let encoded = canonical_cbor(signed).map_err(|_| RelayKeyRotationError::Invalid)?;
    let applied = repo
        .apply_verified_rotation(rotation, &encoded, now)
        .map_err(|_| RelayKeyRotationError::Database)?;
    if !applied {
        return Err(RelayKeyRotationError::Rollback);
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::Database, models::RelayKeyRotation};
    use ed25519_dalek::{Signer, SigningKey};

    fn signed(old: &SigningKey, next: &SigningKey) -> SignedRelayKeyRotation {
        let rotation = RelayKeyRotation {
            domain: domain::RELAY_KEY_ROTATION.into(),
            version: PROTOCOL_VERSION,
            relay: "relay.test".into(),
            previous_key_id: "key-1".into(),
            next_key_id: "key-2".into(),
            next_public_key: next.verifying_key().to_bytes().to_vec(),
            not_before: 100,
            not_after: 1_000,
            issued_at: 100,
            sequence: 1,
            compromise_from: None,
        };
        SignedRelayKeyRotation {
            previous_key_signature: old
                .sign(&canonical_cbor(&rotation).unwrap())
                .to_bytes()
                .to_vec(),
            rotation,
        }
    }

    #[test]
    fn signed_rotation_persists_and_blocks_rollback_or_silent_replacement() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("harbor.db");
        let old = SigningKey::from_bytes(&[21; 32]);
        let next = SigningKey::from_bytes(&[22; 32]);
        let record = signed(&old, &next);

        {
            let db = Database::new(path.clone()).unwrap();
            let repo = RelayNamesRepository::new(&db);
            repo.pin_key(
                "relay.test",
                "key-1",
                &old.verifying_key().to_bytes(),
                0,
                Some(1_000),
            )
            .unwrap();
            assert!(repo
                .pin_key(
                    "relay.test",
                    "key-2",
                    &next.verifying_key().to_bytes(),
                    0,
                    Some(1_000),
                )
                .is_err());
            assert!(apply_signed_rotation(&repo, &record, 100).unwrap());
            assert!(matches!(
                apply_signed_rotation(&repo, &record, 101),
                Err(RelayKeyRotationError::Untrusted | RelayKeyRotationError::Rollback)
            ));
        }

        let reopened = Database::new(path).unwrap();
        let repo = RelayNamesRepository::new(&reopened);
        assert_eq!(repo.trusted_key("relay.test", "key-1", 200).unwrap(), None);
        assert_eq!(
            repo.trusted_key("relay.test", "key-2", 200).unwrap(),
            Some(next.verifying_key().to_bytes().to_vec())
        );
    }

    #[test]
    fn rejects_unknown_tampered_and_expired_rotation_records() {
        let db = Database::in_memory().unwrap();
        let repo = RelayNamesRepository::new(&db);
        let old = SigningKey::from_bytes(&[31; 32]);
        let next = SigningKey::from_bytes(&[32; 32]);
        let attacker = SigningKey::from_bytes(&[33; 32]);
        let record = signed(&old, &next);
        assert_eq!(
            apply_signed_rotation(&repo, &record, 100),
            Err(RelayKeyRotationError::Untrusted)
        );
        repo.pin_key(
            "relay.test",
            "key-1",
            &old.verifying_key().to_bytes(),
            0,
            Some(1_000),
        )
        .unwrap();
        let mut forged = record.clone();
        forged.previous_key_signature = attacker
            .sign(&canonical_cbor(&forged.rotation).unwrap())
            .to_bytes()
            .to_vec();
        assert_eq!(
            apply_signed_rotation(&repo, &forged, 100),
            Err(RelayKeyRotationError::Signature)
        );
        let mut expired = record;
        expired.rotation.not_after = 99;
        assert_eq!(
            apply_signed_rotation(&repo, &expired, 100),
            Err(RelayKeyRotationError::Invalid)
        );
    }
}
