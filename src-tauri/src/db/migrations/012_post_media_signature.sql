-- Migration 012: Bind post media metadata to signed integrity
-- Adds per-media metadata signatures and a uniqueness constraint so media
-- metadata can be verified and upserted idempotently by post/hash.

ALTER TABLE post_media ADD COLUMN signature BLOB NOT NULL DEFAULT X'';
DELETE FROM post_media
WHERE id NOT IN (
    SELECT MIN(id) FROM post_media GROUP BY post_id, media_hash
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_post_media_post_hash ON post_media(post_id, media_hash);

-- Update schema version
UPDATE schema_version SET version = 12 WHERE id = 1;
