//! Validated, finite production resource limits for the relay.

use clap::Args;
use serde::Serialize;
use thiserror::Error;

pub const DEFAULT_MAX_RESERVATIONS: usize = 128;
pub const DEFAULT_MAX_RESERVATIONS_PER_PEER: usize = 4;
pub const DEFAULT_MAX_CIRCUITS: usize = 512;
pub const DEFAULT_MAX_CIRCUITS_PER_PEER: usize = 16;
pub const DEFAULT_RESERVATION_DURATION_SECS: u64 = 3_600;
pub const DEFAULT_MAX_CIRCUIT_DURATION_SECS: u64 = 3_600;
pub const DEFAULT_MAX_CIRCUIT_BYTES: u64 = 67_108_864;
pub const DEFAULT_IDLE_CONNECTION_TIMEOUT_SECS: u64 = 300;
pub const DEFAULT_RESERVATION_ADMISSION_PER_PEER: u32 = 30;
pub const DEFAULT_RESERVATION_ADMISSION_PER_IP: u32 = 60;
pub const DEFAULT_CIRCUIT_ADMISSION_PER_PEER: u32 = 30;
pub const DEFAULT_CIRCUIT_ADMISSION_PER_IP: u32 = 60;
pub const DEFAULT_ADMISSION_WINDOW_SECS: u64 = 120;
pub const DEFAULT_RATE_LIMIT_MAX_REQUESTS: u64 = 60;
pub const DEFAULT_RATE_LIMIT_WINDOW_SECS: u64 = 60;
pub const DEFAULT_RATE_LIMITER_CLEANUP_INTERVAL_SECS: u64 = 300;
pub const DEFAULT_ABUSE_PEER_LIMIT: u32 = 10;
pub const DEFAULT_ABUSE_NETWORK_LIMIT: u32 = 30;
pub const DEFAULT_ABUSE_TARGET_LIMIT: u32 = 20;
pub const DEFAULT_ABUSE_ACTION_LIMIT: u32 = 100;
pub const DEFAULT_ABUSE_GLOBAL_LIMIT: u32 = 1_000;
pub const DEFAULT_ABUSE_WINDOW_SECS: u64 = 60;
pub const DEFAULT_MAX_STORAGE_BYTES: u64 = 1_073_741_824;
pub const DEFAULT_MAX_EPHEMERAL_ENTRIES: usize = 10_000;
pub const DEFAULT_EPHEMERAL_RETENTION_SECS: u64 = 600;
pub const DEFAULT_MAX_ADMISSION_SOURCES: usize = 4_096;
pub const DEFAULT_RECORD_RETENTION_SECS: u64 = 31_536_000;
pub const DEFAULT_MAX_KNOWN_PEERS: usize = 100_000;
pub const DEFAULT_MAX_POSTS: usize = 1_000_000;
pub const DEFAULT_MAX_GRANTS: usize = 500_000;
pub const DEFAULT_MAX_INTRODUCTIONS: usize = 100_000;
pub const DEFAULT_MAX_SOCIAL_EVENTS: usize = 1_000_000;

pub const RESOURCE_LIMIT_DEFAULTS: &[(&str, u64)] = &[
    (
        "HARBOR_RELAY_MAX_RESERVATIONS",
        DEFAULT_MAX_RESERVATIONS as u64,
    ),
    (
        "HARBOR_RELAY_MAX_RESERVATIONS_PER_PEER",
        DEFAULT_MAX_RESERVATIONS_PER_PEER as u64,
    ),
    ("HARBOR_RELAY_MAX_CIRCUITS", DEFAULT_MAX_CIRCUITS as u64),
    (
        "HARBOR_RELAY_MAX_CIRCUITS_PER_PEER",
        DEFAULT_MAX_CIRCUITS_PER_PEER as u64,
    ),
    (
        "HARBOR_RELAY_RESERVATION_DURATION_SECS",
        DEFAULT_RESERVATION_DURATION_SECS,
    ),
    (
        "HARBOR_RELAY_MAX_CIRCUIT_DURATION_SECS",
        DEFAULT_MAX_CIRCUIT_DURATION_SECS,
    ),
    ("HARBOR_RELAY_MAX_CIRCUIT_BYTES", DEFAULT_MAX_CIRCUIT_BYTES),
    (
        "HARBOR_RELAY_IDLE_CONNECTION_TIMEOUT_SECS",
        DEFAULT_IDLE_CONNECTION_TIMEOUT_SECS,
    ),
    (
        "HARBOR_RELAY_RESERVATION_ADMISSION_PER_PEER",
        DEFAULT_RESERVATION_ADMISSION_PER_PEER as u64,
    ),
    (
        "HARBOR_RELAY_RESERVATION_ADMISSION_PER_IP",
        DEFAULT_RESERVATION_ADMISSION_PER_IP as u64,
    ),
    (
        "HARBOR_RELAY_CIRCUIT_ADMISSION_PER_PEER",
        DEFAULT_CIRCUIT_ADMISSION_PER_PEER as u64,
    ),
    (
        "HARBOR_RELAY_CIRCUIT_ADMISSION_PER_IP",
        DEFAULT_CIRCUIT_ADMISSION_PER_IP as u64,
    ),
    (
        "HARBOR_RELAY_ADMISSION_WINDOW_SECS",
        DEFAULT_ADMISSION_WINDOW_SECS,
    ),
    (
        "HARBOR_RELAY_RATE_LIMIT_MAX_REQUESTS",
        DEFAULT_RATE_LIMIT_MAX_REQUESTS,
    ),
    (
        "HARBOR_RELAY_RATE_LIMIT_WINDOW_SECS",
        DEFAULT_RATE_LIMIT_WINDOW_SECS,
    ),
    (
        "HARBOR_RELAY_RATE_LIMITER_CLEANUP_INTERVAL_SECS",
        DEFAULT_RATE_LIMITER_CLEANUP_INTERVAL_SECS,
    ),
    (
        "HARBOR_RELAY_ABUSE_PEER_LIMIT",
        DEFAULT_ABUSE_PEER_LIMIT as u64,
    ),
    (
        "HARBOR_RELAY_ABUSE_NETWORK_LIMIT",
        DEFAULT_ABUSE_NETWORK_LIMIT as u64,
    ),
    (
        "HARBOR_RELAY_ABUSE_TARGET_LIMIT",
        DEFAULT_ABUSE_TARGET_LIMIT as u64,
    ),
    (
        "HARBOR_RELAY_ABUSE_ACTION_LIMIT",
        DEFAULT_ABUSE_ACTION_LIMIT as u64,
    ),
    (
        "HARBOR_RELAY_ABUSE_GLOBAL_LIMIT",
        DEFAULT_ABUSE_GLOBAL_LIMIT as u64,
    ),
    ("HARBOR_RELAY_ABUSE_WINDOW_SECS", DEFAULT_ABUSE_WINDOW_SECS),
    ("HARBOR_RELAY_MAX_STORAGE_BYTES", DEFAULT_MAX_STORAGE_BYTES),
    (
        "HARBOR_RELAY_MAX_EPHEMERAL_ENTRIES",
        DEFAULT_MAX_EPHEMERAL_ENTRIES as u64,
    ),
    (
        "HARBOR_RELAY_EPHEMERAL_RETENTION_SECS",
        DEFAULT_EPHEMERAL_RETENTION_SECS,
    ),
    (
        "HARBOR_RELAY_MAX_ADMISSION_SOURCES",
        DEFAULT_MAX_ADMISSION_SOURCES as u64,
    ),
    (
        "HARBOR_RELAY_RECORD_RETENTION_SECS",
        DEFAULT_RECORD_RETENTION_SECS,
    ),
    (
        "HARBOR_RELAY_MAX_KNOWN_PEERS",
        DEFAULT_MAX_KNOWN_PEERS as u64,
    ),
    ("HARBOR_RELAY_MAX_POSTS", DEFAULT_MAX_POSTS as u64),
    ("HARBOR_RELAY_MAX_GRANTS", DEFAULT_MAX_GRANTS as u64),
    (
        "HARBOR_RELAY_MAX_INTRODUCTIONS",
        DEFAULT_MAX_INTRODUCTIONS as u64,
    ),
    (
        "HARBOR_RELAY_MAX_SOCIAL_EVENTS",
        DEFAULT_MAX_SOCIAL_EVENTS as u64,
    ),
];

const MAX_CONCURRENCY: u64 = 100_000;
const MAX_DURATION_SECS: u64 = 86_400;
const MAX_ADMISSION_RATE: u64 = 1_000_000;
const MAX_CIRCUIT_BYTES: u64 = 17_179_869_184;
const MIN_STORAGE_BYTES: u64 = 16_777_216;
const MAX_STORAGE_BYTES: u64 = 1_099_511_627_776;
const MAX_STATE_ENTRIES: u64 = 1_000_000_000;
const MAX_RETENTION_SECS: u64 = 315_360_000;

#[derive(Args, Clone, Debug)]
pub struct ResourceLimitArgs {
    #[arg(long, env = "HARBOR_RELAY_MAX_RESERVATIONS", default_value_t = DEFAULT_MAX_RESERVATIONS)]
    pub max_reservations: usize,
    #[arg(long, env = "HARBOR_RELAY_MAX_RESERVATIONS_PER_PEER", default_value_t = DEFAULT_MAX_RESERVATIONS_PER_PEER)]
    pub max_reservations_per_peer: usize,
    #[arg(long, env = "HARBOR_RELAY_MAX_CIRCUITS", default_value_t = DEFAULT_MAX_CIRCUITS)]
    pub max_circuits: usize,
    #[arg(long, env = "HARBOR_RELAY_MAX_CIRCUITS_PER_PEER", default_value_t = DEFAULT_MAX_CIRCUITS_PER_PEER)]
    pub max_circuits_per_peer: usize,
    #[arg(long, env = "HARBOR_RELAY_RESERVATION_DURATION_SECS", default_value_t = DEFAULT_RESERVATION_DURATION_SECS)]
    pub reservation_duration_secs: u64,
    #[arg(long, env = "HARBOR_RELAY_MAX_CIRCUIT_DURATION_SECS", default_value_t = DEFAULT_MAX_CIRCUIT_DURATION_SECS)]
    pub max_circuit_duration_secs: u64,
    #[arg(long, env = "HARBOR_RELAY_MAX_CIRCUIT_BYTES", default_value_t = DEFAULT_MAX_CIRCUIT_BYTES)]
    pub max_circuit_bytes: u64,
    #[arg(long, env = "HARBOR_RELAY_IDLE_CONNECTION_TIMEOUT_SECS", default_value_t = DEFAULT_IDLE_CONNECTION_TIMEOUT_SECS)]
    pub idle_connection_timeout_secs: u64,
    #[arg(long, env = "HARBOR_RELAY_RESERVATION_ADMISSION_PER_PEER", default_value_t = DEFAULT_RESERVATION_ADMISSION_PER_PEER)]
    pub reservation_admission_per_peer: u32,
    #[arg(long, env = "HARBOR_RELAY_RESERVATION_ADMISSION_PER_IP", default_value_t = DEFAULT_RESERVATION_ADMISSION_PER_IP)]
    pub reservation_admission_per_ip: u32,
    #[arg(long, env = "HARBOR_RELAY_CIRCUIT_ADMISSION_PER_PEER", default_value_t = DEFAULT_CIRCUIT_ADMISSION_PER_PEER)]
    pub circuit_admission_per_peer: u32,
    #[arg(long, env = "HARBOR_RELAY_CIRCUIT_ADMISSION_PER_IP", default_value_t = DEFAULT_CIRCUIT_ADMISSION_PER_IP)]
    pub circuit_admission_per_ip: u32,
    #[arg(long, env = "HARBOR_RELAY_ADMISSION_WINDOW_SECS", default_value_t = DEFAULT_ADMISSION_WINDOW_SECS)]
    pub admission_window_secs: u64,
    #[arg(long, env = "HARBOR_RELAY_RATE_LIMIT_MAX_REQUESTS", default_value_t = DEFAULT_RATE_LIMIT_MAX_REQUESTS)]
    pub rate_limit_max_requests: u64,
    #[arg(long, env = "HARBOR_RELAY_RATE_LIMIT_WINDOW_SECS", default_value_t = DEFAULT_RATE_LIMIT_WINDOW_SECS)]
    pub rate_limit_window_secs: u64,
    #[arg(long, env = "HARBOR_RELAY_RATE_LIMITER_CLEANUP_INTERVAL_SECS", default_value_t = DEFAULT_RATE_LIMITER_CLEANUP_INTERVAL_SECS)]
    pub rate_limiter_cleanup_interval_secs: u64,
    #[arg(long, env = "HARBOR_RELAY_ABUSE_PEER_LIMIT", default_value_t = DEFAULT_ABUSE_PEER_LIMIT)]
    pub abuse_peer_limit: u32,
    #[arg(long, env = "HARBOR_RELAY_ABUSE_NETWORK_LIMIT", default_value_t = DEFAULT_ABUSE_NETWORK_LIMIT)]
    pub abuse_network_limit: u32,
    #[arg(long, env = "HARBOR_RELAY_ABUSE_TARGET_LIMIT", default_value_t = DEFAULT_ABUSE_TARGET_LIMIT)]
    pub abuse_target_limit: u32,
    #[arg(long, env = "HARBOR_RELAY_ABUSE_ACTION_LIMIT", default_value_t = DEFAULT_ABUSE_ACTION_LIMIT)]
    pub abuse_action_limit: u32,
    #[arg(long, env = "HARBOR_RELAY_ABUSE_GLOBAL_LIMIT", default_value_t = DEFAULT_ABUSE_GLOBAL_LIMIT)]
    pub abuse_global_limit: u32,
    #[arg(long, env = "HARBOR_RELAY_ABUSE_WINDOW_SECS", default_value_t = DEFAULT_ABUSE_WINDOW_SECS)]
    pub abuse_window_secs: u64,
    #[arg(long, env = "HARBOR_RELAY_MAX_STORAGE_BYTES", default_value_t = DEFAULT_MAX_STORAGE_BYTES)]
    pub max_storage_bytes: u64,
    #[arg(long, env = "HARBOR_RELAY_MAX_EPHEMERAL_ENTRIES", default_value_t = DEFAULT_MAX_EPHEMERAL_ENTRIES)]
    pub max_ephemeral_entries: usize,
    #[arg(long, env = "HARBOR_RELAY_EPHEMERAL_RETENTION_SECS", default_value_t = DEFAULT_EPHEMERAL_RETENTION_SECS)]
    pub ephemeral_retention_secs: u64,
    #[arg(long, env = "HARBOR_RELAY_MAX_ADMISSION_SOURCES", default_value_t = DEFAULT_MAX_ADMISSION_SOURCES)]
    pub max_admission_sources: usize,
    #[arg(long, env = "HARBOR_RELAY_RECORD_RETENTION_SECS", default_value_t = DEFAULT_RECORD_RETENTION_SECS)]
    pub record_retention_secs: u64,
    #[arg(long, env = "HARBOR_RELAY_MAX_KNOWN_PEERS", default_value_t = DEFAULT_MAX_KNOWN_PEERS)]
    pub max_known_peers: usize,
    #[arg(long, env = "HARBOR_RELAY_MAX_POSTS", default_value_t = DEFAULT_MAX_POSTS)]
    pub max_posts: usize,
    #[arg(long, env = "HARBOR_RELAY_MAX_GRANTS", default_value_t = DEFAULT_MAX_GRANTS)]
    pub max_grants: usize,
    #[arg(long, env = "HARBOR_RELAY_MAX_INTRODUCTIONS", default_value_t = DEFAULT_MAX_INTRODUCTIONS)]
    pub max_introductions: usize,
    #[arg(long, env = "HARBOR_RELAY_MAX_SOCIAL_EVENTS", default_value_t = DEFAULT_MAX_SOCIAL_EVENTS)]
    pub max_social_events: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ResourceLimits {
    pub max_reservations: usize,
    pub max_reservations_per_peer: usize,
    pub max_circuits: usize,
    pub max_circuits_per_peer: usize,
    pub reservation_duration_secs: u64,
    pub max_circuit_duration_secs: u64,
    pub max_circuit_bytes: u64,
    pub idle_connection_timeout_secs: u64,
    pub reservation_admission_per_peer: u32,
    pub reservation_admission_per_ip: u32,
    pub circuit_admission_per_peer: u32,
    pub circuit_admission_per_ip: u32,
    pub admission_window_secs: u64,
    pub rate_limit_max_requests: u64,
    pub rate_limit_window_secs: u64,
    pub rate_limiter_cleanup_interval_secs: u64,
    pub abuse_peer_limit: u32,
    pub abuse_network_limit: u32,
    pub abuse_target_limit: u32,
    pub abuse_action_limit: u32,
    pub abuse_global_limit: u32,
    pub abuse_window_secs: u64,
    pub max_storage_bytes: u64,
    pub max_ephemeral_entries: usize,
    pub ephemeral_retention_secs: u64,
    pub max_admission_sources: usize,
    pub record_retention_secs: u64,
    pub max_known_peers: usize,
    pub max_posts: usize,
    pub max_grants: usize,
    pub max_introductions: usize,
    pub max_social_events: usize,
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("invalid relay resource limits: {0}")]
pub struct ResourceLimitError(String);

fn bounded(name: &str, value: u64, minimum: u64, maximum: u64) -> Result<(), ResourceLimitError> {
    if !(minimum..=maximum).contains(&value) {
        return Err(ResourceLimitError(format!(
            "{name} must be between {minimum} and {maximum}, got {value}"
        )));
    }
    Ok(())
}

impl TryFrom<ResourceLimitArgs> for ResourceLimits {
    type Error = ResourceLimitError;

    fn try_from(args: ResourceLimitArgs) -> Result<Self, Self::Error> {
        bounded(
            "max_reservations",
            args.max_reservations as u64,
            1,
            MAX_CONCURRENCY,
        )?;
        bounded(
            "max_reservations_per_peer",
            args.max_reservations_per_peer as u64,
            1,
            MAX_CONCURRENCY,
        )?;
        bounded("max_circuits", args.max_circuits as u64, 1, MAX_CONCURRENCY)?;
        bounded(
            "max_circuits_per_peer",
            args.max_circuits_per_peer as u64,
            1,
            MAX_CONCURRENCY,
        )?;
        bounded(
            "reservation_duration_secs",
            args.reservation_duration_secs,
            60,
            MAX_DURATION_SECS,
        )?;
        bounded(
            "max_circuit_duration_secs",
            args.max_circuit_duration_secs,
            30,
            MAX_DURATION_SECS,
        )?;
        bounded(
            "max_circuit_bytes",
            args.max_circuit_bytes,
            1_048_576,
            MAX_CIRCUIT_BYTES,
        )?;
        bounded(
            "idle_connection_timeout_secs",
            args.idle_connection_timeout_secs,
            15,
            3_600,
        )?;
        for (name, value) in [
            (
                "reservation_admission_per_peer",
                args.reservation_admission_per_peer as u64,
            ),
            (
                "reservation_admission_per_ip",
                args.reservation_admission_per_ip as u64,
            ),
            (
                "circuit_admission_per_peer",
                args.circuit_admission_per_peer as u64,
            ),
            (
                "circuit_admission_per_ip",
                args.circuit_admission_per_ip as u64,
            ),
            ("rate_limit_max_requests", args.rate_limit_max_requests),
            ("abuse_peer_limit", args.abuse_peer_limit as u64),
            ("abuse_network_limit", args.abuse_network_limit as u64),
            ("abuse_target_limit", args.abuse_target_limit as u64),
            ("abuse_action_limit", args.abuse_action_limit as u64),
            ("abuse_global_limit", args.abuse_global_limit as u64),
        ] {
            bounded(name, value, 1, MAX_ADMISSION_RATE)?;
        }
        for (name, value) in [
            ("admission_window_secs", args.admission_window_secs),
            ("rate_limit_window_secs", args.rate_limit_window_secs),
            (
                "rate_limiter_cleanup_interval_secs",
                args.rate_limiter_cleanup_interval_secs,
            ),
            ("abuse_window_secs", args.abuse_window_secs),
        ] {
            bounded(name, value, 1, MAX_DURATION_SECS)?;
        }
        bounded(
            "max_storage_bytes",
            args.max_storage_bytes,
            MIN_STORAGE_BYTES,
            MAX_STORAGE_BYTES,
        )?;
        for (name, value) in [
            ("max_ephemeral_entries", args.max_ephemeral_entries as u64),
            ("max_admission_sources", args.max_admission_sources as u64),
            ("max_known_peers", args.max_known_peers as u64),
            ("max_posts", args.max_posts as u64),
            ("max_grants", args.max_grants as u64),
            ("max_introductions", args.max_introductions as u64),
            ("max_social_events", args.max_social_events as u64),
        ] {
            bounded(name, value, 1, MAX_STATE_ENTRIES)?;
        }
        for (name, value) in [
            ("ephemeral_retention_secs", args.ephemeral_retention_secs),
            ("record_retention_secs", args.record_retention_secs),
        ] {
            bounded(name, value, 1, MAX_RETENTION_SECS)?;
        }

        if args.max_reservations_per_peer > args.max_reservations {
            return Err(ResourceLimitError(
                "max_reservations_per_peer cannot exceed max_reservations".into(),
            ));
        }
        if args.max_circuits_per_peer > args.max_circuits {
            return Err(ResourceLimitError(
                "max_circuits_per_peer cannot exceed max_circuits".into(),
            ));
        }
        if args.max_circuit_duration_secs > args.reservation_duration_secs {
            return Err(ResourceLimitError(
                "max_circuit_duration_secs cannot exceed reservation_duration_secs".into(),
            ));
        }
        for (name, value) in [
            ("abuse_peer_limit", args.abuse_peer_limit),
            ("abuse_network_limit", args.abuse_network_limit),
            ("abuse_target_limit", args.abuse_target_limit),
            ("abuse_action_limit", args.abuse_action_limit),
        ] {
            if value > args.abuse_global_limit {
                return Err(ResourceLimitError(format!(
                    "{name} cannot exceed abuse_global_limit"
                )));
            }
        }

        Ok(Self {
            max_reservations: args.max_reservations,
            max_reservations_per_peer: args.max_reservations_per_peer,
            max_circuits: args.max_circuits,
            max_circuits_per_peer: args.max_circuits_per_peer,
            reservation_duration_secs: args.reservation_duration_secs,
            max_circuit_duration_secs: args.max_circuit_duration_secs,
            max_circuit_bytes: args.max_circuit_bytes,
            idle_connection_timeout_secs: args.idle_connection_timeout_secs,
            reservation_admission_per_peer: args.reservation_admission_per_peer,
            reservation_admission_per_ip: args.reservation_admission_per_ip,
            circuit_admission_per_peer: args.circuit_admission_per_peer,
            circuit_admission_per_ip: args.circuit_admission_per_ip,
            admission_window_secs: args.admission_window_secs,
            rate_limit_max_requests: args.rate_limit_max_requests,
            rate_limit_window_secs: args.rate_limit_window_secs,
            rate_limiter_cleanup_interval_secs: args.rate_limiter_cleanup_interval_secs,
            abuse_peer_limit: args.abuse_peer_limit,
            abuse_network_limit: args.abuse_network_limit,
            abuse_target_limit: args.abuse_target_limit,
            abuse_action_limit: args.abuse_action_limit,
            abuse_global_limit: args.abuse_global_limit,
            abuse_window_secs: args.abuse_window_secs,
            max_storage_bytes: args.max_storage_bytes,
            max_ephemeral_entries: args.max_ephemeral_entries,
            ephemeral_retention_secs: args.ephemeral_retention_secs,
            max_admission_sources: args.max_admission_sources,
            record_retention_secs: args.record_retention_secs,
            max_known_peers: args.max_known_peers,
            max_posts: args.max_posts,
            max_grants: args.max_grants,
            max_introductions: args.max_introductions,
            max_social_events: args.max_social_events,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct TestCli {
        #[command(flatten)]
        limits: ResourceLimitArgs,
    }

    fn defaults() -> ResourceLimitArgs {
        TestCli::try_parse_from(["test"]).unwrap().limits
    }

    #[test]
    fn production_defaults_are_finite_and_valid() {
        let limits = ResourceLimits::try_from(defaults()).unwrap();
        assert_eq!(limits.max_circuit_bytes, DEFAULT_MAX_CIRCUIT_BYTES);
        assert_eq!(
            limits.max_circuit_duration_secs,
            DEFAULT_MAX_CIRCUIT_DURATION_SECS
        );
        assert_eq!(
            limits.idle_connection_timeout_secs,
            DEFAULT_IDLE_CONNECTION_TIMEOUT_SECS
        );
        assert_eq!(limits.max_storage_bytes, DEFAULT_MAX_STORAGE_BYTES);
        assert_eq!(limits.max_ephemeral_entries, DEFAULT_MAX_EPHEMERAL_ENTRIES);
        assert_eq!(
            limits.ephemeral_retention_secs,
            DEFAULT_EPHEMERAL_RETENTION_SECS
        );
        assert_eq!(limits.max_admission_sources, DEFAULT_MAX_ADMISSION_SOURCES);
        assert_eq!(limits.record_retention_secs, DEFAULT_RECORD_RETENTION_SECS);
        assert_eq!(limits.max_known_peers, DEFAULT_MAX_KNOWN_PEERS);
        assert_eq!(limits.max_posts, DEFAULT_MAX_POSTS);
        assert_eq!(limits.max_grants, DEFAULT_MAX_GRANTS);
        assert_eq!(limits.max_introductions, DEFAULT_MAX_INTRODUCTIONS);
        assert_eq!(limits.max_social_events, DEFAULT_MAX_SOCIAL_EVENTS);
    }

    #[test]
    fn container_default_file_matches_the_model() {
        let configured = include_str!("../resource-limits.env")
            .lines()
            .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
            .map(|line| {
                let (name, value) = line.split_once('=').unwrap();
                (name, value.parse::<u64>().unwrap())
            })
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(configured.len(), RESOURCE_LIMIT_DEFAULTS.len());
        for (name, value) in RESOURCE_LIMIT_DEFAULTS {
            assert_eq!(
                configured.get(name),
                Some(value),
                "default drift for {name}"
            );
        }
    }

    #[test]
    fn environment_binding_uses_the_same_validated_model() {
        const NAME: &str = "HARBOR_RELAY_ABUSE_PEER_LIMIT";
        let previous = std::env::var_os(NAME);
        std::env::set_var(NAME, "9");
        let parsed = TestCli::try_parse_from(["test"]).unwrap().limits;
        if let Some(value) = previous {
            std::env::set_var(NAME, value);
        } else {
            std::env::remove_var(NAME);
        }
        let limits = ResourceLimits::try_from(parsed).unwrap();
        assert_eq!(limits.abuse_peer_limit, 9);
    }

    #[test]
    fn cli_overrides_are_validated_as_one_model() {
        let args = TestCli::try_parse_from([
            "test",
            "--max-reservations",
            "8",
            "--max-reservations-per-peer",
            "2",
            "--max-circuits",
            "12",
            "--max-circuits-per-peer",
            "3",
            "--max-circuit-bytes",
            "1048576",
        ])
        .unwrap()
        .limits;
        let limits = ResourceLimits::try_from(args).unwrap();
        assert_eq!(limits.max_reservations, 8);
        assert_eq!(limits.max_circuits, 12);
        assert_eq!(limits.max_circuit_bytes, 1_048_576);
    }

    #[test]
    fn bounded_state_and_retention_overrides_use_the_same_model() {
        let args = TestCli::try_parse_from([
            "test",
            "--max-ephemeral-entries",
            "101",
            "--ephemeral-retention-secs",
            "102",
            "--max-admission-sources",
            "103",
            "--record-retention-secs",
            "104",
            "--max-known-peers",
            "105",
            "--max-posts",
            "106",
            "--max-grants",
            "107",
            "--max-introductions",
            "108",
            "--max-social-events",
            "109",
        ])
        .unwrap()
        .limits;
        let limits = ResourceLimits::try_from(args).unwrap();
        assert_eq!(limits.max_ephemeral_entries, 101);
        assert_eq!(limits.ephemeral_retention_secs, 102);
        assert_eq!(limits.max_admission_sources, 103);
        assert_eq!(limits.record_retention_secs, 104);
        assert_eq!(limits.max_known_peers, 105);
        assert_eq!(limits.max_posts, 106);
        assert_eq!(limits.max_grants, 107);
        assert_eq!(limits.max_introductions, 108);
        assert_eq!(limits.max_social_events, 109);
    }

    #[test]
    fn zero_extreme_and_incoherent_limits_fail_closed() {
        let mut args = defaults();
        args.max_circuit_bytes = 0;
        assert!(ResourceLimits::try_from(args).is_err());

        let mut args = defaults();
        args.max_circuits_per_peer = args.max_circuits + 1;
        assert!(ResourceLimits::try_from(args).is_err());

        let mut args = defaults();
        args.max_circuit_duration_secs = args.reservation_duration_secs + 1;
        assert!(ResourceLimits::try_from(args).is_err());

        let mut args = defaults();
        args.max_storage_bytes = u64::MAX;
        assert!(ResourceLimits::try_from(args).is_err());

        let mut args = defaults();
        args.max_ephemeral_entries = 0;
        assert!(ResourceLimits::try_from(args).is_err());

        let mut args = defaults();
        args.record_retention_secs = MAX_RETENTION_SECS + 1;
        assert!(ResourceLimits::try_from(args).is_err());

        let mut args = defaults();
        args.max_social_events = (MAX_STATE_ENTRIES + 1) as usize;
        assert!(ResourceLimits::try_from(args).is_err());
    }
}
