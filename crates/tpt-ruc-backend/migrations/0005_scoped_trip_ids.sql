-- SPDX-License-Identifier: MIT OR Apache-2.0
-- Copyright (c) 2026 TPT Solutions

-- Trip IDs are chosen by the client, so they are unique per owner, not across
-- all owners. Before, a second user who reused an ID got a conflict that
-- revealed another user's trip existed. The key is now (owner_key, id), where
-- owner_key is the user's ID, or 'owner' for trips recorded with the owner key.
CREATE TABLE trips_scoped (
    owner_key       TEXT    NOT NULL,
    id              TEXT    NOT NULL,
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
    received_at     TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    user_id         TEXT    REFERENCES users (id) ON DELETE CASCADE,
    vehicle_id      TEXT    REFERENCES vehicles (id) ON DELETE SET NULL,
    PRIMARY KEY (owner_key, id)
);

INSERT INTO trips_scoped (
    owner_key, id, trip_date, rate_type, gross_kg, electric, metres, rate_version,
    billed_metres, incl_gst_micros, excl_gst_micros, gst_micros, received_at,
    user_id, vehicle_id
)
SELECT
    COALESCE(user_id, 'owner'), id, trip_date, rate_type, gross_kg, electric, metres,
    rate_version, billed_metres, incl_gst_micros, excl_gst_micros, gst_micros,
    received_at, user_id, vehicle_id
FROM trips;

DROP TABLE trips;
ALTER TABLE trips_scoped RENAME TO trips;

CREATE INDEX trips_by_date ON trips (trip_date, id);
CREATE INDEX trips_by_user ON trips (user_id, trip_date);
CREATE INDEX trips_by_vehicle ON trips (vehicle_id, trip_date);
