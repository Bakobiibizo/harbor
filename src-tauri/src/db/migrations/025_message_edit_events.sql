-- Harbor has no production message history yet. Purge the unsafe v1 materialized
-- messages and rebuild the table with mandatory v2 decryption metadata rather
-- than retaining a legacy decryption path.
DROP TABLE IF EXISTS message_edit_heads;
DROP TABLE IF EXISTS message_edit_events;
DROP TABLE IF EXISTS message_edit_revision_counters;
DROP TABLE IF EXISTS message_crypto_nonces;
DROP TABLE IF EXISTS received_nonces;
DROP TABLE IF EXISTS messages;
-- The materialized v1 messages are intentionally removed above, so their
-- standalone event rows must not survive as phantom identities either.
DROP TABLE IF EXISTS message_events;
CREATE TABLE message_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT NOT NULL UNIQUE,
    event_type TEXT NOT NULL,
    message_id TEXT NOT NULL,
    conversation_id TEXT NOT NULL,
    sender_peer_id TEXT NOT NULL,
    recipient_peer_id TEXT NOT NULL,
    lamport_clock INTEGER NOT NULL,
    timestamp INTEGER NOT NULL,
    payload_cbor BLOB,
    signature BLOB NOT NULL,
    received_at INTEGER NOT NULL
);
CREATE INDEX idx_msg_events_message ON message_events(message_id);
CREATE INDEX idx_msg_events_conv ON message_events(conversation_id, timestamp);
CREATE TABLE messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    protocol_version INTEGER NOT NULL CHECK (protocol_version = 2),
    event_id TEXT NOT NULL UNIQUE CHECK (length(event_id) > 0),
    message_id TEXT NOT NULL UNIQUE,
    conversation_id TEXT NOT NULL,
    sender_peer_id TEXT NOT NULL,
    recipient_peer_id TEXT NOT NULL,
    nonce_id BLOB NOT NULL CHECK (length(nonce_id) = 16),
    content_encrypted BLOB NOT NULL,
    content_type TEXT DEFAULT 'text',
    reply_to_message_id TEXT,
    nonce_counter INTEGER NOT NULL CHECK (nonce_counter > 0),
    lamport_clock INTEGER NOT NULL,
    sent_at INTEGER NOT NULL,
    received_at INTEGER,
    delivered_at INTEGER,
    read_at INTEGER,
    status TEXT DEFAULT 'pending',
    UNIQUE (sender_peer_id, recipient_peer_id, nonce_id)
);
CREATE INDEX idx_messages_conv ON messages(conversation_id, sent_at);

-- Never release a nonce/counter after history deletion. This ledger spans
-- creates and edits so uniqueness is enforced across both event kinds.
CREATE TABLE IF NOT EXISTS message_crypto_nonces (
    author_peer_id TEXT NOT NULL,
    recipient_peer_id TEXT NOT NULL,
    nonce_id BLOB NOT NULL CHECK (length(nonce_id) = 16),
    nonce_counter INTEGER NOT NULL CHECK (nonce_counter > 0),
    event_id TEXT NOT NULL UNIQUE,
    event_kind TEXT NOT NULL CHECK (event_kind IN ('create', 'edit')),
    recorded_at INTEGER NOT NULL,
    PRIMARY KEY (author_peer_id, recipient_peer_id, nonce_id),
    UNIQUE (author_peer_id, recipient_peer_id, nonce_counter)
);

-- Direct-message edits are immutable protocol-v2 events. The encrypted payload
-- remains bound to the original message parties, and a revision reservation is
-- committed before encryption so a failed attempt cannot reuse a revision.
CREATE TABLE IF NOT EXISTS message_edit_revision_counters (
    message_id TEXT PRIMARY KEY,
    author_peer_id TEXT NOT NULL,
    recipient_peer_id TEXT NOT NULL,
    last_reserved_revision INTEGER NOT NULL CHECK (last_reserved_revision > 0),
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS message_edit_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT NOT NULL UNIQUE,
    protocol_version INTEGER NOT NULL CHECK (protocol_version = 2),
    message_id TEXT NOT NULL,
    conversation_id TEXT NOT NULL,
    author_peer_id TEXT NOT NULL,
    recipient_peer_id TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK (revision > 0),
    nonce_id BLOB NOT NULL CHECK (length(nonce_id) = 16),
    nonce_counter INTEGER NOT NULL CHECK (nonce_counter > 0),
    lamport_clock INTEGER NOT NULL CHECK (lamport_clock > 0),
    encrypted_content BLOB NOT NULL CHECK (length(encrypted_content) > 0),
    signature BLOB NOT NULL CHECK (length(signature) > 0),
    timestamp INTEGER NOT NULL,
    received_at INTEGER NOT NULL,
    UNIQUE (message_id, revision),
    UNIQUE (author_peer_id, recipient_peer_id, nonce_id),
    FOREIGN KEY (message_id) REFERENCES messages(message_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_message_edit_events_message_revision
    ON message_edit_events(message_id, revision);
CREATE INDEX IF NOT EXISTS idx_message_edit_events_conversation
    ON message_edit_events(conversation_id, timestamp);

-- This is a materialized pointer only. Event content is never updated or
-- deleted when a newer edit arrives.
CREATE TABLE IF NOT EXISTS message_edit_heads (
    message_id TEXT PRIMARY KEY,
    event_id TEXT NOT NULL UNIQUE,
    author_peer_id TEXT NOT NULL,
    recipient_peer_id TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK (revision > 0),
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (message_id) REFERENCES messages(message_id) ON DELETE CASCADE,
    FOREIGN KEY (event_id) REFERENCES message_edit_events(event_id) ON DELETE CASCADE
);

UPDATE schema_version SET version = 25 WHERE id = 1;
