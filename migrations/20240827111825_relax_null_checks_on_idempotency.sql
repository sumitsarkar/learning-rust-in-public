-- Add migration script here

-- Make `response fields` non-mandatory
-- We have to create a temp table because SQLite doesn't support
-- altering constraints on column
CREATE TABLE idempotency_temp (
    user_id TEXT NOT NULL REFERENCES users(user_id),
    idempotency_key TEXT NOT NULL,
    response_status_code INTEGER NULL,
    response_headers TEXT NULL,
    response_body BLOB NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY(user_id, idempotency_key)
);

INSERT INTO idempotency_temp
SELECT * FROM idempotency;

DROP TABLE idempotency;

ALTER Table idempotency_temp RENAME TO idempotency;
