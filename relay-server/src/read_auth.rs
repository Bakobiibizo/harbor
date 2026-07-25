//! Transport binding, freshness, and replay protection for signed relay reads.

use libp2p::PeerId;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

pub const READ_MAX_AGE_SECONDS: i64 = 300;
pub const READ_MAX_FUTURE_SKEW_SECONDS: i64 = 30;
pub const DEFAULT_REPLAY_CAPACITY: usize = 8_192;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayReadAuthError {
    RequesterMismatch,
    Stale,
    Future,
    Replay,
    ReplayCapacity,
}

impl RelayReadAuthError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::RequesterMismatch => "RELAY_READ_REQUESTER_MISMATCH",
            Self::Stale => "RELAY_READ_REQUEST_STALE",
            Self::Future => "RELAY_READ_REQUEST_FUTURE",
            Self::Replay => "RELAY_READ_REQUEST_REPLAY",
            Self::ReplayCapacity => "RELAY_READ_REPLAY_CAPACITY",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReadReplayToken([u8; 32]);

/// Process-wide replay state shared by every Noise-authenticated connection.
/// Entries are retained for the full signed-request validity interval.
pub struct RelayReadGuard {
    seen: HashMap<ReadReplayToken, i64>,
    capacity: usize,
}

impl Default for RelayReadGuard {
    fn default() -> Self {
        Self::new(DEFAULT_REPLAY_CAPACITY)
    }
}

impl RelayReadGuard {
    pub fn new(capacity: usize) -> Self {
        Self {
            seen: HashMap::new(),
            capacity,
        }
    }

    pub fn authorize(
        &mut self,
        transport_peer: &PeerId,
        requester_peer_id: &str,
        signed_at: i64,
        signature: &[u8],
        server_now: i64,
    ) -> Result<ReadReplayToken, RelayReadAuthError> {
        if requester_peer_id != transport_peer.to_string() {
            return Err(RelayReadAuthError::RequesterMismatch);
        }
        if signed_at < server_now.saturating_sub(READ_MAX_AGE_SECONDS) {
            return Err(RelayReadAuthError::Stale);
        }
        if signed_at > server_now.saturating_add(READ_MAX_FUTURE_SKEW_SECONDS) {
            return Err(RelayReadAuthError::Future);
        }

        self.seen
            .retain(|_, valid_through| *valid_through >= server_now);
        let token = fingerprint(transport_peer, requester_peer_id, signed_at, signature);
        if self.seen.contains_key(&token) {
            return Err(RelayReadAuthError::Replay);
        }
        if self.seen.len() >= self.capacity {
            return Err(RelayReadAuthError::ReplayCapacity);
        }

        self.seen
            .insert(token, signed_at.saturating_add(READ_MAX_AGE_SECONDS));
        Ok(token)
    }

    /// Authentication or authorization failed after freshness validation, so
    /// the untrusted fingerprint must not consume replay capacity.
    pub fn discard(&mut self, token: ReadReplayToken) {
        self.seen.remove(&token);
    }
}

fn fingerprint(
    transport_peer: &PeerId,
    requester_peer_id: &str,
    signed_at: i64,
    signature: &[u8],
) -> ReadReplayToken {
    let mut digest = Sha256::new();
    digest.update(b"harbor/relay-read-replay/1");
    digest.update((transport_peer.to_bytes().len() as u32).to_be_bytes());
    digest.update(transport_peer.to_bytes());
    digest.update((requester_peer_id.len() as u32).to_be_bytes());
    digest.update(requester_peer_id.as_bytes());
    digest.update(signed_at.to_be_bytes());
    digest.update((signature.len() as u32).to_be_bytes());
    digest.update(signature);
    ReadReplayToken(digest.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::identity::Keypair;

    fn peer() -> PeerId {
        Keypair::generate_ed25519().public().to_peer_id()
    }

    #[test]
    fn accepts_freshness_boundaries_and_rejects_expired_or_future_requests() {
        let peer = peer();
        let claimed = peer.to_string();
        let mut guard = RelayReadGuard::new(8);

        assert!(guard
            .authorize(&peer, &claimed, 700, b"old-boundary", 1_000)
            .is_ok());
        assert!(guard
            .authorize(&peer, &claimed, 1_030, b"future-boundary", 1_000)
            .is_ok());
        assert_eq!(
            guard.authorize(&peer, &claimed, 699, b"stale", 1_000),
            Err(RelayReadAuthError::Stale)
        );
        assert_eq!(
            guard.authorize(&peer, &claimed, 1_031, b"future", 1_000),
            Err(RelayReadAuthError::Future)
        );
    }

    #[test]
    fn rejects_claimed_peer_spoofing_and_cross_connection_replay() {
        let victim = peer();
        let attacker = peer();
        let claimed = victim.to_string();
        let mut guard = RelayReadGuard::new(8);

        assert_eq!(
            guard.authorize(&attacker, &claimed, 1_000, b"captured", 1_000),
            Err(RelayReadAuthError::RequesterMismatch)
        );
        assert!(guard
            .authorize(&victim, &claimed, 1_000, b"captured", 1_000)
            .is_ok());
        assert_eq!(
            guard.authorize(&victim, &claimed, 1_000, b"captured", 1_001),
            Err(RelayReadAuthError::Replay)
        );
    }

    #[test]
    fn replay_entry_expires_only_after_the_request_validity_boundary() {
        let peer = peer();
        let claimed = peer.to_string();
        let mut guard = RelayReadGuard::new(8);
        assert!(guard
            .authorize(&peer, &claimed, 1_000, b"same", 1_000)
            .is_ok());
        assert_eq!(
            guard.authorize(&peer, &claimed, 1_000, b"same", 1_300),
            Err(RelayReadAuthError::Replay)
        );
        assert_eq!(
            guard.authorize(&peer, &claimed, 1_000, b"same", 1_301),
            Err(RelayReadAuthError::Stale)
        );
    }

    #[test]
    fn failed_signature_can_release_capacity_but_successful_reads_fail_closed_at_capacity() {
        let peer = peer();
        let claimed = peer.to_string();
        let mut guard = RelayReadGuard::new(1);
        let token = guard
            .authorize(&peer, &claimed, 1_000, b"invalid", 1_000)
            .unwrap();
        guard.discard(token);
        assert!(guard
            .authorize(&peer, &claimed, 1_000, b"valid", 1_000)
            .is_ok());
        assert_eq!(
            guard.authorize(&peer, &claimed, 1_000, b"other", 1_000),
            Err(RelayReadAuthError::ReplayCapacity)
        );
    }
}
