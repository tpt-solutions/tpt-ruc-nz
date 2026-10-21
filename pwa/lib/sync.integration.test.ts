// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// Runs the sync engine against a real backend process. It needs the backend
// binary built first (`cargo build -p tpt-ruc-backend` from the repository root).
// Without the binary the suite is skipped, so the unit tests still run anywhere.

import { spawn, type ChildProcess } from "node:child_process";
import { existsSync, mkdtempSync } from "node:fs";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import type { Trip, Vehicle } from "./db";
import { syncPending, type SyncStore } from "./sync";

const BINARY = resolve(
    "..",
    "target",
    "debug",
    process.platform === "win32" ? "tpt-ruc-backend.exe" : "tpt-ruc-backend",
);
const KEY = "integration-test-key-0123456789abcdef";
const vehicle: Vehicle = { id: "v1", name: "Truck", rateType: "1", grossKg: 3000, electric: false };

/** Picks a free local port, so parallel runs do not collide. */
function freePort(): Promise<number> {
    return new Promise((resolvePort, reject) => {
        const server = createServer();
        server.unref();
        server.on("error", reject);
        server.listen(0, "127.0.0.1", () => {
            const address = server.address();
            const port = typeof address === "object" && address ? address.port : 0;
            server.close(() => resolvePort(port));
        });
    });
}

async function waitForHealth(baseUrl: string): Promise<void> {
    const deadline = Date.now() + 15_000;
    while (Date.now() < deadline) {
        try {
            const response = await fetch(`${baseUrl}/healthz`);
            if (response.ok) return;
        } catch {
            // Not listening yet.
        }
        await new Promise((r) => setTimeout(r, 100));
    }
    throw new Error("the backend did not start");
}

/** An in-memory store that records what the engine wrote. */
function memoryStore(trips: Trip[]): SyncStore & { trips: Trip[] } {
    const store = {
        trips: [...trips],
        async listPending() {
            return store.trips.filter((t) => t.sync === undefined);
        },
        async loadVehicles() {
            return new Map([[vehicle.id, vehicle]]);
        },
        async markSynced(t: Trip, syncedMs: number) {
            store.trips = store.trips.map((x) =>
                x.id === t.id ? { ...x, sync: { state: "synced", syncedMs } } : x,
            );
        },
        async markNotStored(t: Trip, state: "conflict" | "rejected", message: string) {
            store.trips = store.trips.map((x) =>
                x.id === t.id ? { ...x, sync: { state, message } } : x,
            );
        },
    };
    return store;
}

const haveBinary = existsSync(BINARY);

describe.skipIf(!haveBinary)("sync against a real backend", () => {
    let child: ChildProcess | undefined;
    let baseUrl = "";

    beforeAll(async () => {
        const port = await freePort();
        baseUrl = `http://127.0.0.1:${port}`;
        const dataDir = mkdtempSync(join(tmpdir(), "tpt-ruc-pwa-it-"));
        child = spawn(BINARY, [], {
            env: {
                ...process.env,
                TPT_RUC_BIND: `127.0.0.1:${port}`,
                TPT_RUC_DATABASE_URL: `sqlite://${join(dataDir, "tpt.db")}`,
                TPT_RUC_API_KEY: KEY,
                RUST_LOG: "warn",
            },
            stdio: "ignore",
        });
        await waitForHealth(baseUrl);
    }, 30_000);

    afterAll(() => {
        child?.kill();
    });

    const deps = () => ({
        baseUrl,
        apiKey: KEY,
        fetch,
        sleep: async () => undefined,
    });

    const oct = (day: number) => new Date(2026, 9, day, 12).getTime();

    it("uploads pending trips and the server stores them with its own charge", async () => {
        const store = memoryStore([
            { id: "it-1", vehicleId: "v1", startMs: oct(1), endMs: oct(1) + 1, distanceM: 12_345 },
            { id: "it-2", vehicleId: "v1", startMs: oct(2), endMs: oct(2) + 1, distanceM: 1_000 },
        ]);
        const report = await syncPending(deps(), store);
        expect(report).toMatchObject({ uploaded: 2, pending: 0, conflicts: [], rejected: [] });

        const listed = await fetch(`${baseUrl}/v1/trips`, {
            headers: { Authorization: `Bearer ${KEY}` },
        });
        const body = (await listed.json()) as { trips: { id: string; billed_metres: number }[] };
        expect(body.trips.map((t) => t.id)).toEqual(["it-1", "it-2"]);
        expect(body.trips[0].billed_metres).toBe(12_345);
    });

    it("reports a changed resend as a conflict and leaves the stored trip alone", async () => {
        const store = memoryStore([
            { id: "it-1", vehicleId: "v1", startMs: oct(1), endMs: oct(1) + 1, distanceM: 99_999 },
        ]);
        const report = await syncPending(deps(), store);
        expect(report.conflicts).toHaveLength(1);
        expect(report.conflicts[0].id).toBe("it-1");
        expect(store.trips[0].sync?.state).toBe("conflict");

        const listed = await fetch(`${baseUrl}/v1/trips`, {
            headers: { Authorization: `Bearer ${KEY}` },
        });
        const body = (await listed.json()) as { trips: { id: string; metres: number }[] };
        expect(body.trips.find((t) => t.id === "it-1")?.metres).toBe(12_345);
    });

    it("reports a refused key without storing anything", async () => {
        const store = memoryStore([
            { id: "it-3", vehicleId: "v1", startMs: oct(3), endMs: oct(3) + 1, distanceM: 500 },
        ]);
        const report = await syncPending({ ...deps(), apiKey: "wrong-key" }, store);
        expect(report.error).toContain("API key");
        expect(report.pending).toBe(1);
        expect(store.trips[0].sync).toBeUndefined();
    });
});

describe.skipIf(haveBinary)("sync against a real backend", () => {
    it("is skipped because the backend binary is not built", () => {
        expect(haveBinary).toBe(false);
    });
});
