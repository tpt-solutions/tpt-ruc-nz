// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import { getDb, type Trip } from "./db";

/** Lists trips, newest first. */
export async function listTrips(): Promise<Trip[]> {
    const db = await getDb();
    const trips = await db.getAll("trips");
    return trips.sort((a, b) => b.startMs - a.startMs);
}

export async function putTrip(trip: Trip): Promise<void> {
    const db = await getDb();
    await db.put("trips", trip);
}

export async function deleteTrip(id: string): Promise<void> {
    const db = await getDb();
    await db.delete("trips", id);
}
