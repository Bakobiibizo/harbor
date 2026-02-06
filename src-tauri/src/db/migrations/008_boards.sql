-- Migration 008: Community boards
-- Adds tables for relay-hosted community boards

CREATE TABLE IF NOT EXISTS relay_communities (
    relay_peer_id TEXT PRIMARY KEY,
    relay_address TEXT NOT NULL,
    community_name TEXT,
    joined_at INTEGER NOT NULL,
    last_sync_at INTEGER
);

CREATE TABLE IF NOT EXISTS boards (
    board_id TEXT NOT NULL,
    relay_peer_id TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    is_default INTEGER DEFAULT 0,
    cached_at INTEGER NOT NULL,
    PRIMARY KEY (board_id, relay_peer_id),
    FOREIGN KEY (relay_peer_id) REFERENCES relay_communities(relay_peer_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS board_posts (
    post_id TEXT NOT NULL,
    board_id TEXT NOT NULL,
    relay_peer_id TEXT NOT NULL,
    author_peer_id TEXT NOT NULL,
    author_display_name TEXT,
    content_type TEXT NOT NULL DEFAULT 'text',
    content_text TEXT,
    lamport_clock INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    deleted_at INTEGER,
    signature BLOB NOT NULL,
    cached_at INTEGER NOT NULL,
    PRIMARY KEY (post_id, relay_peer_id)
);

CREATE INDEX IF NOT EXISTS idx_board_posts_board ON board_posts(board_id, relay_peer_id, created_at DESC);

CREATE TABLE IF NOT EXISTS board_sync_cursors (
    relay_peer_id TEXT NOT NULL,
    board_id TEXT NOT NULL,
    last_post_timestamp INTEGER DEFAULT 0,
    PRIMARY KEY (relay_peer_id, board_id)
);

-- Update schema version
UPDATE schema_version SET version = 8 WHERE id = 1;
