// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// Turns live fixes from the device into the JSON form of the core `Fix`, which the
// wasm TripTracker accepts. The tracker does the filtering and the distance sum,
// so the same rules apply on the device, in the PWA and on the server.

import type { LiveFix } from "./ble";

/** Fix quality names in the core's order. The device sends the index of the name. */
export const FIX_QUALITY_NAMES = [
    "NoFix",
    "Gps",
    "Dgps",
    "Pps",
    "RtkFixed",
    "RtkFloat",
    "Estimated",
    "Manual",
    "Simulation",
] as const;

/** The core `Fix` as JSON. Field names match the core's serde output. */
export interface FixJson {
    coord: { lat: number; lon: number };
    time_ms: number;
    fix: (typeof FIX_QUALITY_NAMES)[number];
    hdop: null;
    speed_mps: number | null;
}

/** Converts a live fix to the core's JSON form. Returns `undefined` for a quality code it does not know. */
export function liveFixToJson(fix: LiveFix): string | undefined {
    const quality = FIX_QUALITY_NAMES[fix.fixQuality];
    if (quality === undefined) return undefined;
    const json: FixJson = {
        coord: { lat: fix.latDegrees, lon: fix.lonDegrees },
        time_ms: fix.timeMs,
        fix: quality,
        hdop: null,
        speed_mps: fix.speedMps,
    };
    return JSON.stringify(json);
}

/** The tracker's answer for one fix, as the wasm module returns it. */
export type TrackerDecision =
    | { status: "accepted"; metres: number; closed?: unknown }
    | { status: "rejected"; reason: string };

/** The part of the wasm TripTracker this module uses. */
export interface TrackerLike {
    push(fixJson: string): TrackerDecision;
    totalM(): number;
}

/**
 * Feeds one live fix to the tracker. A fix the tracker cannot read is reported as
 * rejected, not thrown, so one bad notification cannot stop the live display.
 */
export function feedTracker(
    tracker: TrackerLike,
    fix: LiveFix,
): { decision: TrackerDecision; totalM: number } {
    const json = liveFixToJson(fix);
    if (json === undefined) {
        return {
            decision: { status: "rejected", reason: "unknown fix quality" },
            totalM: tracker.totalM(),
        };
    }
    const decision = tracker.push(json);
    return { decision, totalM: tracker.totalM() };
}
