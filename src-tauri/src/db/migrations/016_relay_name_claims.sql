CREATE TABLE IF NOT EXISTS relay_trust_keys (
 relay TEXT NOT NULL, key_id TEXT NOT NULL, public_key BLOB NOT NULL,
 not_before INTEGER NOT NULL, not_after INTEGER, retired_at INTEGER,
 PRIMARY KEY (relay, key_id)
);
CREATE TABLE IF NOT EXISTS relay_name_claims (
 qualified_name TEXT NOT NULL, local_name TEXT NOT NULL, relay TEXT NOT NULL,
 peer_id TEXT NOT NULL, sequence INTEGER NOT NULL CHECK(sequence > 0),
 claim_cbor BLOB NOT NULL, not_before INTEGER NOT NULL, not_after INTEGER NOT NULL,
 relay_key_id TEXT NOT NULL, status TEXT NOT NULL CHECK(status IN ('active','retired')),
 verified_at INTEGER NOT NULL, retired_at INTEGER,
 PRIMARY KEY (relay, local_name, sequence)
);
CREATE UNIQUE INDEX IF NOT EXISTS relay_name_claims_one_active
 ON relay_name_claims(relay, local_name) WHERE status = 'active';
CREATE INDEX IF NOT EXISTS relay_name_claims_peer ON relay_name_claims(peer_id, relay);
UPDATE schema_version SET version = 16 WHERE id = 1;
