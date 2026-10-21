// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import init, {
    currentRateEffective,
    heavyLicenceExpiresOn,
    licenceRemainingKm as wasmLicenceRemainingKm,
    liability as wasmLiability,
} from "tpt-ruc-wasm";
import type { DistanceLicenceRecord, Trip, Vehicle } from "@/lib/db";

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
 * The date a heavy licence bought under the newest rate change expires, as
 * `YYYY-MM-DD`. `undefined` for vehicles at or under the heavy limit. The core
 * rule is one month after the rate change, so this is a date for heavy
 * licences bought before that change.
 */
export async function heavyLicenceExpiry(grossKg: number): Promise<string | undefined> {
    await loadRuc();
    const [year, month, day] = currentRateEffective().split("-").map(Number);
    return heavyLicenceExpiresOn(grossKg, year, month, day) ?? undefined;
}

/**
 * Kilometres left on a distance licence at the given odometer reading. Zero once the
 * licence distance is used up. This is an estimate: the core model is not verified
 * against NZTA guidance.
 */
export async function licenceRemainingKm(
    licence: DistanceLicenceRecord,
    odometerKm: number,
): Promise<number> {
    await loadRuc();
    return wasmLicenceRemainingKm(licence.startOdometerKm, licence.licensedKm, odometerKm);
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

/**
 * The rate table in force on `on`, as the wasm module names it. Read from a
 * liability calculation, so it always matches the rates the estimates use.
 */
export async function rateTableVersion(on: Date = new Date()): Promise<string> {
    await loadRuc();
    const value = wasmLiability(
        "1",
        3000,
        false,
        on.getFullYear(),
        on.getMonth() + 1,
        on.getDate(),
        1000,
    ) as { rateVersion: string };
    return value.rateVersion;
}
