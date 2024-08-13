
-- Add migration script here
-- Backfill `status` for historical entries
UPDATE subscriptions
    SET status = 'confirmed'
    WHERE status IS NULL;

-- Make `status` mandatory
-- We have to create a temp table because SQLite doesn't support
-- altering constraints on column
CREATE TABLE subscriptions_temp(
  id TEXT NOT NULL PRIMARY KEY,
  email TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL,
  subscribed_at TEXT NOT NULL,
  status TEXT NOT NULL
);

INSERT INTO subscriptions_temp
SELECT * FROM subscriptions;

DROP TABLE subscriptions;

ALTER Table subscriptions_temp RENAME TO subscriptions;
