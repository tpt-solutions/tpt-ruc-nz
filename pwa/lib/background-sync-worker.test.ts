// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import { describe, expect, it, vi } from "vitest";
import { DEFAULT_SETTINGS, type AppSettings } from "./app-settings";
import { runBackgroundSync, type BackgroundDeps } from "./background-sync-worker";
import type { Trip, Vehicle } from "./db";
import type { SyncStore } from "./sync";

const VEHICLE = {
    id: "v1",
    name: "Truck",
    rateType: "H",
    grossKg: 12_000,
    electric: false,
} as unknown as Vehicle;

const TRIP = {
    id: "t1",
    vehicleId: "v1",
    startMs: Date.UTC(2026, 9, 1, 12),
    distanceM: 5_000,
} as unknown as Trip;

const ON: AppSettings = {
    ...DEFAULT_SETTINGS,
    syncEnabled: true,
    syncSchedule: "hourly",
    backendUrl: "http://backend.test",
    apiKey: "k".repeat(32),
};

function setup(settings: AppSettings, trips: Trip[] = [TRIP], status = 200) {
    const synced: string[] = [];
    const store: SyncStore = {
        listPending: async () => trips.filter((t) => !synced.includes(t.id)),
        loadVehicles: async () => new Map([[VEHICLE.id, VEHICLE]]),
        markSynced: async (trip) => void synced.push(trip.id),
        markNotStored: async () => {},
    };
    const fetchMock = vi.fn(async (_input: RequestInfo | URL, init?: RequestInit) => {
        const sent = JSON.parse(String(init?.body)).trips as { id: string }[];
        return new Response(
            JSON.stringify({ results: sent.map((t) => ({ id: t.id, status: "created" })) }),
            { status },
        );
    });
    const recordAttempt = vi.fn<BackgroundDeps["recordAttempt"]>(async () => {});
    const deps: BackgroundDeps = {
        readSettings: async () => settings,
        recordAttempt,
        loadKey: async () => undefined,
        signer: () => async () => ({ "X-Device-Signature": "sig" }),
        fetch: fetchMock as unknown as typeof fetch,
        sleep: async () => {},
        store,
        nowMs: () => 1_800_000_000_000,
    };
    return { deps, fetchMock, recordAttempt, synced };
}

describe("runBackgroundSync", () => {
    it("uploads waiting trips and records the attempt", async () => {
        const { deps, fetchMock, recordAttempt, synced } = setup(ON);
        const result = await runBackgroundSync(deps);
        expect(result.ran).toBe(true);
        expect(fetchMock).toHaveBeenCalledTimes(1);
        expect(fetchMock.mock.calls[0][0]).toBe("http://backend.test/v1/trips");
        expect(synced).toEqual(["t1"]);
        expect(recordAttempt).toHaveBeenCalledWith(1_800_000_000_000);
    });

    it("sends nothing when upload is off", async () => {
        const { deps, fetchMock } = setup({ ...ON, syncEnabled: false });
        expect(await runBackgroundSync(deps)).toEqual({ ran: false, reason: "upload off" });
        expect(fetchMock).not.toHaveBeenCalled();
    });

    it("sends nothing on the manual and on-open schedules", async () => {
        for (const syncSchedule of ["manual", "open"] as const) {
            const { deps, fetchMock } = setup({ ...ON, syncSchedule });
            expect(await runBackgroundSync(deps)).toEqual({ ran: false, reason: "not scheduled" });
            expect(fetchMock).not.toHaveBeenCalled();
        }
    });

    it("sends nothing without a backend address or key", async () => {
        const { deps, fetchMock } = setup({ ...ON, apiKey: "" });
        expect(await runBackgroundSync(deps)).toEqual({ ran: false, reason: "not set up" });
        expect(fetchMock).not.toHaveBeenCalled();
    });

    it("makes no request and records no attempt when nothing is waiting", async () => {
        const { deps, fetchMock, recordAttempt } = setup(ON, []);
        expect(await runBackgroundSync(deps)).toEqual({ ran: false, reason: "nothing waiting" });
        expect(fetchMock).not.toHaveBeenCalled();
        expect(recordAttempt).not.toHaveBeenCalled();
    });

    it("signs uploads when the device has a signing key", async () => {
        const { deps, fetchMock } = setup(ON);
        deps.loadKey = async () =>
            ({ id: "dk_1", privateKey: {} }) as unknown as Awaited<
                ReturnType<BackgroundDeps["loadKey"]>
            >;
        await runBackgroundSync(deps);
        const headers = fetchMock.mock.calls[0][1]?.headers as Record<string, string>;
        expect(headers["X-Device-Signature"]).toBe("sig");
    });

    it("reports a refused key in the report instead of throwing", async () => {
        const { deps } = setup(ON, [TRIP], 401);
        const result = await runBackgroundSync(deps);
        expect(result.ran).toBe(true);
        if (result.ran) expect(result.report.error).toContain("API key");
    });
});
