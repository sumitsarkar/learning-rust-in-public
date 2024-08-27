-- Add migration script here
CREATE TABLE idempotency (
    user_id TEXT NOT NULL REFERENCES users(user_id),
    idempotency_key TEXT NOT NULL,
    response_status_code INTEGER NOT NULL,
    response_headers TEXT NOT NULL,
    response_body BLOB NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY(user_id, idempotency_key)
);