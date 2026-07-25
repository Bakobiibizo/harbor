BEGIN IMMEDIATE;

ALTER TABLE posts ADD COLUMN relay_status TEXT NOT NULL DEFAULT 'relay_acknowledged'
  CHECK(relay_status IN ('local_pending','relay_acknowledged','conflict','failed'));

CREATE TABLE post_relay_outbox (
  event_id TEXT PRIMARY KEY REFERENCES post_events(event_id) ON DELETE CASCADE,
  post_id TEXT NOT NULL,
  mutation_type TEXT NOT NULL CHECK(mutation_type IN ('create','update','delete')),
  payload_cbor BLOB NOT NULL,
  state TEXT NOT NULL DEFAULT 'queued'
    CHECK(state IN ('queued','in_flight','acknowledged','conflict','failed')),
  attempt_count INTEGER NOT NULL DEFAULT 0 CHECK(attempt_count >= 0),
  max_attempts INTEGER NOT NULL DEFAULT 8 CHECK(max_attempts > 0),
  next_attempt_at INTEGER NOT NULL,
  attempt_deadline_at INTEGER,
  relay_peer_id TEXT,
  last_error TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  terminal_at INTEGER
);

CREATE INDEX post_relay_outbox_due
  ON post_relay_outbox(state, next_attempt_at, created_at);
CREATE INDEX post_relay_outbox_post
  ON post_relay_outbox(post_id, created_at DESC);
CREATE INDEX post_relay_outbox_deadline
  ON post_relay_outbox(state, attempt_deadline_at);

UPDATE schema_version SET version = 27 WHERE id = 1;

COMMIT;
