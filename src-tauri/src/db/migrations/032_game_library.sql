BEGIN;

CREATE TABLE game_installations (
    game_id TEXT PRIMARY KEY,
    version_id TEXT NOT NULL,
    title TEXT NOT NULL,
    creator_peer_id TEXT NOT NULL,
    archive_digest TEXT NOT NULL,
    package_digest TEXT NOT NULL,
    byte_length INTEGER NOT NULL CHECK(byte_length > 0),
    permissions_json TEXT NOT NULL,
    source TEXT NOT NULL CHECK(source IN ('manual', 'folder', 'store')),
    store_approval_json TEXT,
    installed_at INTEGER NOT NULL,
    last_played_at INTEGER,
    play_count INTEGER NOT NULL DEFAULT 0 CHECK(play_count >= 0)
);

CREATE UNIQUE INDEX idx_game_installations_archive
    ON game_installations(archive_digest);

CREATE TABLE game_library_settings (
    id INTEGER PRIMARY KEY CHECK(id = 1),
    discovery_folder TEXT,
    trusted_store_public_key TEXT
);

INSERT INTO game_library_settings(id, discovery_folder) VALUES(1, NULL);

CREATE TABLE game_saves (
    game_id TEXT NOT NULL,
    slot TEXT NOT NULL,
    data BLOB NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY(game_id, slot)
);

UPDATE schema_version SET version = 32 WHERE id = 1;

COMMIT;
