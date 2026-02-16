-- Add edited_at column to messages table for tracking edited messages
-- Note: Column addition is handled in Rust code to check if it already exists

-- Update schema version
UPDATE schema_version SET version = 10 WHERE id = 1;
