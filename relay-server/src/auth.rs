//! Passwordless, action-scoped relay sessions.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use libp2p::{identity::PublicKey, PeerId};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

const CHALLENGE_TTL_SECS: i64 = 120;
const SESSION_TTL_SECS: i64 = 900;

/// Hard bounds for authentication state held by one relay process.
#[derive(Clone, Copy, Debug)]
pub struct StateLimits {
    pub max_entries: usize,
    pub replay_retention_secs: i64,
}

impl Default for StateLimits {
    fn default() -> Self {
        Self {
            max_entries: 10_000,
            replay_retention_secs: 600,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthChallenge {
    pub domain: String,
    pub version: u8,
    pub id: String,
    pub relay: String,
    pub peer_id: String,
    pub audience: String,
    pub issued_at: i64,
    pub expires_at: i64,
    pub nonce: String,
    pub key_id: String,
    pub relay_public_key: Vec<u8>,
    pub relay_signature: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionClaims {
    domain: String,
    version: u8,
    id: String,
    relay: String,
    peer_id: String,
    audience: String,
    issued_at: i64,
    expires_at: i64,
    key_id: String,
    epoch: String,
}

pub struct AuthService {
    relay: String,
    key_id: String,
    signing_key: libp2p::identity::Keypair,
    outstanding: HashMap<String, AuthChallenge>,
    used_challenges: HashMap<String, i64>,
    revoked_sessions: HashSet<String>,
    limits: StateLimits,
    epoch: String,
}

impl AuthService {
    pub fn relay_name(&self) -> &str {
        &self.relay
    }

    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    pub fn signing_key(&self) -> libp2p::identity::Keypair {
        self.signing_key.clone()
    }
    #[allow(dead_code)] // Used by library consumers/tests; the binary supplies explicit limits.
    pub fn new(
        relay: impl Into<String>,
        key_id: impl Into<String>,
        signing_key: libp2p::identity::Keypair,
    ) -> Self {
        Self::new_with_limits(relay, key_id, signing_key, StateLimits::default())
    }

    pub fn new_with_limits(
        relay: impl Into<String>,
        key_id: impl Into<String>,
        signing_key: libp2p::identity::Keypair,
        limits: StateLimits,
    ) -> Self {
        assert!(limits.max_entries > 0, "auth max_entries must be nonzero");
        assert!(
            limits.replay_retention_secs > 0,
            "auth replay retention must be positive"
        );
        Self {
            relay: relay.into(),
            key_id: key_id.into(),
            signing_key,
            outstanding: HashMap::new(),
            used_challenges: HashMap::new(),
            revoked_sessions: HashSet::new(),
            limits,
            epoch: uuid::Uuid::new_v4().to_string(),
        }
    }

    pub fn issue_challenge(
        &mut self,
        peer_id: &PeerId,
        audience: &str,
        at: i64,
    ) -> Result<AuthChallenge, String> {
        validate_audience(audience)?;
        self.prune(at);
        if self.outstanding.len() >= self.limits.max_entries {
            return Err("challenge capacity reached".into());
        }
        let mut random = [0u8; 32];
        OsRng.fill_bytes(&mut random);
        let mut challenge = AuthChallenge {
            domain: "harbor/relay-challenge/1".into(),
            version: 1,
            id: uuid::Uuid::new_v4().to_string(),
            relay: self.relay.clone(),
            peer_id: peer_id.to_string(),
            audience: audience.into(),
            issued_at: at,
            expires_at: at + CHALLENGE_TTL_SECS,
            nonce: URL_SAFE_NO_PAD.encode(random),
            key_id: self.key_id.clone(),
            relay_public_key: self.signing_key.public().encode_protobuf(),
            relay_signature: vec![],
        };
        challenge.relay_signature = self
            .signing_key
            .sign(&challenge_bytes(&challenge)?)
            .map_err(|e| e.to_string())?;
        self.outstanding
            .insert(challenge.id.clone(), challenge.clone());
        Ok(challenge)
    }

    pub fn complete(
        &mut self,
        challenge: &AuthChallenge,
        public_key_protobuf: &[u8],
        client_signature: &[u8],
        at: i64,
    ) -> Result<String, String> {
        self.prune(at);
        if challenge.relay != self.relay || challenge.key_id != self.key_id {
            return Err("challenge authority mismatch".into());
        }
        if at > challenge.expires_at || at < challenge.issued_at {
            return Err("challenge expired".into());
        }
        if self.used_challenges.contains_key(&challenge.id) {
            return Err("challenge already used".into());
        }
        let stored = self
            .outstanding
            .get(&challenge.id)
            .ok_or("unknown challenge")?;
        if challenge_bytes(stored)? != challenge_bytes(challenge)?
            || stored.relay_signature != challenge.relay_signature
        {
            return Err("challenge tampered".into());
        }
        if !self
            .signing_key
            .public()
            .verify(&challenge_bytes(challenge)?, &challenge.relay_signature)
        {
            return Err("invalid relay signature".into());
        }
        let public = PublicKey::try_decode_protobuf(public_key_protobuf)
            .map_err(|_| "invalid public key")?;
        if PeerId::from_public_key(&public).to_string() != challenge.peer_id {
            return Err("peer ID does not match public key".into());
        }
        if !public.verify(&challenge_bytes(challenge)?, client_signature) {
            return Err("invalid client signature".into());
        }
        if self.used_challenges.len() >= self.limits.max_entries {
            return Err("challenge replay capacity reached".into());
        }
        self.outstanding.remove(&challenge.id);
        self.used_challenges.insert(
            challenge.id.clone(),
            at.saturating_add(self.limits.replay_retention_secs),
        );
        let claims = SessionClaims {
            domain: "harbor/relay-session/1".into(),
            version: 1,
            id: uuid::Uuid::new_v4().to_string(),
            relay: self.relay.clone(),
            peer_id: challenge.peer_id.clone(),
            audience: challenge.audience.clone(),
            issued_at: at,
            expires_at: at + SESSION_TTL_SECS,
            key_id: self.key_id.clone(),
            epoch: self.epoch.clone(),
        };
        self.encode_token(&claims)
    }

    pub fn authorize(&self, token: &str, audience: &str, at: i64) -> Result<PeerId, String> {
        let (payload, sig) = token.split_once('.').ok_or("malformed token")?;
        let bytes = URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(|_| "malformed token")?;
        let signature = URL_SAFE_NO_PAD.decode(sig).map_err(|_| "malformed token")?;
        if !self.signing_key.public().verify(&bytes, &signature) {
            return Err("invalid token signature".into());
        }
        let claims: SessionClaims = cbor_decode(&bytes)?;
        if claims.domain != "harbor/relay-session/1" {
            return Err("invalid token domain".into());
        }
        if claims.relay != self.relay
            || claims.key_id != self.key_id
            || claims.epoch != self.epoch
            || claims.audience != audience
        {
            return Err("token scope mismatch".into());
        }
        if at > claims.expires_at
            || at < claims.issued_at
            || self.revoked_sessions.contains(&claims.id)
        {
            return Err("token expired or revoked".into());
        }
        claims
            .peer_id
            .parse()
            .map_err(|_| "invalid token peer ID".into())
    }

    fn encode_token(&self, claims: &SessionClaims) -> Result<String, String> {
        let bytes = cbor(claims)?;
        let signature = self.signing_key.sign(&bytes).map_err(|e| e.to_string())?;
        Ok(format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(bytes),
            URL_SAFE_NO_PAD.encode(signature)
        ))
    }

    fn prune(&mut self, at: i64) {
        self.outstanding
            .retain(|_, challenge| challenge.expires_at >= at);
        self.used_challenges
            .retain(|_, expires_at| *expires_at >= at);
    }

    #[cfg(test)]
    fn state_counts(&self) -> (usize, usize) {
        (self.outstanding.len(), self.used_challenges.len())
    }
}

fn validate_audience(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b == b'-' || b == b':')
    {
        Err("invalid audience".into())
    } else {
        Ok(())
    }
}
fn challenge_bytes(value: &AuthChallenge) -> Result<Vec<u8>, String> {
    let mut unsigned = value.clone();
    unsigned.relay_signature.clear();
    if unsigned.domain != "harbor/relay-challenge/1" {
        return Err("invalid challenge domain".into());
    }
    cbor(&unsigned)
}
fn cbor<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    ciborium::ser::into_writer(value, &mut out).map_err(|e| e.to_string())?;
    Ok(out)
}
fn cbor_decode<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T, String> {
    ciborium::de::from_reader(bytes).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> (AuthService, libp2p::identity::Keypair, PeerId) {
        let relay = libp2p::identity::Keypair::generate_ed25519();
        let client = libp2p::identity::Keypair::generate_ed25519();
        let peer = client.public().to_peer_id();
        (AuthService::new("relay.test", "k1", relay), client, peer)
    }
    #[test]
    fn authenticates_and_scopes_session() {
        let (mut s, c, p) = setup();
        let ch = s.issue_challenge(&p, "introduce", 100).unwrap();
        let sig = c.sign(&challenge_bytes(&ch).unwrap()).unwrap();
        let token = s
            .complete(&ch, &c.public().encode_protobuf(), &sig, 101)
            .unwrap();
        assert_eq!(s.authorize(&token, "introduce", 102).unwrap(), p);
        assert!(s.authorize(&token, "register", 102).is_err());
    }
    #[test]
    fn rejects_replay_expiry_wrong_key_and_tampering() {
        let (mut s, c, p) = setup();
        let ch = s.issue_challenge(&p, "introduce", 100).unwrap();
        let sig = c.sign(&challenge_bytes(&ch).unwrap()).unwrap();
        let token = s
            .complete(&ch, &c.public().encode_protobuf(), &sig, 101)
            .unwrap();
        assert!(s
            .complete(&ch, &c.public().encode_protobuf(), &sig, 101)
            .is_err());
        assert!(s.authorize(&token, "introduce", 2000).is_err());
        let other = libp2p::identity::Keypair::generate_ed25519();
        let mut ch2 = s.issue_challenge(&p, "introduce", 100).unwrap();
        ch2.peer_id = other.public().to_peer_id().to_string();
        assert!(s
            .complete(&ch2, &c.public().encode_protobuf(), &sig, 101)
            .is_err());
        let mut bad = token;
        bad.push('x');
        assert!(s.authorize(&bad, "introduce", 102).is_err());
    }
    #[test]
    fn concurrent_authorization_does_not_consume_token() {
        let (mut s, c, p) = setup();
        let ch = s.issue_challenge(&p, "introduce", 100).unwrap();
        let sig = c.sign(&challenge_bytes(&ch).unwrap()).unwrap();
        let token = s
            .complete(&ch, &c.public().encode_protobuf(), &sig, 101)
            .unwrap();
        for _ in 0..8 {
            assert!(s.authorize(&token, "introduce", 102).is_ok());
        }
    }
    #[test]
    fn relay_restart_invalidates_prior_session_epoch() {
        let (mut s, c, p) = setup();
        let ch = s.issue_challenge(&p, "introduce", 100).unwrap();
        let sig = c.sign(&challenge_bytes(&ch).unwrap()).unwrap();
        let token = s
            .complete(&ch, &c.public().encode_protobuf(), &sig, 101)
            .unwrap();
        let restarted = AuthService::new("relay.test", "k1", s.signing_key());
        assert!(restarted.authorize(&token, "introduce", 102).is_err());
    }

    #[test]
    fn challenge_churn_is_hard_bounded_and_expiry_reclaims_capacity() {
        let relay = libp2p::identity::Keypair::generate_ed25519();
        let mut service = AuthService::new_with_limits(
            "relay.test",
            "k1",
            relay,
            StateLimits {
                max_entries: 3,
                replay_retention_secs: 10,
            },
        );
        for _ in 0..3 {
            let peer = libp2p::identity::Keypair::generate_ed25519()
                .public()
                .to_peer_id();
            service.issue_challenge(&peer, "introduce", 100).unwrap();
        }
        let rotating_peer = libp2p::identity::Keypair::generate_ed25519()
            .public()
            .to_peer_id();
        assert!(service
            .issue_challenge(&rotating_peer, "introduce", 100)
            .is_err());
        assert_eq!(service.state_counts(), (3, 0));

        // The issuance path always prunes before checking its hard bound.
        assert!(service
            .issue_challenge(&rotating_peer, "introduce", 221)
            .is_ok());
        assert_eq!(service.state_counts(), (1, 0));
    }

    #[test]
    fn replay_records_expire_without_weakening_single_use_within_retention() {
        let relay = libp2p::identity::Keypair::generate_ed25519();
        let client = libp2p::identity::Keypair::generate_ed25519();
        let peer = client.public().to_peer_id();
        let mut service = AuthService::new_with_limits(
            "relay.test",
            "k1",
            relay,
            StateLimits {
                max_entries: 1,
                replay_retention_secs: 10,
            },
        );
        let challenge = service.issue_challenge(&peer, "introduce", 100).unwrap();
        let signature = client.sign(&challenge_bytes(&challenge).unwrap()).unwrap();
        service
            .complete(
                &challenge,
                &client.public().encode_protobuf(),
                &signature,
                101,
            )
            .unwrap();
        assert!(service
            .complete(
                &challenge,
                &client.public().encode_protobuf(),
                &signature,
                102,
            )
            .is_err());
        assert_eq!(service.state_counts(), (0, 1));

        service.issue_challenge(&peer, "introduce", 112).unwrap();
        assert_eq!(service.state_counts(), (1, 0));
    }
}
