-- ============================================================
-- IDENTITY
-- ============================================================
CREATE TABLE IF NOT EXISTS local_identity (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    peer_id TEXT NOT NULL UNIQUE,
    public_key BLOB NOT NULL,
    x25519_public BLOB NOT NULL,
    private_key_encrypted BLOB NOT NULL,
    display_name TEXT NOT NULL,
    avatar_hash TEXT,
    bio TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- ============================================================
-- CONTACTS
-- ============================================================
CREATE TABLE IF NOT EXISTS contacts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    peer_id TEXT NOT NULL UNIQUE,
    public_key BLOB NOT NULL,
    x25519_public BLOB NOT NULL,
    display_name TEXT NOT NULL,
    avatar_hash TEXT,
    bio TEXT,
    is_blocked INTEGER DEFAULT 0,
    trust_level INTEGER DEFAULT 0,
    last_seen_at INTEGER,
    added_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_contacts_peer_id ON contacts(peer_id);

-- ============================================================
-- PERMISSIONS (event-sourced)
-- ============================================================
CREATE TABLE IF NOT EXISTS permission_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT NOT NULL UNIQUE,
    event_type TEXT NOT NULL,
    grant_id TEXT NOT NULL,
    issuer_peer_id TEXT NOT NULL,
    subject_peer_id TEXT NOT NULL,
    capability TEXT NOT NULL,
    scope_json TEXT,
    issued_at INTEGER,
    expires_at INTEGER,
    payload_cbor BLOB NOT NULL,
    signature BLOB NOT NULL,
    received_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_perm_events_grant ON permission_events(grant_id);
CREATE INDEX IF NOT EXISTS idx_perm_events_subject ON permission_events(subject_peer_id);

-- Materialized view of current permissions
CREATE TABLE IF NOT EXISTS permissions_current (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    grant_id TEXT NOT NULL UNIQUE,
    issuer_peer_id TEXT NOT NULL,
    subject_peer_id TEXT NOT NULL,
    capability TEXT NOT NULL,
    issued_at INTEGER NOT NULL,
    expires_at INTEGER,
    revoked_at INTEGER,
    payload_cbor BLOB NOT NULL,
    signature BLOB NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_perm_current_subject ON permissions_current(subject_peer_id, capability);
CREATE INDEX IF NOT EXISTS idx_perm_current_issuer ON permissions_current(issuer_peer_id, capability);

-- ============================================================
-- MESSAGES (event-sourced)
-- ============================================================
CREATE TABLE IF NOT EXISTS message_events (
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

CREATE INDEX IF NOT EXISTS idx_msg_events_message ON message_events(message_id);
CREATE INDEX IF NOT EXISTS idx_msg_events_conv ON message_events(conversation_id, timestamp);

-- Materialized messages for UI
CREATE TABLE IF NOT EXISTS messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    message_id TEXT NOT NULL UNIQUE,
    conversation_id TEXT NOT NULL,
    sender_peer_id TEXT NOT NULL,
    recipient_peer_id TEXT NOT NULL,
    content_encrypted BLOB NOT NULL,
    content_type TEXT DEFAULT 'text',
    reply_to_message_id TEXT,
    lamport_clock INTEGER NOT NULL,
    sent_at INTEGER NOT NULL,
    received_at INTEGER,
    delivered_at INTEGER,
    read_at INTEGER,
    status TEXT DEFAULT 'pending'
);

CREATE INDEX IF NOT EXISTS idx_messages_conv ON messages(conversation_id, sent_at);

-- ============================================================
-- POSTS (event-sourced)
-- ============================================================
CREATE TABLE IF NOT EXISTS post_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT NOT NULL UNIQUE,
    event_type TEXT NOT NULL,
    post_id TEXT NOT NULL,
    author_peer_id TEXT NOT NULL,
    lamport_clock INTEGER NOT NULL,
    timestamp INTEGER NOT NULL,
    payload_cbor BLOB,
    signature BLOB NOT NULL,
    received_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_post_events_post ON post_events(post_id);

-- Materialized posts for UI
CREATE TABLE IF NOT EXISTS posts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    post_id TEXT NOT NULL UNIQUE,
    author_peer_id TEXT NOT NULL,
    content_type TEXT NOT NULL,
    content_text TEXT,
    visibility TEXT DEFAULT 'contacts',
    lamport_clock INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER,
    is_local INTEGER DEFAULT 1,
    signature BLOB NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_posts_author ON posts(author_peer_id);
CREATE INDEX IF NOT EXISTS idx_posts_created ON posts(created_at DESC);

-- Post media (metadata only, files on disk)
CREATE TABLE IF NOT EXISTS post_media (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    post_id TEXT NOT NULL,
    media_hash TEXT NOT NULL,
    media_type TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    file_name TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    width INTEGER,
    height INTEGER,
    duration_seconds INTEGER,
    sort_order INTEGER DEFAULT 0,
    FOREIGN KEY (post_id) REFERENCES posts(post_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_post_media_post ON post_media(post_id);
CREATE INDEX IF NOT EXISTS idx_post_media_hash ON post_media(media_hash);

-- ============================================================
-- SYNC STATE
-- ============================================================
CREATE TABLE IF NOT EXISTS sync_state (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    peer_id TEXT NOT NULL,
    sync_type TEXT NOT NULL,
    last_sync_at INTEGER,
    last_lamport_clock INTEGER DEFAULT 0,
    UNIQUE(peer_id, sync_type)
);

-- Offline message queue
CREATE TABLE IF NOT EXISTS sync_queue (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    target_peer_id TEXT NOT NULL,
    item_type TEXT NOT NULL,
    item_id TEXT NOT NULL,
    payload_cbor BLOB NOT NULL,
    priority INTEGER DEFAULT 5,
    attempts INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL,
    next_attempt_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_sync_queue_next ON sync_queue(next_attempt_at);

-- ============================================================
-- CALLS
-- ============================================================
CREATE TABLE IF NOT EXISTS call_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    call_id TEXT NOT NULL UNIQUE,
    peer_id TEXT NOT NULL,
    direction TEXT NOT NULL,
    status TEXT NOT NULL,
    started_at INTEGER,
    ended_at INTEGER,
    duration_seconds INTEGER
);

-- ============================================================
-- SETTINGS
-- ============================================================
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

-- ============================================================
-- LAMPORT CLOCK
-- ============================================================
CREATE TABLE IF NOT EXISTS lamport_clock (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    current_value INTEGER NOT NULL DEFAULT 0
);

INSERT OR IGNORE INTO lamport_clock (id, current_value) VALUES (1, 0);

-- ============================================================
-- SCHEMA VERSION
-- ============================================================
CREATE TABLE IF NOT EXISTS schema_version (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    version INTEGER NOT NULL DEFAULT 1
);

INSERT OR IGNORE INTO schema_version (id, version) VALUES (1, 1);
