CREATE TABLE IF NOT EXISTS group_call_rooms (
    room_id TEXT PRIMARY KEY,
    creator_peer_id TEXT NOT NULL,
    topology TEXT NOT NULL,
    media_mode TEXT NOT NULL,
    roster_version INTEGER NOT NULL,
    participants_json TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('invited', 'active', 'left', 'terminated')),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_group_call_rooms_state_updated
    ON group_call_rooms(state, updated_at DESC);

CREATE TABLE IF NOT EXISTS group_call_nonces (
    room_id TEXT NOT NULL,
    sender_peer_id TEXT NOT NULL,
    nonce TEXT NOT NULL,
    received_at INTEGER NOT NULL,
    PRIMARY KEY (room_id, sender_peer_id, nonce)
);

UPDATE schema_version SET version = 15 WHERE id = 1;
