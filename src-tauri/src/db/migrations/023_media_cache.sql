CREATE TABLE IF NOT EXISTS media_cache_settings (
    id INTEGER PRIMARY KEY CHECK(id = 1),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0, 1)),
    retention_seconds INTEGER NOT NULL DEFAULT 604800,
    max_bytes INTEGER NOT NULL DEFAULT 536870912,
    updated_at INTEGER NOT NULL
);

INSERT OR IGNORE INTO media_cache_settings
    (id, enabled, retention_seconds, max_bytes, updated_at)
VALUES (1, 1, 604800, 536870912, strftime('%s', 'now'));

CREATE TABLE IF NOT EXISTS media_cache_entries (
    media_hash TEXT PRIMARY KEY,
    observed_at INTEGER NOT NULL,
    last_accessed_at INTEGER NOT NULL,
    retain_until INTEGER NOT NULL,
    size_bytes INTEGER,
    cached_at INTEGER,
    FOREIGN KEY(media_hash) REFERENCES media_transfers(media_hash) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS media_cache_sources (
    media_hash TEXT NOT NULL,
    source_peer_id TEXT NOT NULL,
    observed_at INTEGER NOT NULL,
    retain_until INTEGER NOT NULL,
    PRIMARY KEY(media_hash, source_peer_id),
    FOREIGN KEY(media_hash) REFERENCES media_cache_entries(media_hash) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS media_local_pins (
    media_hash TEXT PRIMARY KEY,
    pinned_at INTEGER NOT NULL,
    FOREIGN KEY(media_hash) REFERENCES media_transfers(media_hash) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_media_cache_eviction
    ON media_cache_entries(retain_until ASC, last_accessed_at ASC, media_hash ASC);

CREATE INDEX IF NOT EXISTS idx_media_cache_source
    ON media_cache_sources(source_peer_id, media_hash);

UPDATE schema_version SET version = 23 WHERE id = 1;
