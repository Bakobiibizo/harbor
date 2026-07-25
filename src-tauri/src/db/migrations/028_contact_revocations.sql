BEGIN IMMEDIATE;

CREATE TABLE contact_revocation_tombstones (
  peer_id TEXT PRIMARY KEY,
  action TEXT NOT NULL CHECK(action IN ('blocked','removed')),
  revoked_at INTEGER NOT NULL
);

CREATE TABLE contact_revocation_outbox (
  event_id TEXT PRIMARY KEY,
  request_id TEXT NOT NULL UNIQUE,
  peer_id TEXT NOT NULL,
  action TEXT NOT NULL CHECK(action IN ('blocked','removed')),
  payload_cbor BLOB NOT NULL,
  state TEXT NOT NULL DEFAULT 'queued'
    CHECK(state IN ('queued','in_flight','delivered','failed')),
  attempt_count INTEGER NOT NULL DEFAULT 0,
  -- Security revocations must survive arbitrarily long offline periods.
  max_attempts INTEGER NOT NULL DEFAULT 2147483647,
  next_attempt_at INTEGER NOT NULL,
  attempt_deadline_at INTEGER,
  last_error TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  terminal_at INTEGER
);

CREATE INDEX contact_revocation_outbox_due
  ON contact_revocation_outbox(state,next_attempt_at,created_at);
CREATE INDEX contact_revocation_outbox_peer
  ON contact_revocation_outbox(peer_id,created_at DESC);

UPDATE schema_version SET version = 28 WHERE id = 1;

COMMIT;
