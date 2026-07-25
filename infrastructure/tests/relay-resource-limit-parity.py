#!/usr/bin/env python3
"""Fail when relay resource defaults or bindings drift across production surfaces."""

from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
ENV_FILE = ROOT / "relay-server/resource-limits.env"
DOC = ROOT / "docs/relay-resource-limits.md"
TEMPLATES = (
    ROOT / "infrastructure/relay-cloudformation.yaml",
    ROOT / "infrastructure/community-relay-cloudformation.yaml",
    ROOT / "src-tauri/harbor-relay-cloudformation.yaml",
)

BINDINGS = {
    "HARBOR_RELAY_MAX_RESERVATIONS": ("MaxReservations", "--max-reservations"),
    "HARBOR_RELAY_MAX_RESERVATIONS_PER_PEER": ("MaxReservationsPerPeer", "--max-reservations-per-peer"),
    "HARBOR_RELAY_MAX_CIRCUITS": ("MaxCircuits", "--max-circuits"),
    "HARBOR_RELAY_MAX_CIRCUITS_PER_PEER": ("MaxCircuitsPerPeer", "--max-circuits-per-peer"),
    "HARBOR_RELAY_RESERVATION_DURATION_SECS": ("ReservationDurationSecs", "--reservation-duration-secs"),
    "HARBOR_RELAY_MAX_CIRCUIT_DURATION_SECS": ("MaxCircuitDurationSecs", "--max-circuit-duration-secs"),
    "HARBOR_RELAY_MAX_CIRCUIT_BYTES": ("MaxCircuitBytes", "--max-circuit-bytes"),
    "HARBOR_RELAY_IDLE_CONNECTION_TIMEOUT_SECS": ("IdleConnectionTimeoutSecs", "--idle-connection-timeout-secs"),
    "HARBOR_RELAY_RESERVATION_ADMISSION_PER_PEER": ("ReservationAdmissionPerPeer", "--reservation-admission-per-peer"),
    "HARBOR_RELAY_RESERVATION_ADMISSION_PER_IP": ("ReservationAdmissionPerIp", "--reservation-admission-per-ip"),
    "HARBOR_RELAY_CIRCUIT_ADMISSION_PER_PEER": ("CircuitAdmissionPerPeer", "--circuit-admission-per-peer"),
    "HARBOR_RELAY_CIRCUIT_ADMISSION_PER_IP": ("CircuitAdmissionPerIp", "--circuit-admission-per-ip"),
    "HARBOR_RELAY_ADMISSION_WINDOW_SECS": ("AdmissionWindowSecs", "--admission-window-secs"),
    "HARBOR_RELAY_RATE_LIMIT_MAX_REQUESTS": ("RateLimitMaxRequests", "--rate-limit-max-requests"),
    "HARBOR_RELAY_RATE_LIMIT_WINDOW_SECS": ("RateLimitWindowSecs", "--rate-limit-window-secs"),
    "HARBOR_RELAY_RATE_LIMITER_CLEANUP_INTERVAL_SECS": ("RateLimiterCleanupIntervalSecs", "--rate-limiter-cleanup-interval-secs"),
    "HARBOR_RELAY_ABUSE_PEER_LIMIT": ("AbusePeerLimit", "--abuse-peer-limit"),
    "HARBOR_RELAY_ABUSE_NETWORK_LIMIT": ("AbuseNetworkLimit", "--abuse-network-limit"),
    "HARBOR_RELAY_ABUSE_TARGET_LIMIT": ("AbuseTargetLimit", "--abuse-target-limit"),
    "HARBOR_RELAY_ABUSE_ACTION_LIMIT": ("AbuseActionLimit", "--abuse-action-limit"),
    "HARBOR_RELAY_ABUSE_GLOBAL_LIMIT": ("AbuseGlobalLimit", "--abuse-global-limit"),
    "HARBOR_RELAY_ABUSE_WINDOW_SECS": ("AbuseWindowSecs", "--abuse-window-secs"),
    "HARBOR_RELAY_MAX_STORAGE_BYTES": ("MaxStorageBytes", "--max-storage-bytes"),
    "HARBOR_RELAY_MAX_EPHEMERAL_ENTRIES": ("MaxEphemeralEntries", "--max-ephemeral-entries"),
    "HARBOR_RELAY_EPHEMERAL_RETENTION_SECS": ("EphemeralRetentionSecs", "--ephemeral-retention-secs"),
    "HARBOR_RELAY_MAX_ADMISSION_SOURCES": ("MaxAdmissionSources", "--max-admission-sources"),
    "HARBOR_RELAY_RECORD_RETENTION_SECS": ("RecordRetentionSecs", "--record-retention-secs"),
    "HARBOR_RELAY_MAX_KNOWN_PEERS": ("MaxKnownPeers", "--max-known-peers"),
    "HARBOR_RELAY_MAX_POSTS": ("MaxPosts", "--max-posts"),
    "HARBOR_RELAY_MAX_GRANTS": ("MaxGrants", "--max-grants"),
    "HARBOR_RELAY_MAX_INTRODUCTIONS": ("MaxIntroductions", "--max-introductions"),
    "HARBOR_RELAY_MAX_SOCIAL_EVENTS": ("MaxSocialEvents", "--max-social-events"),
}


def fail(message: str) -> None:
    print(f"relay resource parity failed: {message}", file=sys.stderr)
    raise SystemExit(1)


def parse_defaults() -> dict[str, str]:
    defaults: dict[str, str] = {}
    for line in ENV_FILE.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        name, value = line.split("=", 1)
        if name in defaults or not value.isdigit() or int(value) <= 0:
            fail(f"invalid finite default {line!r}")
        defaults[name] = value
    if defaults.keys() != BINDINGS.keys():
        fail("resource-limits.env names do not match the parity contract")
    return defaults


def parameter_block(template: str, name: str) -> str:
    match = re.search(rf"(?m)^  {re.escape(name)}:\s*$", template)
    if not match:
        fail(f"missing CloudFormation parameter {name}")
    following = re.search(r"(?m)^  [A-Za-z][A-Za-z0-9]+:\s*$", template[match.end():])
    end = match.end() + following.start() if following else len(template)
    return template[match.start():end]


def main() -> None:
    defaults = parse_defaults()
    docs = DOC.read_text(encoding="utf-8")

    for path in TEMPLATES:
        template = path.read_text(encoding="utf-8")
        for env_name, (parameter, flag) in BINDINGS.items():
            value = defaults[env_name]
            block = parameter_block(template, parameter)
            if not re.search(rf"(?m)^    Default:\s*['\"]?{re.escape(value)}['\"]?\s*$", block):
                fail(f"{path} default drift for {parameter}")
            if f"          - {parameter}" not in template:
                fail(f"{path} does not expose {parameter} in its parameter group")
            if f"Environment={env_name}=${{{parameter}}}" not in template:
                fail(f"{path} does not bind {parameter} to {env_name}")
            expected_doc = f"| `{flag}` | `{env_name}` | `{parameter}` | {value} |"
            if expected_doc not in docs:
                fail(f"documentation drift for {env_name}")

    compose = (ROOT / "relay-server/docker-compose.yml").read_text(encoding="utf-8")
    dockerfile = (ROOT / "relay-server/Dockerfile").read_text(encoding="utf-8")
    entrypoint = (ROOT / "relay-server/docker-entrypoint.sh").read_text(encoding="utf-8")
    if "./resource-limits.env" not in compose:
        fail("docker-compose does not load resource-limits.env")
    if "COPY resource-limits.env /etc/harbor-relay/resource-limits.env" not in dockerfile:
        fail("Dockerfile does not package the canonical container defaults")
    if "harbor-relay-entrypoint" not in dockerfile or "exec /usr/local/bin/harbor-relay" not in entrypoint:
        fail("container entrypoint does not validate/load defaults before relay startup")

    result = subprocess.run(
        [
            "cargo",
            "run",
            "--quiet",
            "--manifest-path",
            str(ROOT / "relay-server/Cargo.toml"),
            "--bin",
            "harbor-relay",
            "--",
            "--help",
        ],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode != 0:
        fail(f"relay --help failed: {result.stderr.strip()}")
    for env_name, (_, flag) in BINDINGS.items():
        if flag not in result.stdout or env_name not in result.stdout:
            fail(f"CLI/environment binding is not inspectable for {env_name}")

    print(f"relay resource parity passed for {len(BINDINGS)} finite limits across all surfaces")


if __name__ == "__main__":
    main()
