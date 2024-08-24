-- Add migration script here
CREATE TABLE
    IF NOT EXISTS sessions (
        id TEXT PRIMARY KEY NOT NULL,
        session TEXT NOT NULL,
        expires integer not null
    );