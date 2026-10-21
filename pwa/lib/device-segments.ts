// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import { getDb, type DeviceSegment } from "./db";
import { putTrip } from "./trips";

/** The fields a downloaded segment carries. */
export interface IncomingSegment {
    startMs: number;
    endMs: number;
    distanceM: number;
}

/** A segment's identity: its start and end time. The same segment always has the same key. */
export function segmentKey(segment: { startMs: number; endMs: number }): string {
    return `${segment.startMs}-${segment.endMs}`;
}

/**
 * Splits incoming segments into those not yet stored and a count of duplicates.
 * A segment repeated within `incoming` counts as a duplicate too.
 */
export function partitionNew(
    existing: ReadonlySet<string>,
    incoming: readonly IncomingSegment[],
): { fresh: DeviceSegment[]; duplicates: number } {
    const seen = new Set(existing);
    const fresh: DeviceSegment[] = [];
    let duplicates = 0;
    for (const segment of incoming) {
        const id = segmentKey(segment);
        if (seen.has(id)) {
            duplicates++;
            continue;
        }
        seen.add(id);
        fresh.push({ id, ...segment });
    }
    return { fresh, duplicates };
}

/**
 * Parses an import file. It must hold a JSON array of segment objects. Anything
 * else is rejected before the database is touched.
 */
export function parseSegmentsJson(text: string): IncomingSegment[] {
    const value: unknown = JSON.parse(text);
    if (!Array.isArray(value)) {
        throw new TypeError("expected a JSON array of segments");
    }
    return value.map((item, index) => {
        const record = item as Record<string, unknown>;
        const fields = ["startMs", "endMs", "distanceM"] as const;
        for (const field of fields) {
            const v = record?.[field];
            if (typeof v !== "number" || !Number.isFinite(v) || v < 0) {
                throw new TypeError(`segment ${index} has an invalid "${field}"`);
            }
        }
        const segment = item as IncomingSegment;
        if (segment.endMs < segment.startMs) {
            throw new TypeError(`segment ${index} ends before it starts`);
        }
        return {
            startMs: segment.startMs,
            endMs: segment.endMs,
            distanceM: segment.distanceM,
        };
    });
}

/** Stores the segments that are new, and reports how many were duplicates. */
export async function mergeDeviceSegments(
    incoming: readonly IncomingSegment[],
): Promise<{ added: number; duplicates: number }> {
    const db = await getDb();
    const existing = new Set((await db.getAllKeys("deviceSegments")).map(String));
    const { fresh, duplicates } = partitionNew(existing, incoming);
    if (fresh.length > 0) {
        const tx = db.transaction("deviceSegments", "readwrite");
        await Promise.all([...fresh.map((segment) => tx.store.put(segment)), tx.done]);
    }
    return { added: fresh.length, duplicates };
}

/** Lists stored device segments, oldest first. */
export async function listDeviceSegments(): Promise<DeviceSegment[]> {
    const db = await getDb();
    const segments = await db.getAll("deviceSegments");
    return segments.sort((a, b) => a.startMs - b.startMs);
}

/** Lists the stored segments not yet assigned to a vehicle, oldest first. */
export async function listUnassignedSegments(): Promise<DeviceSegment[]> {
    const segments = await listDeviceSegments();
    return segments.filter((segment) => segment.vehicleId === undefined);
}

/**
 * Makes a device segment a trip of `vehicleId`. The trip takes the segment's id, so
 * assigning the same segment again overwrites that trip instead of adding one.
 */
export async function assignSegment(segment: DeviceSegment, vehicleId: string): Promise<void> {
    await putTrip({
        id: segment.id,
        vehicleId,
        startMs: segment.startMs,
        endMs: segment.endMs,
        distanceM: segment.distanceM,
        source: "device",
    });
    const db = await getDb();
    await db.put("deviceSegments", { ...segment, vehicleId });
}
