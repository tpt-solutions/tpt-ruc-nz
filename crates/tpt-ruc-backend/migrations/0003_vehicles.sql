-- SPDX-License-Identifier: MIT OR Apache-2.0
-- Copyright (c) 2026 TPT Solutions

-- Vehicles in the fleet. Managers and admins maintain them. Trips may refer to
-- a vehicle, so the vehicle's rate fields are copied onto the trip when it is
-- recorded. Deleting a vehicle keeps its trips.
CREATE TABLE vehicles (
    id                TEXT    PRIMARY KEY NOT NULL,
    name              TEXT    NOT NULL,
    rate_type         TEXT    NOT NULL,
    gross_kg          INTEGER NOT NULL CHECK (gross_kg > 0),
    electric          INTEGER NOT NULL DEFAULT 0,
    -- When the current distance licence was bought, as YYYY-MM-DD. Optional.
    licence_bought_on TEXT,
    created_at        TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Which drivers may record trips against which vehicle. Removing a user or a
-- vehicle removes the assignment.
CREATE TABLE vehicle_drivers (
    vehicle_id TEXT NOT NULL REFERENCES vehicles (id) ON DELETE CASCADE,
    user_id    TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    PRIMARY KEY (vehicle_id, user_id)
);

CREATE INDEX vehicle_drivers_by_user ON vehicle_drivers (user_id);

ALTER TABLE trips ADD COLUMN vehicle_id TEXT REFERENCES vehicles (id) ON DELETE SET NULL;

CREATE INDEX trips_by_vehicle ON trips (vehicle_id, trip_date);
