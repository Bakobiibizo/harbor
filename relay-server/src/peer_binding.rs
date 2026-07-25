//! Binding between Harbor Ed25519 signing keys and libp2p transport identities.

use libp2p::{identity, PeerId};

pub const REGISTRATION_MAX_AGE_SECONDS: i64 = 300;
pub const REGISTRATION_MAX_FUTURE_SKEW_SECONDS: i64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerKeyBindingError {
    InvalidPeerId,
    InvalidPublicKey,
    PeerIdMismatch,
    RegistrationStale,
    RegistrationFuture,
}

impl PeerKeyBindingError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidPeerId => "RELAY_PEER_ID_INVALID",
            Self::InvalidPublicKey => "RELAY_PEER_KEY_INVALID",
            Self::PeerIdMismatch => "RELAY_PEER_KEY_MISMATCH",
            Self::RegistrationStale => "RELAY_PEER_REGISTRATION_STALE",
            Self::RegistrationFuture => "RELAY_PEER_REGISTRATION_FUTURE",
        }
    }
}

pub fn peer_id_for_ed25519(public_key: &[u8]) -> Result<PeerId, PeerKeyBindingError> {
    let key = identity::ed25519::PublicKey::try_from_bytes(public_key)
        .map_err(|_| PeerKeyBindingError::InvalidPublicKey)?;
    Ok(PeerId::from_public_key(&identity::PublicKey::from(key)))
}

pub fn verify_peer_key_binding(
    claimed_peer_id: &str,
    public_key: &[u8],
) -> Result<PeerId, PeerKeyBindingError> {
    let claimed = claimed_peer_id
        .parse::<PeerId>()
        .map_err(|_| PeerKeyBindingError::InvalidPeerId)?;
    let derived = peer_id_for_ed25519(public_key)?;
    if claimed != derived {
        return Err(PeerKeyBindingError::PeerIdMismatch);
    }
    Ok(derived)
}

pub fn verify_registration_time(
    signed_at: i64,
    server_now: i64,
) -> Result<(), PeerKeyBindingError> {
    if signed_at < server_now.saturating_sub(REGISTRATION_MAX_AGE_SECONDS) {
        return Err(PeerKeyBindingError::RegistrationStale);
    }
    if signed_at > server_now.saturating_add(REGISTRATION_MAX_FUTURE_SKEW_SECONDS) {
        return Err(PeerKeyBindingError::RegistrationFuture);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;

    #[test]
    fn golden_ed25519_key_derives_the_transport_peer_id() {
        let signing = SigningKey::from_bytes(&[7; 32]);
        let derived = peer_id_for_ed25519(&signing.verifying_key().to_bytes()).unwrap();
        assert_eq!(
            derived.to_string(),
            "12D3KooWRawPbxPtP1eZaJpumGnyWX2DcUyd3RQnydr3eAto4Az7"
        );
        assert!(
            verify_peer_key_binding(&derived.to_string(), &signing.verifying_key().to_bytes())
                .is_ok()
        );
    }

    #[test]
    fn rejects_cross_peer_key_substitution_and_malformed_inputs() {
        let first = SigningKey::from_bytes(&[1; 32]);
        let second = SigningKey::from_bytes(&[2; 32]);
        let first_peer = peer_id_for_ed25519(&first.verifying_key().to_bytes()).unwrap();
        assert_eq!(
            verify_peer_key_binding(&first_peer.to_string(), &second.verifying_key().to_bytes()),
            Err(PeerKeyBindingError::PeerIdMismatch)
        );
        assert_eq!(
            verify_peer_key_binding("not-a-peer-id", &first.verifying_key().to_bytes()),
            Err(PeerKeyBindingError::InvalidPeerId)
        );
        assert_eq!(
            verify_peer_key_binding(&first_peer.to_string(), &[1, 2, 3]),
            Err(PeerKeyBindingError::InvalidPublicKey)
        );
    }

    #[test]
    fn registration_time_accepts_exact_boundaries_and_rejects_outside_them() {
        assert_eq!(verify_registration_time(700, 1_000), Ok(()));
        assert_eq!(verify_registration_time(1_030, 1_000), Ok(()));
        assert_eq!(
            verify_registration_time(699, 1_000),
            Err(PeerKeyBindingError::RegistrationStale)
        );
        assert_eq!(
            verify_registration_time(1_031, 1_000),
            Err(PeerKeyBindingError::RegistrationFuture)
        );
    }
}
