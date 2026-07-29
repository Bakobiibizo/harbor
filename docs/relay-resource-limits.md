# Relay resource limits

Harbor uses one validated resource-limit model for relay CLI arguments and `HARBOR_RELAY_*` environment variables. The AWS templates expose the same values as CloudFormation parameters, and the container defaults come from `relay-server/resource-limits.env`. Every value is finite, range-checked at startup, and logged as the structured `Effective relay resource limits` record before networking starts.

CLI arguments override environment values. Invalid, zero, excessive, or incoherent values stop startup. In particular, per-peer concurrency cannot exceed its global limit, circuit duration cannot exceed reservation duration, abuse dimensions cannot exceed the global abuse budget, and SQLite storage must remain between 16 MiB and 1 TiB.

| CLI argument | Environment variable | CloudFormation parameter | Default | Meaning |
| --- | --- | --- | ---: | --- |
| `--max-reservations` | `HARBOR_RELAY_MAX_RESERVATIONS` | `MaxReservations` | 128 | Concurrent reservations |
| `--max-reservations-per-peer` | `HARBOR_RELAY_MAX_RESERVATIONS_PER_PEER` | `MaxReservationsPerPeer` | 4 | Concurrent reservations per peer |
| `--max-circuits` | `HARBOR_RELAY_MAX_CIRCUITS` | `MaxCircuits` | 512 | Concurrent circuits |
| `--max-circuits-per-peer` | `HARBOR_RELAY_MAX_CIRCUITS_PER_PEER` | `MaxCircuitsPerPeer` | 16 | Concurrent circuits per peer |
| `--reservation-duration-secs` | `HARBOR_RELAY_RESERVATION_DURATION_SECS` | `ReservationDurationSecs` | 3600 | Reservation lease seconds |
| `--max-circuit-duration-secs` | `HARBOR_RELAY_MAX_CIRCUIT_DURATION_SECS` | `MaxCircuitDurationSecs` | 3600 | Maximum circuit lifetime seconds |
| `--max-circuit-bytes` | `HARBOR_RELAY_MAX_CIRCUIT_BYTES` | `MaxCircuitBytes` | 67108864 | Bytes forwarded by one circuit (64 MiB) |
| `--idle-connection-timeout-secs` | `HARBOR_RELAY_IDLE_CONNECTION_TIMEOUT_SECS` | `IdleConnectionTimeoutSecs` | 300 | Idle swarm connection seconds |
| `--reservation-admission-per-peer` | `HARBOR_RELAY_RESERVATION_ADMISSION_PER_PEER` | `ReservationAdmissionPerPeer` | 30 | Reservation admissions per peer/window |
| `--reservation-admission-per-ip` | `HARBOR_RELAY_RESERVATION_ADMISSION_PER_IP` | `ReservationAdmissionPerIp` | 60 | Reservation admissions per source IP/window |
| `--circuit-admission-per-peer` | `HARBOR_RELAY_CIRCUIT_ADMISSION_PER_PEER` | `CircuitAdmissionPerPeer` | 30 | Circuit admissions per peer/window |
| `--circuit-admission-per-ip` | `HARBOR_RELAY_CIRCUIT_ADMISSION_PER_IP` | `CircuitAdmissionPerIp` | 60 | Circuit admissions per source IP/window |
| `--admission-window-secs` | `HARBOR_RELAY_ADMISSION_WINDOW_SECS` | `AdmissionWindowSecs` | 120 | Reservation/circuit admission window seconds |
| `--rate-limit-max-requests` | `HARBOR_RELAY_RATE_LIMIT_MAX_REQUESTS` | `RateLimitMaxRequests` | 60 | Identity/board requests per peer/window |
| `--rate-limit-window-secs` | `HARBOR_RELAY_RATE_LIMIT_WINDOW_SECS` | `RateLimitWindowSecs` | 60 | Identity/board request window seconds |
| `--rate-limiter-cleanup-interval-secs` | `HARBOR_RELAY_RATE_LIMITER_CLEANUP_INTERVAL_SECS` | `RateLimiterCleanupIntervalSecs` | 300 | Stale counter cleanup seconds |
| `--abuse-peer-limit` | `HARBOR_RELAY_ABUSE_PEER_LIMIT` | `AbusePeerLimit` | 10 | Introduction admissions per peer/window |
| `--abuse-network-limit` | `HARBOR_RELAY_ABUSE_NETWORK_LIMIT` | `AbuseNetworkLimit` | 30 | Introduction admissions per network/window |
| `--abuse-target-limit` | `HARBOR_RELAY_ABUSE_TARGET_LIMIT` | `AbuseTargetLimit` | 20 | Introduction admissions per target/window |
| `--abuse-action-limit` | `HARBOR_RELAY_ABUSE_ACTION_LIMIT` | `AbuseActionLimit` | 100 | Introduction admissions per action/window |
| `--abuse-global-limit` | `HARBOR_RELAY_ABUSE_GLOBAL_LIMIT` | `AbuseGlobalLimit` | 1000 | Global introduction admissions/window |
| `--abuse-window-secs` | `HARBOR_RELAY_ABUSE_WINDOW_SECS` | `AbuseWindowSecs` | 60 | Introduction abuse window seconds |
| `--max-storage-bytes` | `HARBOR_RELAY_MAX_STORAGE_BYTES` | `MaxStorageBytes` | 1073741824 | Relay SQLite page budget (1 GiB) |
| `--max-ephemeral-entries` | `HARBOR_RELAY_MAX_EPHEMERAL_ENTRIES` | `MaxEphemeralEntries` | 10000 | Maximum entries in each bounded in-memory admission or replay collection |
| `--ephemeral-retention-secs` | `HARBOR_RELAY_EPHEMERAL_RETENTION_SECS` | `EphemeralRetentionSecs` | 600 | Maximum retention for expired in-memory admission and replay state |
| `--max-admission-sources` | `HARBOR_RELAY_MAX_ADMISSION_SOURCES` | `MaxAdmissionSources` | 4096 | Maximum concurrently tracked network-source admission buckets |
| `--record-retention-secs` | `HARBOR_RELAY_RECORD_RETENTION_SECS` | `RecordRetentionSecs` | 31536000 | Maximum retained age for relay-managed expirable records |
| `--max-known-peers` | `HARBOR_RELAY_MAX_KNOWN_PEERS` | `MaxKnownPeers` | 100000 | Maximum retained peer records |
| `--max-posts` | `HARBOR_RELAY_MAX_POSTS` | `MaxPosts` | 1000000 | Maximum retained wall and community post records |
| `--max-grants` | `HARBOR_RELAY_MAX_GRANTS` | `MaxGrants` | 500000 | Maximum retained wall-access grant records |
| `--max-introductions` | `HARBOR_RELAY_MAX_INTRODUCTIONS` | `MaxIntroductions` | 100000 | Maximum retained introduction envelope records |
| `--max-social-events` | `HARBOR_RELAY_MAX_SOCIAL_EVENTS` | `MaxSocialEvents` | 1000000 | Maximum retained social-event records |

`MaxStorageBytes` constrains SQLite's maximum page count and retained journal size. Cardinality limits provide an independent hard ceiling, while retention limits define when stale records become eligible for pruning. These limits do not include operating-system logs, backups, or EBS snapshots, which require separate retention policies. Every relay persists identity, introduction, and wall state; community mode adds board state to the same bounded database.

Run the cross-surface parity gate after changing any default or binding:

```bash
./infrastructure/tests/relay-resource-limit-parity.py
```
