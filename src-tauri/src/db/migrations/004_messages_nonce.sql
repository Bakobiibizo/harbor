-- Add nonce_counter column to messages table for replay protection
ALTER TABLE messages ADD COLUMN nonce_counter INTEGER DEFAULT 0;

-- Update schema version
UPDATE schema_version SET version = 4 WHERE id = 1;
