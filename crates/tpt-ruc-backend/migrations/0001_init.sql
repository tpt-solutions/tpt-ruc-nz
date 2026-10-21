-- SPDX-License-Identifier: MIT OR Apache-2.0
-- Copyright (c) 2026 TPT Solutions

-- One row per trip summary. Client-supplied columns are stored as sent. The
-- charge columns are the server's own calculation, with the rate version that
-- produced it. Money is in integer micro-dollars, matching tpt-ruc-core.
CREATE TABLE trips (
    id              TEXT    PRIMARY KEY NOT NULL,
    trip_date       TEXT    NOT NULL,
    rate_type       TEXT    NOT NULL,
    gross_kg        INTEGER NOT NULL,
    electric        INTEGER NOT NULL,
    metres          REAL    NOT NULL,
    rate_version    TEXT    NOT NULL,
    billed_metres   INTEGER NOT NULL,
    incl_gst_micros INTEGER NOT NULL,
    excl_gst_micros INTEGER NOT NULL,
    gst_micros      INTEGER NOT NULL,
    received_at     TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX trips_by_date ON trips (trip_date, id);
