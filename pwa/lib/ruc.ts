// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import init, { liability as wasmLiability } from "tpt-ruc-wasm";
import type { Trip, Vehicle } from "@/lib/db";

/** The liability for one trip, in micro-dollars, as the wasm module returns it. */
export interface TripEstimate {
    inclGstMicros: number;
    exclGstMicros: number;
    gstMicros: number;
    rateVersion: string;
}

let ready: Promise<void> | undefined;

/** Loads the wasm module once. Later calls reuse the same load. */
export function loadRuc(): Promise<void> {
    ready ??= init().then(() => undefined);
    return ready;
}

/**
 * Prices a trip for its vehicle, using the NZTA table in force on the trip's date.
 * Rejects when the vehicle type is unknown or the date has no table.
 */
export async function estimateTrip(vehicle: Vehicle, trip: Trip): Promise<TripEstimate> {
    await loadRuc();
    const d = new Date(trip.startMs);
    const value = wasmLiability(
        vehicle.rateType,
        vehicle.grossKg,
        vehicle.electric,
        d.getFullYear(),
        d.getMonth() + 1,
        d.getDate(),
        trip.distanceM,
    ) as {
        inclGstMicros: number;
        exclGstMicros: number;
        gstMicros: number;
        rateVersion: string;
    };
    return {
        inclGstMicros: value.inclGstMicros,
        exclGstMicros: value.exclGstMicros,
        gstMicros: value.gstMicros,
        rateVersion: value.rateVersion,
    };
}
