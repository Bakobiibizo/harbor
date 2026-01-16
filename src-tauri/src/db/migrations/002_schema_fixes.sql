-- ============================================================
-- Migration 002: Schema fixes based on review feedback
-- ============================================================

-- ============================================================
-- FIX 1: Per-author lamport clocks instead of global
-- ============================================================
DROP TABLE IF EXISTS lamport_clock;

CREATE TABLE IF NOT EXISTS lamport_clocks (
    author_peer_id TEXT PRIMARY KEY,
    current_value INTEGER NOT NULL DEFAULT 0
);

-- ============================================================
-- FIX 2: Peer keys table for discovered (non-contact) peers
-- ============================================================
CREATE TABLE IF NOT EXISTS peer_keys (
    peer_id TEXT PRIMARY KEY,
    ed25519_public BLOB NOT NULL,
    x25519_public BLOB NOT NULL,
    display_name TEXT,
    avatar_hash TEXT,
    bio TEXT,
    identity_version TEXT,  -- hash of cbor identity doc for caching
    first_seen_at INTEGER NOT NULL,
    last_seen_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_peer_keys_last_seen ON peer_keys(last_seen_at);

-- ============================================================
-- FIX 3: Add author_peer_id consistently to event tables
-- ============================================================

-- message_events already has sender_peer_id, add explicit author_peer_id
ALTER TABLE message_events ADD COLUMN author_peer_id TEXT;
-- For existing rows, set author_peer_id = sender_peer_id for 'sent' events
-- This will be handled in code for new entries

-- permission_events: add author_peer_id (the one who authored this event)
ALTER TABLE permission_events ADD COLUMN author_peer_id TEXT;
-- For request events: author = requester (subject_peer_id)
-- For grant/revoke events: author = issuer (issuer_peer_id)

-- ============================================================
-- FIX 4: Fix permission_events grant_id to allow NULL for requests
-- ============================================================
-- SQLite doesn't support modifying columns, so we create a new table
CREATE TABLE IF NOT EXISTS permission_events_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT NOT NULL UNIQUE,
    event_type TEXT NOT NULL,  -- 'request', 'grant', 'revoke'
    entity_id TEXT NOT NULL,   -- request_id for requests, grant_id for grant/revoke
    author_peer_id TEXT NOT NULL,
    issuer_peer_id TEXT,       -- NULL for request events
    subject_peer_id TEXT NOT NULL,
    capability TEXT NOT NULL,
    scope_json TEXT,
    lamport_clock INTEGER NOT NULL,
    issued_at INTEGER,
    expires_at INTEGER,
    payload_cbor BLOB NOT NULL,
    signature BLOB NOT NULL,
    received_at INTEGER NOT NULL
);

-- Migrate data (if any exists)
INSERT INTO permission_events_new (
    id, event_id, event_type, entity_id, author_peer_id,
    issuer_peer_id, subject_peer_id, capability, scope_json,
    lamport_clock, issued_at, expires_at, payload_cbor, signature, received_at
)
SELECT
    id, event_id, event_type, grant_id,
    CASE WHEN event_type = 'request' THEN subject_peer_id ELSE issuer_peer_id END,
    issuer_peer_id, subject_peer_id, capability, scope_json,
    0, -- default lamport_clock for migrated rows
    issued_at, expires_at, payload_cbor, signature, received_at
FROM permission_events;

DROP TABLE IF EXISTS permission_events;
ALTER TABLE permission_events_new RENAME TO permission_events;

CREATE INDEX IF NOT EXISTS idx_perm_events_entity ON permission_events(entity_id);
CREATE INDEX IF NOT EXISTS idx_perm_events_subject ON permission_events(subject_peer_id);
CREATE INDEX IF NOT EXISTS idx_perm_events_author ON permission_events(author_peer_id, lamport_clock);

-- ============================================================
-- FIX 5: Add deduplication constraint
-- ============================================================
CREATE UNIQUE INDEX IF NOT EXISTS idx_post_events_dedup
    ON post_events(author_peer_id, post_id, lamport_clock);

CREATE UNIQUE INDEX IF NOT EXISTS idx_msg_events_dedup
    ON message_events(author_peer_id, message_id, lamport_clock);

-- ============================================================
-- FIX 6: Per-conversation encryption counters (nonce management)
-- ============================================================
CREATE TABLE IF NOT EXISTS conversation_counters (
    conversation_id TEXT PRIMARY KEY,
    send_counter INTEGER NOT NULL DEFAULT 0,
    highest_received_counter INTEGER NOT NULL DEFAULT 0
);

-- Track received nonces to prevent replay
CREATE TABLE IF NOT EXISTS received_nonces (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id TEXT NOT NULL,
    sender_peer_id TEXT NOT NULL,
    nonce_counter INTEGER NOT NULL,
    received_at INTEGER NOT NULL,
    UNIQUE(conversation_id, sender_peer_id, nonce_counter)
);

CREATE INDEX IF NOT EXISTS idx_received_nonces_conv
    ON received_nonces(conversation_id, sender_peer_id);

-- ============================================================
-- FIX 7: Message events split - add ack_sender_peer_id for status events
-- ============================================================
ALTER TABLE message_events ADD COLUMN ack_sender_peer_id TEXT;

-- ============================================================
-- Update schema version
-- ============================================================
UPDATE schema_version SET version = 2 WHERE id = 1;
