-- Post comments table
-- Stores comments on posts (local-first, future: P2P sync)

CREATE TABLE IF NOT EXISTS post_comments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    comment_id TEXT NOT NULL UNIQUE,
    post_id TEXT NOT NULL,
    author_peer_id TEXT NOT NULL,
    author_name TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    deleted_at INTEGER DEFAULT NULL,
    FOREIGN KEY (post_id) REFERENCES posts(post_id) ON DELETE CASCADE
);

-- Index for quick lookup of comments by post
CREATE INDEX IF NOT EXISTS idx_post_comments_post_id ON post_comments(post_id);

-- Index for getting all comments by a specific author
CREATE INDEX IF NOT EXISTS idx_post_comments_author ON post_comments(author_peer_id);

-- Update schema version
UPDATE schema_version SET version = 9 WHERE id = 1;
