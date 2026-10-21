-- SPDX-License-Identifier: MIT OR Apache-2.0
-- Copyright (c) 2026 TPT Solutions

-- Passwords for users who sign in with a browser form. The column is NULL until
-- an administrator sets one, and a user without a password cannot sign in that
-- way. Only an Argon2id hash is stored.
ALTER TABLE users ADD COLUMN password_hash TEXT;

-- Sessions issued by a sign-in. Like user tokens, only a SHA-256 hash of the
-- session token is stored. Expiry is compared in SQL, so there is one clock.
-- Deleting a user deletes their sessions.
CREATE TABLE sessions (
    id          TEXT PRIMARY KEY NOT NULL,
    user_id     TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    token_hash  TEXT NOT NULL UNIQUE,
    expires_at  TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX sessions_by_user ON sessions (user_id);
