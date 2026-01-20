-- Migration 006: Bootstrap Nodes Configuration
-- Stores configured bootstrap/relay node addresses for P2P network connectivity

-- Bootstrap nodes table stores user-configured bootstrap node addresses
CREATE TABLE IF NOT EXISTS bootstrap_nodes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    address TEXT NOT NULL UNIQUE,           -- Multiaddress (e.g., /ip4/1.2.3.4/tcp/9000/p2p/12D3KooW...)
    name TEXT,                               -- Optional friendly name
    is_enabled INTEGER DEFAULT 1,            -- Whether to use this node
    priority INTEGER DEFAULT 0,              -- Order for fallback (lower = higher priority)
    is_default INTEGER DEFAULT 0,            -- Is this a built-in default node
    last_connected_at INTEGER,               -- Unix timestamp of last successful connection
    created_at INTEGER DEFAULT (strftime('%s', 'now')),
    updated_at INTEGER DEFAULT (strftime('%s', 'now'))
);

-- Index for quick lookup of enabled nodes by priority
CREATE INDEX IF NOT EXISTS idx_bootstrap_nodes_enabled ON bootstrap_nodes(is_enabled, priority);

-- Update schema version
UPDATE schema_version SET version = 6 WHERE id = 1;
