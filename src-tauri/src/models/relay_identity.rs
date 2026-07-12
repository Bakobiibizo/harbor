//! Versioned wire objects for relay-scoped names and private introductions.
//!
//! Durable objects include an explicit signature domain. This prevents a valid
//! signature for one Harbor protocol object from being replayed as another.

use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u16 = 1;
pub const MAX_LOCAL_NAME_BYTES: usize = 32;
pub const MAX_RELAY_HOST_BYTES: usize = 253;
pub const MAX_CIPHERTEXT_BYTES: usize = 64 * 1024;
pub const MAX_CAPABILITIES: usize = 32;

pub mod domain {
    pub const NAME_CLAIM_REQUEST: &str = "harbor/name-claim-request/1";
    pub const NAME_CLAIM: &str = "harbor/name-claim/1";
    pub const RELAY_CHALLENGE: &str = "harbor/relay-challenge/1";
    pub const INTRODUCTION: &str = "harbor/introduction/1";
    pub const CONTACT_CARD: &str = "harbor/contact-card/1";
    pub const CAPABILITY_GRANT: &str = "harbor/capability-grant/1";
    pub const CAPABILITY_REVOCATION: &str = "harbor/capability-revocation/1";
    pub const MENTION: &str = "harbor/mention/1";
    pub const RELAY_KEY_ROTATION: &str = "harbor/relay-key-rotation/1";
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NameClaim {
    pub request: NameClaimRequest,
    pub user_signature: Vec<u8>,
    pub status: String,
    pub not_before: i64,
    pub not_after: i64,
    pub relay_key_id: String,
    pub relay_signature: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayChallenge {
    pub domain: String,
    pub version: u16,
    pub challenge_id: String,
    pub relay: String,
    pub audience: String,
    pub requester_peer_id: String,
    pub issued_at: i64,
    pub expires_at: i64,
    pub nonce: Vec<u8>,
    pub difficulty: u8,
    pub relay_key_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntroductionRequest {
    pub domain: String,
    pub version: u16,
    pub request_id: String,
    pub target: String,
    pub requester_peer_id: String,
    pub requester_signing_key: Vec<u8>,
    pub requester_ephemeral_x25519_key: Vec<u8>,
    pub purpose: String,
    pub message_ciphertext: Vec<u8>,
    pub issued_at: i64,
    pub expires_at: i64,
    pub challenge_id: String,
    pub work_nonce: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityGrantRecord {
    pub domain: String,
    pub version: u16,
    pub grant_id: String,
    pub issuer_peer_id: String,
    pub subject_peer_id: String,
    pub capability: String,
    pub revision: u64,
    pub issued_at: i64,
    pub expires_at: Option<i64>,
    pub revocation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContactCard {
    pub domain: String,
    pub version: u16,
    pub name_claim_digest: Vec<u8>,
    pub peer_id: String,
    pub ed25519_public_key: Vec<u8>,
    pub x25519_public_key: Vec<u8>,
    pub routing: Vec<String>,
    pub capabilities: Vec<CapabilityGrantRecord>,
    pub issued_at: i64,
    pub expires_at: i64,
    pub revision: u64,
    pub revocation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityRevocationRecord {
    pub domain: String,
    pub version: u16,
    pub grant_id: String,
    pub issuer_peer_id: String,
    pub revision: u64,
    pub revoked_at: i64,
    pub revocation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MentionRecord {
    pub domain: String,
    pub version: u16,
    pub qualified_name: String,
    pub target_peer_id: Option<String>,
    pub name_claim_digest: Option<Vec<u8>>,
    pub delivery_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayKeyRotation {
    pub domain: String,
    pub version: u16,
    pub relay: String,
    pub previous_key_id: String,
    pub next_key_id: String,
    pub next_public_key: Vec<u8>,
    pub not_before: i64,
    pub issued_at: i64,
    pub sequence: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::Signable;

    fn fixture() -> NameClaimRequest {
        NameClaimRequest {
            domain: domain::NAME_CLAIM_REQUEST.into(),
            version: PROTOCOL_VERSION,
            local_name: "alice".into(),
            relay: "relay.example".into(),
            peer_id: "12D3KooWFixture".into(),
            ed25519_public_key: vec![1; 32],
            x25519_public_key: vec![2; 32],
            sequence: 1,
            issued_at: 1_783_861_200,
            nonce: vec![3; 32],
        }
    }

    #[test]
    fn name_claim_request_has_deterministic_bytes() {
        let first = fixture().signable_bytes().unwrap();
        let second = fixture().signable_bytes().unwrap();
        assert_eq!(first, second);
        assert_eq!(hex::encode(first), "aa66646f6d61696e781b686172626f722f6e616d652d636c61696d2d726571756573742f316776657273696f6e016a6c6f63616c5f6e616d6565616c6963656572656c61796d72656c61792e6578616d706c6567706565725f69646f313244334b6f6f574669787475726572656432353531395f7075626c69635f6b657998200101010101010101010101010101010101010101010101010101010101010101717832353531395f7075626c69635f6b6579982002020202020202020202020202020202020202020202020202020202020202026873657175656e636501696973737565645f61741a6a538fd0656e6f6e636598200303030303030303030303030303030303030303030303030303030303030303");
    }

    #[test]
    fn signature_domain_changes_signed_bytes() {
        let baseline = fixture().signable_bytes().unwrap();
        let mut wrong_domain = fixture();
        wrong_domain.domain = domain::MENTION.into();
        assert_ne!(baseline, wrong_domain.signable_bytes().unwrap());
    }
}
