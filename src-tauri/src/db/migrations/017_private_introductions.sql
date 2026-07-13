CREATE TABLE IF NOT EXISTS introduction_decisions (
    request_id TEXT PRIMARY KEY,
    requester_peer_id TEXT NOT NULL,
    requester_name TEXT NOT NULL,
    request_digest BLOB NOT NULL,
    decision TEXT NOT NULL CHECK(decision IN ('pending','approved','ignored','rejected','blocked')),
    received_at INTEGER NOT NULL,
    decided_at INTEGER
);
CREATE TABLE IF NOT EXISTS introduction_blocks (
    requester_peer_id TEXT PRIMARY KEY,
    requester_name TEXT NOT NULL,
    blocked_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS contact_capability_state (
    grant_id TEXT PRIMARY KEY,
    issuer_peer_id TEXT NOT NULL,
    subject_peer_id TEXT NOT NULL,
    capability TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK(revision > 0),
    issued_at INTEGER NOT NULL,
    expires_at INTEGER,
    revocation_id TEXT NOT NULL UNIQUE,
    revoked_at INTEGER,
    updated_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS contact_capability_active
 ON contact_capability_state(issuer_peer_id, subject_peer_id, capability, expires_at, revoked_at);
UPDATE schema_version SET version = 17 WHERE id = 1;
