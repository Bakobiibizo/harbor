//! Source-scoped admission budgets that cannot be bypassed by rotating PeerIds.

use std::collections::{HashMap, VecDeque};

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub per_source: u32,
    pub global: u32,
    pub max_sources: usize,
    pub window_secs: i64,
}

/// A bounded sliding-window guard keyed by a privacy-preserving network prefix.
pub struct SourceAdmissionGuard {
    limits: Limits,
    events: VecDeque<(i64, String)>,
    sources: HashMap<String, u32>,
}

impl SourceAdmissionGuard {
    pub fn new(limits: Limits) -> Self {
        assert!(limits.per_source > 0);
        assert!(limits.global > 0);
        assert!(limits.max_sources > 0);
        assert!(limits.window_secs > 0);
        Self {
            limits,
            events: VecDeque::new(),
            sources: HashMap::new(),
        }
    }

    /// Records one request. The transport PeerId is deliberately not part of
    /// the key, so creating fresh identities cannot reset a source budget.
    pub fn check_and_record(
        &mut self,
        source_network: &str,
        _transport_peer_id: &str,
        at: i64,
    ) -> Result<(), &'static str> {
        self.prune(at);
        let existing = self.sources.get(source_network).copied().unwrap_or(0);
        if existing >= self.limits.per_source
            || self.events.len() >= self.limits.global as usize
            || (existing == 0 && self.sources.len() >= self.limits.max_sources)
        {
            return Err("RELAY_SOURCE_ADMISSION_LIMIT");
        }
        self.events.push_back((at, source_network.to_owned()));
        *self.sources.entry(source_network.to_owned()).or_default() += 1;
        Ok(())
    }

    pub fn prune(&mut self, at: i64) {
        while self.events.front().is_some_and(|(recorded_at, _)| {
            at.saturating_sub(*recorded_at) >= self.limits.window_secs
        }) {
            let (_, source) = self.events.pop_front().expect("front was present");
            if let Some(count) = self.sources.get_mut(&source) {
                *count -= 1;
                if *count == 0 {
                    self.sources.remove(&source);
                }
            }
        }
    }

    #[cfg(test)]
    fn state_counts(&self) -> (usize, usize) {
        (self.events.len(), self.sources.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotating_peer_ids_cannot_evade_the_source_budget() {
        let mut guard = SourceAdmissionGuard::new(Limits {
            per_source: 2,
            global: 10,
            max_sources: 10,
            window_secs: 60,
        });
        assert!(guard
            .check_and_record("203.0.113.0/24", "peer-1", 100)
            .is_ok());
        assert!(guard
            .check_and_record("203.0.113.0/24", "peer-2", 100)
            .is_ok());
        assert_eq!(
            guard.check_and_record("203.0.113.0/24", "peer-3", 100),
            Err("RELAY_SOURCE_ADMISSION_LIMIT")
        );
        assert_eq!(guard.state_counts(), (2, 1));
    }

    #[test]
    fn unique_source_churn_is_bounded_and_expiry_reclaims_capacity() {
        let mut guard = SourceAdmissionGuard::new(Limits {
            per_source: 2,
            global: 3,
            max_sources: 2,
            window_secs: 10,
        });
        assert!(guard.check_and_record("source-a", "peer-a", 100).is_ok());
        assert!(guard.check_and_record("source-b", "peer-b", 100).is_ok());
        assert!(guard.check_and_record("source-c", "peer-c", 100).is_err());
        assert_eq!(guard.state_counts(), (2, 2));

        assert!(guard.check_and_record("source-c", "peer-c", 110).is_ok());
        assert_eq!(guard.state_counts(), (1, 1));
    }

    #[test]
    fn global_cardinality_is_a_hard_bound() {
        let mut guard = SourceAdmissionGuard::new(Limits {
            per_source: 10,
            global: 2,
            max_sources: 10,
            window_secs: 60,
        });
        assert!(guard.check_and_record("a", "one", 100).is_ok());
        assert!(guard.check_and_record("b", "two", 100).is_ok());
        assert!(guard.check_and_record("c", "three", 100).is_err());
        assert_eq!(guard.state_counts(), (2, 2));
    }
}
