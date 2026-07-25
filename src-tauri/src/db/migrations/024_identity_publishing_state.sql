BEGIN IMMEDIATE;

CREATE TABLE identity_publishing_state (
  peer_id TEXT PRIMARY KEY,
  mode TEXT NOT NULL CHECK(mode IN ('required','unverified','verified')),
  updated_at INTEGER NOT NULL
);

INSERT INTO identity_publishing_state(peer_id, mode, updated_at)
SELECT
  peer_id,
  CASE mode WHEN 'compatibility' THEN 'unverified' ELSE mode END,
  updated_at
FROM identity_migration_state;

DROP TABLE identity_migration_state;

UPDATE schema_version SET version = 24 WHERE id = 1;

COMMIT;
