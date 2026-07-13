CREATE TABLE IF NOT EXISTS identity_migration_state (
  peer_id TEXT PRIMARY KEY,
  mode TEXT NOT NULL CHECK(mode IN ('required','compatibility','verified')),
  updated_at INTEGER NOT NULL
);
