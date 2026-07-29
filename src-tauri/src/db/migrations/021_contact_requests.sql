CREATE TABLE IF NOT EXISTS contact_requests (
    request_id TEXT PRIMARY KEY,
    peer_id TEXT NOT NULL,
    direction TEXT NOT NULL CHECK(direction IN ('incoming', 'outgoing')),
    display_name TEXT,
    public_key BLOB,
    x25519_public BLOB,
    avatar_hash TEXT,
    bio TEXT,
    status TEXT NOT NULL CHECK(status IN ('pending', 'review', 'accepted', 'declined', 'failed', 'revoked')),
    pending_action TEXT,
    error TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(peer_id, direction)
);
CREATE INDEX IF NOT EXISTS contact_requests_status
 ON contact_requests(status, direction, updated_at DESC);
UPDATE schema_version SET version = 21 WHERE id = 1;

