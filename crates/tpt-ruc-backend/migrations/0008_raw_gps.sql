-- SPDX-License-Identifier: MIT OR Apache-2.0
-- Copyright (c) 2026 TPT Solutions

-- Raw GPS points, stored only when the owner turns on TPT_RUC_ALLOW_RAW_GPS.
-- Summaries are the default, so this table stays empty on a server that does
-- not opt in. Points belong to the user who sent them, and deleting the user
-- deletes their points.
CREATE TABLE raw_points (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id     TEXT    NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    recorded_at TEXT    NOT NULL,
    lat         REAL    NOT NULL CHECK (lat >= -90 AND lat <= 90),
    lon         REAL    NOT NULL CHECK (lon >= -180 AND lon <= 180),
    speed_kmh   REAL,
    created_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX raw_points_by_user ON raw_points (user_id, recorded_at);
