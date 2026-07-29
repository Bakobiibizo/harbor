BEGIN IMMEDIATE;

CREATE TABLE call_signaling_replay (
  sequence INTEGER PRIMARY KEY AUTOINCREMENT,
  fingerprint TEXT NOT NULL UNIQUE,
  sender_peer_id TEXT NOT NULL,
  seen_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL
);

CREATE INDEX call_signaling_replay_expiry
  ON call_signaling_replay(expires_at);
CREATE INDEX call_signaling_replay_age
  ON call_signaling_replay(sequence);

UPDATE schema_version SET version = 30 WHERE id = 1;

COMMIT;
