// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import { describe, expect, it } from "vitest";
import type { Trip, Vehicle } from "./db";
import {
    MAX_ATTEMPTS,
    SYNC_BATCH_SIZE,
    backoffDelayMs,
    postTrips,
    syncPending,
    toUploadTrip,
    type SyncDeps,
    type SyncStore,
    type UploadTrip,
} from "./sync";

const vehicle: Vehicle = { id: "v1", name: "Truck", rateType: "1", grossKg: 3000, electric: false };

function trip(id: string, startMs = Date.UTC(2026, 9, 1, 12)): Trip {
    return { id, vehicleId: "v1", startMs, endMs: startMs + 1, distanceM: 1_000 };
}

/** A store held in memory, recording what the engine wrote. */
function memoryStore(trips: Trip[], vehicles = [vehicle]): SyncStore & { trips: Trip[] } {
    const store = {
        trips: [...trips],
        async listPending() {
            return store.trips.filter((t) => t.sync === undefined);
        },
        async loadVehicles() {
            return new Map(vehicles.map((v) => [v.id, v]));
        },
        async markSynced(t: Trip, syncedMs: number) {
            replace(t, { sync: { state: "synced", syncedMs } });
        },
        async markNotStored(t: Trip, state: "conflict" | "rejected", message: string) {
            replace(t, { sync: { state, message } });
        },
    };
    function replace(t: Trip, patch: Partial<Trip>) {
        store.trips = store.trips.map((existing) =>
            existing.id === t.id ? { ...existing, ...patch } : existing,
        );
    }
    return store;
}

/** A fetch that answers each call with the next scripted response. */
function scripted(responses: (() => Response)[]) {
    const calls: { url: string; body: unknown; headers: Record<string, string> }[] = [];
    const fetchFn = (async (url: string, init: RequestInit) => {
        calls.push({
            url,
            body: JSON.parse(String(init.body)),
            headers: init.headers as Record<string, string>,
        });
        const next = responses.shift();
        if (!next) throw new Error("no scripted response left");
        return next();
    }) as unknown as typeof fetch;
    return { fetch: fetchFn, calls };
}

function json(status: number, body: unknown, headers: Record<string, string> = {}): Response {
    return new Response(JSON.stringify(body), {
        status,
        headers: { "Content-Type": "application/json", ...headers },
    });
}

function deps(fetchFn: typeof fetch, sleeps: number[] = []): SyncDeps {
    return {
        baseUrl: "http://backend.test",
        apiKey: "secret",
        fetch: fetchFn,
        sleep: async (ms) => {
            sleeps.push(ms);
        },
    };
}

const created = (id: string) => ({ id, status: "created" });

describe("toUploadTrip", () => {
    it("sends the local date, distance and vehicle in the backend's shape", () => {
        const upload = toUploadTrip(trip("t1", new Date(2026, 9, 1, 12).getTime()), vehicle);
        expect(upload).toEqual({
            id: "t1",
            date: "2026-10-01",
            metres: 1_000,
            vehicle: { rate_type: "1", gross_kg: 3000, electric: false },
        });
    });
});

describe("backoffDelayMs", () => {
    it("doubles each attempt and stops at 30 seconds", () => {
        expect(backoffDelayMs(1)).toBe(1_000);
        expect(backoffDelayMs(2)).toBe(2_000);
        expect(backoffDelayMs(3)).toBe(4_000);
        expect(backoffDelayMs(20)).toBe(30_000);
    });
});

describe("postTrips", () => {
    it("sends the key as a Bearer token and the trips as JSON", async () => {
        const { fetch: fetchFn, calls } = scripted([() => json(200, { results: [created("t1")] })]);
        const upload: UploadTrip = toUploadTrip(trip("t1"), vehicle);
        const outcome = await postTrips(deps(fetchFn), [upload]);
        expect(outcome.kind).toBe("ok");
        expect(calls[0].url).toBe("http://backend.test/v1/trips");
        expect(calls[0].headers.Authorization).toBe("Bearer secret");
        expect(calls[0].body).toEqual({ trips: [upload] });
    });

    it("adds a device signature when signing is available", async () => {
        const { fetch: fetchFn, calls } = scripted([() => json(200, { results: [] })]);
        await postTrips(
            { ...deps(fetchFn), sign: async () => ({ "X-Device-Signature": "sig" }) },
            [],
        );
        expect(calls[0].headers["X-Device-Signature"]).toBe("sig");
    });

    it("maps 401 and 403 to fatal errors with a clear message", async () => {
        const unauthorised = await postTrips(
            deps(scripted([() => json(401, { error: "unauthorized" })]).fetch),
            [],
        );
        expect(unauthorised).toMatchObject({ kind: "fatal" });
        expect(unauthorised.kind === "fatal" && unauthorised.message).toContain("API key");

        const forbidden = await postTrips(
            deps(scripted([() => json(403, { error: "forbidden" })]).fetch),
            [],
        );
        expect(forbidden.kind).toBe("fatal");
    });

    it("treats a network failure, 429 and 5xx as retryable, honouring Retry-After", async () => {
        const offline = (async () => {
            throw new TypeError("network down");
        }) as unknown as typeof fetch;
        expect((await postTrips(deps(offline), [])).kind).toBe("retry");

        const busy = await postTrips(
            deps(scripted([() => json(429, {}, { "Retry-After": "7" })]).fetch),
            [],
        );
        expect(busy).toMatchObject({ kind: "retry", retryAfterMs: 7_000 });

        const broken = await postTrips(deps(scripted([() => json(503, {})]).fetch), []);
        expect(broken).toMatchObject({ kind: "retry" });
    });

    it("reports a 409 or 422 with the per-trip results", async () => {
        const conflict = await postTrips(
            deps(
                scripted([
                    () =>
                        json(409, {
                            results: [{ id: "a", status: "conflict", error: "different" }],
                        }),
                ]).fetch,
            ),
            [],
        );
        expect(conflict).toMatchObject({ kind: "conflict" });
    });
});

describe("syncPending", () => {
    it("marks every accepted trip synced", async () => {
        const store = memoryStore([trip("a"), trip("b")]);
        const { fetch: fetchFn } = scripted([
            () => json(200, { results: [created("a"), created("b")] }),
        ]);
        const report = await syncPending(deps(fetchFn), store, () => 42);
        expect(report).toMatchObject({ uploaded: 2, pending: 0, conflicts: [], rejected: [] });
        expect(store.trips.every((t) => t.sync?.state === "synced" && t.sync.syncedMs === 42)).toBe(
            true,
        );
    });

    it("reports a conflict instead of dropping it, and still stores the others", async () => {
        const store = memoryStore([trip("a"), trip("b")]);
        const { fetch: fetchFn, calls } = scripted([
            () =>
                json(409, {
                    results: [
                        { id: "a", status: "not_stored" },
                        { id: "b", status: "conflict", error: "this ID is already stored" },
                    ],
                }),
            () => json(200, { results: [created("a")] }),
        ]);
        const report = await syncPending(deps(fetchFn), store);

        expect(report.conflicts).toEqual([{ id: "b", message: "this ID is already stored" }]);
        expect(report.uploaded).toBe(1);
        expect(store.trips.find((t) => t.id === "b")?.sync).toMatchObject({ state: "conflict" });
        // The second request carried only the trip the server had not refused.
        expect(calls[1].body).toEqual({
            trips: [toUploadTrip(trip("a"), vehicle)],
        });
    });

    it("marks a trip the backend rejects as invalid and sends the rest", async () => {
        const store = memoryStore([trip("a"), trip("bad")]);
        const { fetch: fetchFn } = scripted([
            () =>
                json(422, {
                    results: [
                        { id: "a", status: "not_stored" },
                        { id: "bad", status: "rejected", error: "no RUC rates in force" },
                    ],
                }),
            () => json(200, { results: [created("a")] }),
        ]);
        const report = await syncPending(deps(fetchFn), store);
        expect(report.rejected).toEqual([{ id: "bad", message: "no RUC rates in force" }]);
        expect(report.uploaded).toBe(1);
    });

    it("rejects trips whose vehicle was deleted without calling the backend", async () => {
        const store = memoryStore([trip("orphan")], []);
        const { fetch: fetchFn, calls } = scripted([]);
        const report = await syncPending(deps(fetchFn), store);
        expect(calls).toHaveLength(0);
        expect(report.rejected[0].id).toBe("orphan");
    });

    it("retries a busy backend with backoff, then succeeds", async () => {
        const store = memoryStore([trip("a")]);
        const sleeps: number[] = [];
        const { fetch: fetchFn } = scripted([
            () => json(503, {}),
            () => json(429, {}, { "Retry-After": "3" }),
            () => json(200, { results: [created("a")] }),
        ]);
        const report = await syncPending(deps(fetchFn, sleeps), store);
        expect(report.uploaded).toBe(1);
        expect(sleeps).toEqual([1_000, 3_000]);
    });

    it("gives up after the maximum attempts and keeps the trips pending", async () => {
        const store = memoryStore([trip("a")]);
        const responses = Array.from({ length: MAX_ATTEMPTS }, () => () => json(503, {}));
        const { fetch: fetchFn, calls } = scripted(responses);
        const report = await syncPending(deps(fetchFn), store);
        expect(calls).toHaveLength(MAX_ATTEMPTS);
        expect(report.error).toContain("busy");
        expect(report.pending).toBe(1);
        expect(store.trips[0].sync).toBeUndefined();
    });

    it("stops at once when the key is refused", async () => {
        const store = memoryStore([trip("a")]);
        const { fetch: fetchFn, calls } = scripted([() => json(401, {})]);
        const report = await syncPending(deps(fetchFn), store);
        expect(calls).toHaveLength(1);
        expect(report.error).toContain("API key");
        expect(report.pending).toBe(1);
    });

    it("splits large backlogs into batches", async () => {
        const many = Array.from({ length: SYNC_BATCH_SIZE + 5 }, (_, i) =>
            trip(`t${i}`, Date.UTC(2026, 9, 1) + i),
        );
        const store = memoryStore(many);
        const responses = [
            () => json(200, { results: many.slice(0, SYNC_BATCH_SIZE).map((t) => created(t.id)) }),
            () => json(200, { results: many.slice(SYNC_BATCH_SIZE).map((t) => created(t.id)) }),
        ];
        const { fetch: fetchFn, calls } = scripted(responses);
        const report = await syncPending(deps(fetchFn), store);
        expect(calls).toHaveLength(2);
        expect(report.uploaded).toBe(many.length);
        expect(report.pending).toBe(0);
    });
});
