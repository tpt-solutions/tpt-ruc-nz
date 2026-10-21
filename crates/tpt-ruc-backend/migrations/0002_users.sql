-- SPDX-License-Identifier: MIT OR Apache-2.0
-- Copyright (c) 2026 TPT Solutions

-- Users and their roles. Each user holds a random token. Only a SHA-256 hash of
-- the token is stored, so a copy of the database cannot be used to sign in.
-- Roles: admin manages users and sees every trip, manager sees every trip and
-- runs reports, driver records and sees only their own trips.
CREATE TABLE users (
    id          TEXT PRIMARY KEY NOT NULL,
    name        TEXT NOT NULL,
    role        TEXT NOT NULL CHECK (role IN ('admin', 'manager', 'driver')),
    token_hash  TEXT NOT NULL UNIQUE,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Trips recorded with the owner key have no user. Deleting a user deletes their
-- trips too, so a deletion request removes everything about that person.
ALTER TABLE trips ADD COLUMN user_id TEXT REFERENCES users (id) ON DELETE CASCADE;

CREATE INDEX trips_by_user ON trips (user_id, trip_date);
