-- SPDX-License-Identifier: MIT OR Apache-2.0
-- Copyright (c) 2026 TPT Solutions

-- Devices with their own Ed25519 keys. A device belongs to one user. Uploads
-- signed by a device must carry a sequence number above last_sequence, which
-- is updated in the same transaction as the upload. Revoked devices are refused.
CREATE TABLE devices (
    id            TEXT    PRIMARY KEY NOT NULL,
    user_id       TEXT    NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    name          TEXT    NOT NULL,
    public_key    TEXT    NOT NULL CHECK (length(public_key) = 64),
    last_sequence INTEGER NOT NULL DEFAULT 0,
    revoked       INTEGER NOT NULL DEFAULT 0,
    created_at    TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX devices_by_user ON devices (user_id);
