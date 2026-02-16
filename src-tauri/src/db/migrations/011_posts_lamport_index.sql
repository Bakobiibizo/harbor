-- Migration 009: Add composite index for efficient cursor-based post queries
-- The content sync service queries posts by author_peer_id with lamport_clock
-- filtering. This composite index allows the database to satisfy the WHERE clause
-- and ORDER BY in a single index scan instead of a full table scan + sort.

CREATE INDEX IF NOT EXISTS idx_posts_author_lamport ON posts(author_peer_id, lamport_clock);

-- Update schema version
UPDATE schema_version SET version = 9 WHERE id = 1;
