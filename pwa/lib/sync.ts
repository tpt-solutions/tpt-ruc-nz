// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// Uploads trips to the backend's `POST /v1/trips`. The engine is pure: the HTTP
// call, the wait between retries and the local store are passed in, so the retry
// and conflict rules can be tested without a browser.

import { localIsoDate } from "./csv";
import type { Trip, Vehicle } from "./db";

/** Trips per request. The backend accepts up to 500. */
export const SYNC_BATCH_SIZE = 100;
/** Attempts per request before giving up until the next sync. */
export const MAX_ATTEMPTS = 5;
const BASE_DELAY_MS = 1_000;
const MAX_DELAY_MS = 30_000;

/** A trip in the shape the backend expects. */
export interface UploadTrip {
    id: string;
    date: string;
    metres: number;
    vehicle: { rate_type: string; gross_kg: number; electric: boolean };
}

/** One trip's result, as the backend reports it. */
export interface ItemResult {
    id: string;
    status: "created" | "unchanged" | "rejected" | "conflict" | "not_stored";
    error?: string;
}

/** What one request to the backend came back with. */
export type UploadOutcome =
    | { kind: "ok"; results: ItemResult[] }
    | { kind: "conflict"; results: ItemResult[] }
    | { kind: "invalid"; results: ItemResult[] }
    | { kind: "retry"; retryAfterMs?: number; message: string }
    | { kind: "fatal"; message: string };

export interface SyncDeps {
    /** Backend origin, without a trailing slash. */
    baseUrl: string;
    apiKey: string;
    fetch: typeof fetch;
    /** Signs the request body. Omitted when the browser cannot sign. */
    sign?: (body: string) => Promise<string>;
    /** Waits between retries. Tests replace it so they do not wait. */
    sleep: (ms: number) => Promise<void>;
}

/** The local store the engine reads from and writes to. */
export interface SyncStore {
    listPending(): Promise<Trip[]>;
    loadVehicles(): Promise<Map<string, Vehicle>>;
    markSynced(trip: Trip, syncedMs: number): Promise<void>;
    markNotStored(trip: Trip, state: "conflict" | "rejected", message: string): Promise<void>;
}

/** The summary the sync screen shows after a run. */
export interface SyncReport {
    uploaded: number;
    conflicts: { id: string; message: string }[];
    rejected: { id: string; message: string }[];
    /** Trips still waiting to upload, after this run. */
    pending: number;
    /** Set when the run stopped early, for example because the key was refused. */
    error?: string;
}

/** The upload shape for one trip. Dates are the device's local day, as the charge is priced. */
export function toUploadTrip(trip: Trip, vehicle: Vehicle): UploadTrip {
    return {
        id: trip.id,
        date: localIsoDate(trip.startMs),
        metres: trip.distanceM,
        vehicle: {
            rate_type: vehicle.rateType,
            gross_kg: vehicle.grossKg,
            electric: vehicle.electric,
        },
    };
}

/** Wait before retry `attempt` (1 for the first retry): doubles each time, capped. */
export function backoffDelayMs(attempt: number): number {
    return Math.min(MAX_DELAY_MS, BASE_DELAY_MS * 2 ** (attempt - 1));
}

/** Reads a `Retry-After` header given in seconds. Returns undefined for anything else. */
function retryAfterMs(response: Response): number | undefined {
    const header = response.headers.get("Retry-After");
    if (header === null) return undefined;
    const seconds = Number(header);
    return Number.isFinite(seconds) && seconds >= 0 ? seconds * 1_000 : undefined;
}

async function readResults(response: Response): Promise<ItemResult[] | undefined> {
    try {
        const body = (await response.json()) as { results?: unknown };
        return Array.isArray(body.results) ? (body.results as ItemResult[]) : undefined;
    } catch {
        return undefined;
    }
}

/** Sends one batch, once. Transient failures come back as `retry`. */
export async function postTrips(deps: SyncDeps, trips: UploadTrip[]): Promise<UploadOutcome> {
    const body = JSON.stringify({ trips });
    const headers: Record<string, string> = {
        "Content-Type": "application/json",
        Authorization: `Bearer ${deps.apiKey}`,
    };
    if (deps.sign) headers["X-Device-Signature"] = await deps.sign(body);

    let response: Response;
    try {
        response = await deps.fetch(`${deps.baseUrl}/v1/trips`, {
            method: "POST",
            headers,
            body,
        });
    } catch {
        return { kind: "retry", message: "Could not reach the backend" };
    }

    if (response.status === 401) {
        return { kind: "fatal", message: "The backend refused the API key. Check it in Settings." };
    }
    if (response.status === 403) {
        return { kind: "fatal", message: "This API key is not allowed to upload trips." };
    }
    if (response.status === 429 || response.status >= 500) {
        return {
            kind: "retry",
            retryAfterMs: retryAfterMs(response),
            message: `The backend is busy (HTTP ${response.status})`,
        };
    }

    const results = await readResults(response);
    if (response.status === 200) {
        return results
            ? { kind: "ok", results }
            : { kind: "fatal", message: "The backend sent an unexpected response" };
    }
    if (response.status === 409 && results) return { kind: "conflict", results };
    if (response.status === 422 && results) return { kind: "invalid", results };
    return { kind: "fatal", message: `The backend answered HTTP ${response.status}` };
}

/** `postTrips`, retried with backoff while the failure is transient. */
async function sendWithRetries(deps: SyncDeps, trips: UploadTrip[]): Promise<UploadOutcome> {
    for (let attempt = 1; ; attempt++) {
        const outcome = await postTrips(deps, trips);
        if (outcome.kind !== "retry" || attempt >= MAX_ATTEMPTS) return outcome;
        await deps.sleep(outcome.retryAfterMs ?? backoffDelayMs(attempt));
    }
}

interface Sendable {
    trip: Trip;
    upload: UploadTrip;
}

/**
 * Uploads every pending trip. Trips the backend refuses are marked, not dropped,
 * and the rest of their batch is sent again, because the backend stores nothing
 * from a batch it refuses.
 */
export async function syncPending(
    deps: SyncDeps,
    store: SyncStore,
    nowMs: () => number = Date.now,
): Promise<SyncReport> {
    const report: SyncReport = { uploaded: 0, conflicts: [], rejected: [], pending: 0 };
    const pending = await store.listPending();
    const vehicles = await store.loadVehicles();

    // A trip whose vehicle was deleted cannot be priced, so it is reported and skipped.
    const sendable: Sendable[] = [];
    for (const trip of pending) {
        const vehicle = vehicles.get(trip.vehicleId);
        if (!vehicle) {
            const message = "its vehicle was deleted, so it cannot be uploaded";
            await store.markNotStored(trip, "rejected", message);
            report.rejected.push({ id: trip.id, message });
            continue;
        }
        sendable.push({ trip, upload: toUploadTrip(trip, vehicle) });
    }

    const stop = async (message: string): Promise<SyncReport> => {
        report.error = message;
        report.pending = (await store.listPending()).length;
        return report;
    };

    for (let start = 0; start < sendable.length; start += SYNC_BATCH_SIZE) {
        let remaining = sendable.slice(start, start + SYNC_BATCH_SIZE);
        while (remaining.length > 0) {
            const outcome = await sendWithRetries(
                deps,
                remaining.map((item) => item.upload),
            );
            if (outcome.kind === "retry" || outcome.kind === "fatal") {
                return stop(outcome.message);
            }
            if (outcome.kind === "ok") {
                for (const item of remaining) {
                    await store.markSynced(item.trip, nowMs());
                    report.uploaded++;
                }
                break;
            }

            // Conflict or invalid: mark the refused trips, then send the others again.
            const refused = new Map(
                outcome.results
                    .filter(
                        (result) => result.status === "conflict" || result.status === "rejected",
                    )
                    .map((result) => [result.id, result]),
            );
            if (refused.size === 0) {
                return stop("The backend refused a batch without saying which trips");
            }
            const next: Sendable[] = [];
            for (const item of remaining) {
                const result = refused.get(item.trip.id);
                if (!result) {
                    next.push(item);
                    continue;
                }
                const state = result.status === "conflict" ? "conflict" : "rejected";
                const message = result.error ?? "the backend did not store this trip";
                await store.markNotStored(item.trip, state, message);
                (state === "conflict" ? report.conflicts : report.rejected).push({
                    id: item.trip.id,
                    message,
                });
            }
            remaining = next;
        }
    }

    report.pending = (await store.listPending()).length;
    return report;
}
