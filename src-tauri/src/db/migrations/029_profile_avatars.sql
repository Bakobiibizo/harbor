CREATE TABLE IF NOT EXISTS local_profile_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    revision INTEGER NOT NULL DEFAULT 0,
    avatar_mime_type TEXT,
    updated_at INTEGER NOT NULL
);

INSERT OR IGNORE INTO local_profile_state(id, revision, avatar_mime_type, updated_at)
VALUES (1, 0, NULL, strftime('%s','now'));

CREATE TABLE IF NOT EXISTS contact_profile_state (
    peer_id TEXT PRIMARY KEY,
    revision INTEGER NOT NULL DEFAULT 0,
    avatar_mime_type TEXT,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY(peer_id) REFERENCES contacts(peer_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS pending_contact_profiles (
    peer_id TEXT PRIMARY KEY,
    revision INTEGER NOT NULL,
    display_name TEXT NOT NULL,
    avatar_hash TEXT,
    avatar_mime_type TEXT,
    bio TEXT,
    received_at INTEGER NOT NULL,
    FOREIGN KEY(peer_id) REFERENCES contacts(peer_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_pending_contact_profiles_avatar
ON pending_contact_profiles(avatar_hash);

UPDATE schema_version SET version = 29 WHERE id = 1;
