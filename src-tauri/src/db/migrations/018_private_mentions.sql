CREATE TABLE IF NOT EXISTS private_mentions (
  mention_id TEXT PRIMARY KEY,
  post_id TEXT NOT NULL,
  qualified_name TEXT NOT NULL,
  intent TEXT NOT NULL CHECK(intent IN ('notify','repost-request')),
  sender_peer_id TEXT NOT NULL,
  authorized_peer_id TEXT,
  claim_digest TEXT,
  preview TEXT NOT NULL,
  envelope_ciphertext BLOB NOT NULL,
  signature BLOB NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','accepted','declined','blocked')),
  created_at INTEGER NOT NULL,
  reviewed_at INTEGER
);
CREATE INDEX IF NOT EXISTS idx_private_mentions_status ON private_mentions(status, created_at DESC);
