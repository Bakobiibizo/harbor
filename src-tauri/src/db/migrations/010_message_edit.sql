-- Add edited_at column to messages table for tracking edited messages
ALTER TABLE messages ADD COLUMN edited_at INTEGER;

-- Update schema version
UPDATE schema_version SET version = 10 WHERE id = 1;
