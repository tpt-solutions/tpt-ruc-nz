// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import { describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS, mergeSettings, normaliseBackendUrl } from "./app-settings";

describe("mergeSettings", () => {
    it("starts with sync off and no backend", () => {
        expect(mergeSettings(undefined)).toEqual(DEFAULT_SETTINGS);
        expect(DEFAULT_SETTINGS.syncEnabled).toBe(false);
        expect(DEFAULT_SETTINGS.backendUrl).toBe("");
    });

    it("keeps valid stored values", () => {
        expect(
            mergeSettings({ syncEnabled: true, backendUrl: "https://x.example", apiKey: "k" }),
        ).toEqual({
            units: "km",
            syncEnabled: true,
            backendUrl: "https://x.example",
            apiKey: "k",
            syncSchedule: "open",
            lastSyncAttemptMs: 0,
            deviceId: "",
            deviceKeyId: "",
            deviceBackendUrl: "",
            deviceSequence: 0,
        });
    });

    it("ignores values of the wrong type instead of trusting them", () => {
        expect(
            mergeSettings({
                syncEnabled: "yes",
                backendUrl: 5,
                apiKey: null,
                syncSchedule: "weekly",
                lastSyncAttemptMs: "soon",
            }),
        ).toEqual(DEFAULT_SETTINGS);
    });

    it("keeps a valid schedule and last attempt, and drops bad numbers", () => {
        const kept = mergeSettings({
            syncSchedule: "hourly",
            lastSyncAttemptMs: 1_700_000_000_000,
        });
        expect(kept.syncSchedule).toBe("hourly");
        expect(kept.lastSyncAttemptMs).toBe(1_700_000_000_000);
        expect(mergeSettings({ lastSyncAttemptMs: -5 }).lastSyncAttemptMs).toBe(0);
        expect(mergeSettings({ lastSyncAttemptMs: Infinity }).lastSyncAttemptMs).toBe(0);
    });

    it("defaults to syncing when the app opens", () => {
        expect(DEFAULT_SETTINGS.syncSchedule).toBe("open");
    });

    it("always reports kilometres", () => {
        expect(mergeSettings({ units: "miles" }).units).toBe("km");
    });
});

describe("normaliseBackendUrl", () => {
    it("accepts an origin and drops a trailing slash", () => {
        expect(normaliseBackendUrl(" http://localhost:8080/ ")).toBe("http://localhost:8080");
        expect(normaliseBackendUrl("https://api.example.nz")).toBe("https://api.example.nz");
    });

    it("treats a blank field as unset", () => {
        expect(normaliseBackendUrl("   ")).toBe("");
    });

    it("refuses other schemes, paths and nonsense", () => {
        expect(() => normaliseBackendUrl("ftp://host")).toThrow("http:// or https://");
        expect(() => normaliseBackendUrl("http://host/api")).toThrow("must not include a path");
        expect(() => normaliseBackendUrl("http://host?x=1")).toThrow("must not include");
        expect(() => normaliseBackendUrl("not a url")).toThrow("full address");
    });
});
