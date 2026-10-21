// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// IndexedDB side of the sync engine. The engine itself is in ./sync.ts.

import { getDb, listVehicles, type Trip } from "./db";
import type { SyncStore } from "./sync";
import { putTrip } from "./trips";

/** Trips that have never been sent, oldest first. Conflicts and rejections wait for a user action. */
export function isPending(trip: Trip): boolean {
    return trip.sync === undefined;
}

export const indexedDbSyncStore: SyncStore = {
    async listPending() {
        const db = await getDb();
        const trips = await db.getAll("trips");
        return trips.filter(isPending).sort((a, b) => a.startMs - b.startMs);
    },
    async loadVehicles() {
        const vehicles = await listVehicles();
        return new Map(vehicles.map((vehicle) => [vehicle.id, vehicle]));
    },
    async markSynced(trip, syncedMs) {
        await putTrip({ ...trip, sync: { state: "synced", syncedMs } });
    },
    async markNotStored(trip, state, message) {
        await putTrip({ ...trip, sync: { state, message } });
    },
};

/** Clears a conflict or rejection so the trip is sent again on the next sync. */
export async function requeueTrip(tripId: string): Promise<void> {
    const db = await getDb();
    const trip = await db.get("trips", tripId);
    if (!trip) return;
    await putTrip({ ...trip, sync: undefined });
}

/** Trips the backend refused, with the reason, for the sync screen. */
export async function listRefusedTrips(): Promise<Trip[]> {
    const db = await getDb();
    const trips = await db.getAll("trips");
    return trips.filter(
        (trip) => trip.sync?.state === "conflict" || trip.sync?.state === "rejected",
    );
}
