-- Signed wall social events for syncable comments and reactions.
CREATE TABLE IF NOT EXISTS wall_social_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT NOT NULL UNIQUE,
    event_type TEXT NOT NULL,
    post_id TEXT NOT NULL,
    actor_peer_id TEXT NOT NULL,
    author_name TEXT,
    comment_id TEXT,
    content TEXT,
    reaction_type TEXT,
    timestamp INTEGER NOT NULL,
    payload_cbor BLOB NOT NULL,
    signature BLOB NOT NULL,
    received_at INTEGER NOT NULL,
    FOREIGN KEY (post_id) REFERENCES posts(post_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_wall_social_post ON wall_social_events(post_id, timestamp);
CREATE INDEX IF NOT EXISTS idx_wall_social_actor ON wall_social_events(actor_peer_id, timestamp);
CREATE INDEX IF NOT EXISTS idx_wall_social_comment ON wall_social_events(comment_id);

-- Bridge pre-existing local-only comments/likes into the social event log so
-- reloads and sync exporters can observe legacy user state. These legacy rows
-- intentionally retain empty payload/signature when no historical canonical
-- signed payload existed; all new production events are created through the
-- signed service path.
INSERT OR IGNORE INTO wall_social_events (
    event_id, event_type, post_id, actor_peer_id, author_name, comment_id,
    content, reaction_type, timestamp, payload_cbor, signature, received_at
)
SELECT
    'legacy-comment-' || comment_id,
    'legacy_comment_create',
    post_id,
    author_peer_id,
    author_name,
    comment_id,
    content,
    NULL,
    created_at,
    X'',
    X'',
    COALESCE(created_at, strftime('%s', 'now'))
FROM post_comments;

INSERT OR IGNORE INTO wall_social_events (
    event_id, event_type, post_id, actor_peer_id, author_name, comment_id,
    content, reaction_type, timestamp, payload_cbor, signature, received_at
)
SELECT
    'legacy-reaction-' || post_id || '-' || liker_peer_id || '-' || reaction_type,
    'legacy_reaction_add',
    post_id,
    liker_peer_id,
    NULL,
    NULL,
    NULL,
    reaction_type,
    timestamp,
    X'',
    signature,
    COALESCE(timestamp, strftime('%s', 'now'))
FROM post_likes;

UPDATE schema_version SET version = 14 WHERE id = 1;
