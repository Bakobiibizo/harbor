BEGIN;

CREATE TABLE game_signing_requests (
    domain TEXT NOT NULL CHECK(domain IN ('harbor.games.auth.v1', 'harbor.game-package.v1')),
    request_id TEXT NOT NULL,
    callback_url TEXT NOT NULL,
    proof_json TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('pending', 'delivered')),
    created_at INTEGER NOT NULL,
    delivered_at INTEGER,
    PRIMARY KEY(domain, request_id)
);

CREATE INDEX idx_game_signing_delivery
    ON game_signing_requests(status, created_at);

UPDATE schema_version SET version = 31 WHERE id = 1;

COMMIT;
