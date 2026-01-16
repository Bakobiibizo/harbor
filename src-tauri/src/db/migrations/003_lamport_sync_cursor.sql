-- ============================================================
-- Migration 003: Lamport-based sync cursor
-- ============================================================
-- Replace timestamp-based sync with lamport-based cursor.
-- This ensures no events are missed due to clock skew between peers.

-- ============================================================
-- FIX 1: Replace sync_state with lamport cursor table
-- ============================================================
DROP TABLE IF EXISTS sync_state;

-- Sync cursor tracks lamport clock per author per peer
-- This allows efficient resumable sync without missing events
CREATE TABLE IF NOT EXISTS sync_cursors (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    -- Peer we're syncing content FROM
    source_peer_id TEXT NOT NULL,
    -- Type of content being synced
    sync_type TEXT NOT NULL,  -- 'posts', 'permissions'
    -- Author whose content we're tracking
    author_peer_id TEXT NOT NULL,
    -- Highest lamport clock seen from this author
    highest_lamport_clock INTEGER NOT NULL DEFAULT 0,
    -- When we last synced with this peer
    last_sync_at INTEGER NOT NULL,
    UNIQUE(source_peer_id, sync_type, author_peer_id)
);

CREATE INDEX IF NOT EXISTS idx_sync_cursors_source
    ON sync_cursors(source_peer_id, sync_type);

CREATE INDEX IF NOT EXISTS idx_sync_cursors_author
    ON sync_cursors(author_peer_id);

-- ============================================================
-- Update schema version
-- ============================================================
UPDATE schema_version SET version = 3 WHERE id = 1;
