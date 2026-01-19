-- Post likes/reactions table
-- Tracks which users have liked which posts

CREATE TABLE IF NOT EXISTS post_likes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    post_id TEXT NOT NULL,
    liker_peer_id TEXT NOT NULL,
    reaction_type TEXT DEFAULT 'like',  -- For future: 'like', 'love', 'laugh', etc.
    timestamp INTEGER NOT NULL,
    signature BLOB NOT NULL,
    created_at INTEGER DEFAULT (strftime('%s', 'now')),
    UNIQUE(post_id, liker_peer_id),
    FOREIGN KEY (post_id) REFERENCES posts(post_id) ON DELETE CASCADE
);

-- Index for quick lookup of likes by post
CREATE INDEX IF NOT EXISTS idx_post_likes_post_id ON post_likes(post_id);

-- Index for getting all posts a user has liked
CREATE INDEX IF NOT EXISTS idx_post_likes_liker ON post_likes(liker_peer_id);
