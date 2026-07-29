-- Durable storage for the exact signed and encoded direct-message wire event.
-- Delivery attempts may come and go, but the bytes never need to be rebuilt.
CREATE TABLE IF NOT EXISTS direct_message_outbox (
    event_id TEXT PRIMARY KEY CHECK (length(event_id) > 0),
    message_id TEXT NOT NULL CHECK (length(message_id) > 0),
    peer_id TEXT NOT NULL CHECK (length(peer_id) > 0),
    payload BLOB NOT NULL CHECK (length(payload) > 0),
    state TEXT NOT NULL DEFAULT 'queued'
        CHECK (state IN (
            'queued', 'in_flight', 'sent', 'delivered', 'read',
            'failed', 'canceled'
        )),
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    max_attempts INTEGER NOT NULL CHECK (max_attempts > 0),
    next_attempt_at INTEGER NOT NULL,
    attempt_deadline_at INTEGER,
    last_error TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    terminal_at INTEGER,
    CHECK (attempt_count <= max_attempts),
    CHECK (
        (state IN ('in_flight', 'sent') AND attempt_deadline_at IS NOT NULL)
        OR (state NOT IN ('in_flight', 'sent') AND attempt_deadline_at IS NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_direct_message_outbox_due
    ON direct_message_outbox(state, next_attempt_at, created_at);
CREATE INDEX IF NOT EXISTS idx_direct_message_outbox_deadline
    ON direct_message_outbox(state, attempt_deadline_at);
CREATE INDEX IF NOT EXISTS idx_direct_message_outbox_message
    ON direct_message_outbox(message_id);
CREATE INDEX IF NOT EXISTS idx_direct_message_outbox_terminal
    ON direct_message_outbox(terminal_at)
    WHERE terminal_at IS NOT NULL;

UPDATE schema_version SET version = 26 WHERE id = 1;
