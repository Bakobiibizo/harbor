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
}
impl AbuseGuard {
    pub fn new(limits: Limits) -> Self {
        Self {
            limits,
            events: VecDeque::new(),
            used: HashSet::new(),
            pressure: HashMap::new(),
        }
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
            assert!(g
                .check_and_record(&c, solve(&c), "10.0.0.0/24", 100, false)
                .is_ok());
        }
        let c = challenge("3", 4);
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
        assert!(g.check_and_record(&c, 0, "net", 100, true).is_ok());
    }
    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }
}
