ALTER TABLE relay_trust_keys ADD COLUMN sequence INTEGER NOT NULL DEFAULT 0;
ALTER TABLE relay_trust_keys ADD COLUMN compromise_from INTEGER;
ALTER TABLE relay_trust_keys ADD COLUMN rotation_cbor BLOB;

CREATE INDEX IF NOT EXISTS relay_trust_keys_active_sequence
  ON relay_trust_keys(relay, retired_at, sequence DESC);

UPDATE schema_version SET version = 20 WHERE id = 1;
