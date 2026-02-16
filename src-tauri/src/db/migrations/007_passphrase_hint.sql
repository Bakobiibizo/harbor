-- Add passphrase hint column to local_identity table
-- This is an optional field to help users remember their passphrase

ALTER TABLE local_identity ADD COLUMN passphrase_hint TEXT;

-- Update schema version
UPDATE schema_version SET version = 7 WHERE id = 1;
