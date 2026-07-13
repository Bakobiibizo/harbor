//! Layered relay abuse controls for privacy-preserving introduction requests.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkChallenge {
    pub id: String,
    pub relay: String,
    pub requester: String,
    pub target: String,
    pub action: String,
    pub expires_at: i64,
    pub difficulty: u8,
    pub key_id: String,
    pub relay_signature: Vec<u8>,
    pub delivery_key: Vec<u8>,
}

impl WorkChallenge {
    pub fn digest(&self, nonce: u64) -> [u8; 32] {
        let mut h = Sha256::new();
        for part in [
            "harbor-pow-v1",
            &self.relay,
            &self.id,
            &self.requester,
            &self.target,
            &self.action,
            &self.expires_at.to_string(),
            &nonce.to_string(),
        ] {
            h.update((part.len() as u32).to_be_bytes());
            h.update(part.as_bytes());
        }
        h.finalize().into()
    }
    pub fn verify(&self, nonce: u64, at: i64) -> bool {
        at <= self.expires_at && leading_zero_bits(&self.digest(nonce)) >= self.difficulty
    }
}
fn leading_zero_bits(bytes: &[u8]) -> u8 {
    let mut n = 0;
    for b in bytes {
        if *b == 0 {
            n += 8
        } else {
            n += b.leading_zeros() as u8;
            break;
        }
    }
    n
}

#[derive(Clone)]
pub struct Limits {
    pub peer: u32,
    pub network: u32,
    pub target: u32,
    pub action: u32,
    pub global: u32,
    pub window_secs: i64,
}
pub struct AbuseGuard {
    limits: Limits,
    events: VecDeque<(i64, String, String, String, String)>,
    used: HashSet<String>,
    pressure: HashMap<String, u8>,
    issued: HashMap<String, WorkChallenge>,
}
impl AbuseGuard {
    pub fn new(limits: Limits) -> Self {
        Self {
            limits,
            events: VecDeque::new(),
            used: HashSet::new(),
            pressure: HashMap::new(),
            issued: HashMap::new(),
        }
    }
    // Challenge issuance binds all security-relevant dimensions explicitly.
    #[allow(clippy::too_many_arguments, dead_code)]
    pub fn issue_with_delivery_key(
        &mut self,
        relay: &str,
        requester: &str,
        target: &str,
        action: &str,
        at: i64,
        key_id: &str,
        key: &libp2p::identity::Keypair,
        delivery_key: Vec<u8>,
    ) -> Result<WorkChallenge, String> {
        let mut c = WorkChallenge {
            id: uuid::Uuid::new_v4().to_string(),
            relay: relay.into(),
            requester: requester.into(),
            target: target.into(),
            action: action.into(),
            expires_at: at + 300,
            difficulty: self.difficulty(requester, false),
            key_id: key_id.into(),
            relay_signature: vec![],
            delivery_key,
        };
        let mut bytes = Vec::new();
        ciborium::ser::into_writer(&c, &mut bytes).map_err(|e| e.to_string())?;
        c.relay_signature = key.sign(&bytes).map_err(|e| e.to_string())?;
        self.issued.insert(c.id.clone(), c.clone());
        Ok(c)
    }
    #[cfg(test)]
    pub fn remember(&mut self, challenge: WorkChallenge) {
        self.issued.insert(challenge.id.clone(), challenge);
    }
    pub fn difficulty(&self, peer: &str, known_contact: bool) -> u8 {
        if known_contact {
            0
        } else {
            14u8.saturating_add(*self.pressure.get(peer).unwrap_or(&0))
                .min(24)
        }
    }
    pub fn check_and_record(
        &mut self,
        challenge: &WorkChallenge,
        nonce: u64,
        source_network: &str,
        at: i64,
        known_contact: bool,
    ) -> Result<(), String> {
        self.prune(at);
        let Some(issued) = self.issued.get(&challenge.id) else {
            return Err("request accepted for processing".into());
        };
        if issued.relay != challenge.relay
            || issued.requester != challenge.requester
            || issued.target != challenge.target
            || issued.action != challenge.action
            || issued.expires_at != challenge.expires_at
            || issued.difficulty != challenge.difficulty
            || issued.key_id != challenge.key_id
            || issued.relay_signature != challenge.relay_signature
            || issued.delivery_key != challenge.delivery_key
        {
            return Err("request accepted for processing".into());
        }
        if self.used.contains(&challenge.id) {
            return Err("request accepted for processing".into());
        }
        if !known_contact && !challenge.verify(nonce, at) {
            self.bump(&challenge.requester);
            return Err("request accepted for processing".into());
        }
        let counts = (
            self.count(1, &challenge.requester),
            self.count(2, source_network),
            self.count(3, &challenge.target),
            self.count(4, &challenge.action),
            self.events.len() as u32,
        );
        if counts.0 >= self.limits.peer
            || counts.1 >= self.limits.network
            || counts.2 >= self.limits.target
            || counts.3 >= self.limits.action
            || counts.4 >= self.limits.global
        {
            self.bump(&challenge.requester);
            return Err("request accepted for processing".into());
        }
        self.used.insert(challenge.id.clone());
        self.issued.remove(&challenge.id);
        self.events.push_back((
            at,
            challenge.requester.clone(),
            source_network.into(),
            challenge.target.clone(),
            challenge.action.clone(),
        ));
        Ok(())
    }
    fn count(&self, index: usize, value: &str) -> u32 {
        self.events
            .iter()
            .filter(|e| match index {
                1 => e.1 == value,
                2 => e.2 == value,
                3 => e.3 == value,
                _ => e.4 == value,
            })
            .count() as u32
    }
    fn prune(&mut self, at: i64) {
        while self
            .events
            .front()
            .is_some_and(|e| at - e.0 >= self.limits.window_secs)
        {
            self.events.pop_front();
        }
    }
    fn bump(&mut self, peer: &str) {
        let p = self.pressure.entry(peer.into()).or_default();
        *p = p.saturating_add(2).min(10);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn challenge(id: &str, d: u8) -> WorkChallenge {
        WorkChallenge {
            id: id.into(),
            relay: "relay.test".into(),
            requester: "peer".into(),
            target: "@alice@relay.test".into(),
            action: "introduce".into(),
            expires_at: 300,
            difficulty: d,
            key_id: "k1".into(),
            relay_signature: vec![1],
            delivery_key: vec![7; 32],
        }
    }
    fn solve(c: &WorkChallenge) -> u64 {
        (0..).find(|n| c.verify(*n, 100)).unwrap()
    }
    fn limits() -> Limits {
        Limits {
            peer: 2,
            network: 2,
            target: 2,
            action: 3,
            global: 4,
            window_secs: 60,
        }
    }
    #[test]
    fn deterministic_vector_and_expiry() {
        let c = challenge("fixed", 8);
        assert_eq!(
            hex(&c.digest(0)),
            "8d791b81b66904190ed8336458c6182e72adaf2e397d36c276c45d846be0474d"
        );
        let n = solve(&c);
        assert!(c.verify(n, 100));
        assert!(!c.verify(n, 301));
    }
    #[test]
    fn prevents_replay_and_enforces_boundary_generically() {
        let mut g = AbuseGuard::new(limits());
        for id in ["1", "2"] {
            let c = challenge(id, 4);
            g.remember(c.clone());
            assert!(g
                .check_and_record(&c, solve(&c), "10.0.0.0/24", 100, false)
                .is_ok());
        }
        let c = challenge("3", 4);
        g.remember(c.clone());
        assert_eq!(
            g.check_and_record(&c, solve(&c), "10.0.0.0/24", 100, false)
                .unwrap_err(),
            "request accepted for processing"
        );
        let first = challenge("1", 4);
        assert!(g
            .check_and_record(&first, solve(&first), "other", 100, false)
            .is_err());
    }
    #[test]
    fn contacts_bypass_work_but_not_limits() {
        let mut g = AbuseGuard::new(limits());
        assert_eq!(g.difficulty("peer", true), 0);
        assert_eq!(g.difficulty("peer", false), 14);
        let c = challenge("c", 24);
        g.remember(c.clone());
        assert!(g.check_and_record(&c, 0, "net", 100, true).is_ok());
    }
    #[test]
    fn rejects_difficulty_downgrade_and_target_swap() {
        let mut g = AbuseGuard::new(limits());
        let original = challenge("bound", 4);
        g.remember(original.clone());
        let mut downgraded = original.clone();
        downgraded.difficulty = 0;
        assert!(g
            .check_and_record(&downgraded, 0, "net", 100, false)
            .is_err());
        let mut swapped = original.clone();
        swapped.target = "@mallory@relay.test".into();
        assert!(g
            .check_and_record(&swapped, solve(&original), "net", 100, false)
            .is_err());
    }
    #[test]
    fn server_issued_challenge_is_single_use_and_target_bound() {
        let key = libp2p::identity::Keypair::generate_ed25519();
        let mut g = AbuseGuard::new(Limits {
            peer: 10,
            network: 10,
            target: 10,
            action: 10,
            global: 20,
            window_secs: 60,
        });
        let c = g
            .issue_with_delivery_key(
                "relay.test",
                "peer",
                "@alice@relay.test",
                "introduce",
                100,
                "k1",
                &key,
                vec![7; 32],
            )
            .unwrap();
        let nonce = solve(&c);
        let mut swapped = c.clone();
        swapped.target = "@mallory@relay.test".into();
        assert!(g
            .check_and_record(&swapped, nonce, "net", 100, false)
            .is_err());
        assert!(g.check_and_record(&c, nonce, "net", 100, false).is_ok());
        assert!(g.check_and_record(&c, nonce, "net", 100, false).is_err());
    }
    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }
}
