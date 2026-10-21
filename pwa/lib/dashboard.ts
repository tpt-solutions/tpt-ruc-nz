// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// Dashboard figures for one vehicle. The charge for each trip comes from the
// wasm estimate, so these functions only select and add up trips.

import type { Trip } from "./db";
import type { TripEstimate } from "./ruc";

/** The first instant of the local calendar month containing `nowMs`. */
export function startOfMonthMs(nowMs: number): number {
    const now = new Date(nowMs);
    return new Date(now.getFullYear(), now.getMonth(), 1).getTime();
}

/** Trips for `vehicleId` that started in the same local month as `nowMs`, oldest first. */
export function tripsThisMonth(trips: readonly Trip[], vehicleId: string, nowMs: number): Trip[] {
    const now = new Date(nowMs);
    const start = new Date(now.getFullYear(), now.getMonth(), 1).getTime();
    const next = new Date(now.getFullYear(), now.getMonth() + 1, 1).getTime();
    return trips
        .filter(
            (trip) => trip.vehicleId === vehicleId && trip.startMs >= start && trip.startMs < next,
        )
        .sort((a, b) => a.startMs - b.startMs);
}

/**
 * Whole calendar days from the local day of `nowMs` to the day named by
 * `isoDate` (`YYYY-MM-DD`). Zero on the same day, negative once it has passed.
 */
export function daysUntil(isoDate: string, nowMs: number): number {
    const [year, month, day] = isoDate.split("-").map(Number);
    const target = Date.UTC(year, month - 1, day);
    const now = new Date(nowMs);
    const today = Date.UTC(now.getFullYear(), now.getMonth(), now.getDate());
    return Math.round((target - today) / 86_400_000);
}

/** Totals for a set of trip estimates, in micro-dollars. */
export interface ChargeTotals {
    trips: number;
    distanceM: number;
    inclGstMicros: number;
    exclGstMicros: number;
    gstMicros: number;
}

/** Adds up trips and their estimates. Each estimate lines up with its trip by index. */
export function totalCharges(
    trips: readonly Trip[],
    estimates: readonly TripEstimate[],
): ChargeTotals {
    const totals: ChargeTotals = {
        trips: trips.length,
        distanceM: 0,
        inclGstMicros: 0,
        exclGstMicros: 0,
        gstMicros: 0,
    };
    trips.forEach((trip, index) => {
        totals.distanceM += trip.distanceM;
        const estimate = estimates[index];
        if (!estimate) return;
        totals.inclGstMicros += estimate.inclGstMicros;
        totals.exclGstMicros += estimate.exclGstMicros;
        totals.gstMicros += estimate.gstMicros;
    });
    return totals;
}
