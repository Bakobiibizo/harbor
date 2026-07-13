CREATE TABLE IF NOT EXISTS media_transfers (
    media_hash TEXT PRIMARY KEY,
    source_peer_id TEXT,
    media_type TEXT NOT NULL DEFAULT 'image',
    mime_type TEXT,
    file_name TEXT,
    total_bytes INTEGER,
    bytes_received INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'queued'
        CHECK(status IN ('queued', 'discovering', 'transferring', 'ready', 'unavailable', 'retrying', 'failed')),
    attempt_count INTEGER NOT NULL DEFAULT 0,
    error_code TEXT,
    error_message TEXT,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_media_transfers_status_updated
    ON media_transfers(status, updated_at DESC);

UPDATE schema_version SET version = 22 WHERE id = 1;
