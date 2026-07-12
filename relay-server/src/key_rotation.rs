//! Relay signing-key trust-on-first-use and signed rotation chains.
use libp2p::identity::PublicKey;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RotationRecord {
    pub domain: String,
    pub version: u8,
    pub relay: String,
    pub sequence: u64,
    pub previous_key_id: String,
    pub successor_key_id: String,
    pub successor_public_key: Vec<u8>,
    pub valid_from: i64,
    pub expires_at: i64,
    pub compromise_from: Option<i64>,
    pub signature: Vec<u8>,
}
fn bytes(r: &RotationRecord) -> Result<Vec<u8>, String> {
    let mut u = r.clone();
    u.signature.clear();
    if u.domain != "harbor/relay-key-rotation/1" {
        return Err("invalid rotation domain".into());
    }
    let mut out = Vec::new();
    ciborium::ser::into_writer(&u, &mut out).map_err(|e| e.to_string())?;
    Ok(out)
}
#[derive(Clone)]
struct Pin {
    sequence: u64,
    key_id: String,
    public_key: PublicKey,
    valid_from: i64,
    expires_at: i64,
    compromise_from: Option<i64>,
}
pub struct PinStore {
    pins: HashMap<String, Pin>,
}
impl PinStore {
    pub fn new() -> Self {
        Self {
            pins: HashMap::new(),
        }
    }
    pub fn trust_first_use(
        &mut self,
        relay: &str,
        key_id: &str,
        key: PublicKey,
        valid_from: i64,
        expires_at: i64,
    ) -> Result<(), String> {
        if self.pins.contains_key(relay) {
            return Err("relay already pinned".into());
        }
        self.pins.insert(
            relay.into(),
            Pin {
                sequence: 0,
                key_id: key_id.into(),
                public_key: key,
                valid_from,
                expires_at,
                compromise_from: None,
            },
        );
        Ok(())
    }
    pub fn apply_rotation(&mut self, r: &RotationRecord, at: i64) -> Result<(), String> {
        let p = self
            .pins
            .get(&r.relay)
            .ok_or("untrusted relay; explicit approval required")?;
        if r.sequence != p.sequence + 1 || r.previous_key_id != p.key_id {
            return Err("rotation rollback or discontinuity".into());
        }
        if at < r.valid_from || at > r.expires_at || r.valid_from < p.valid_from {
            return Err("rotation outside validity window".into());
        }
        if !p.public_key.verify(&bytes(r)?, &r.signature) {
            return Err("unverifiable replacement; explicit approval required".into());
        }
        let key = PublicKey::try_decode_protobuf(&r.successor_public_key)
            .map_err(|_| "invalid successor key")?;
        self.pins.insert(
            r.relay.clone(),
            Pin {
                sequence: r.sequence,
                key_id: r.successor_key_id.clone(),
                public_key: key,
                valid_from: r.valid_from,
                expires_at: r.expires_at,
                compromise_from: r.compromise_from,
            },
        );
        Ok(())
    }
    pub fn accepts(&self, relay: &str, key_id: &str, issued_at: i64, at: i64) -> bool {
        self.pins.get(relay).is_some_and(|p| {
            p.key_id == key_id
                && issued_at >= p.valid_from
                && issued_at <= p.expires_at
                && at <= p.expires_at
                && p.compromise_from.is_none_or(|c| issued_at < c)
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn rotation(
        old: &libp2p::identity::Keypair,
        new: &libp2p::identity::Keypair,
        seq: u64,
        prev: &str,
    ) -> RotationRecord {
        let mut r = RotationRecord {
            domain: "harbor/relay-key-rotation/1".into(),
            version: 1,
            relay: "relay.test".into(),
            sequence: seq,
            previous_key_id: prev.into(),
            successor_key_id: format!("k{}", seq + 1),
            successor_public_key: new.public().encode_protobuf(),
            valid_from: 100,
            expires_at: 1000,
            compromise_from: None,
            signature: vec![],
        };
        r.signature = old.sign(&bytes(&r).unwrap()).unwrap();
        r
    }
    #[test]
    fn ordinary_rotation_and_rollback() {
        let old = libp2p::identity::Keypair::generate_ed25519();
        let new = libp2p::identity::Keypair::generate_ed25519();
        let mut s = PinStore::new();
        s.trust_first_use("relay.test", "k1", old.public(), 0, 500)
            .unwrap();
        let r = rotation(&old, &new, 1, "k1");
        s.apply_rotation(&r, 100).unwrap();
        assert!(s.accepts("relay.test", "k2", 101, 101));
        assert!(s.apply_rotation(&r, 101).is_err())
    }
    #[test]
    fn rejects_unknown_wrong_and_expired_replacements() {
        let a = libp2p::identity::Keypair::generate_ed25519();
        let b = libp2p::identity::Keypair::generate_ed25519();
        let evil = libp2p::identity::Keypair::generate_ed25519();
        let mut s = PinStore::new();
        let r = rotation(&a, &b, 1, "k1");
        assert!(s.apply_rotation(&r, 100).is_err());
        s.trust_first_use("relay.test", "k1", a.public(), 0, 500)
            .unwrap();
        let mut bad = r.clone();
        bad.signature = evil.sign(&bytes(&bad).unwrap()).unwrap();
        assert!(s.apply_rotation(&bad, 100).is_err());
        assert!(!s.accepts("relay.test", "k1", 10, 501));
    }
    #[test]
    fn invalidates_compromise_window() {
        let a = libp2p::identity::Keypair::generate_ed25519();
        let mut s = PinStore::new();
        s.trust_first_use("relay.test", "k1", a.public(), 0, 500)
            .unwrap();
        s.pins.get_mut("relay.test").unwrap().compromise_from = Some(200);
        assert!(s.accepts("relay.test", "k1", 199, 250));
        assert!(!s.accepts("relay.test", "k1", 200, 250));
    }
}
