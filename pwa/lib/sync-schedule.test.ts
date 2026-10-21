// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import { describe, expect, it } from "vitest";
import {
    SYNC_SCHEDULES,
    isSyncDue,
    isSyncSchedule,
    scheduleIntervalMs,
    shouldSyncOn,
} from "./sync-schedule";

const NOW = 1_800_000_000_000;
const MIN = 60_000;

describe("scheduleIntervalMs", () => {
    it("gives the interval for repeating schedules and null otherwise", () => {
        expect(scheduleIntervalMs("15m")).toBe(15 * MIN);
        expect(scheduleIntervalMs("hourly")).toBe(60 * MIN);
        expect(scheduleIntervalMs("daily")).toBe(24 * 60 * MIN);
        expect(scheduleIntervalMs("manual")).toBeNull();
        expect(scheduleIntervalMs("open")).toBeNull();
    });
});

describe("isSyncDue", () => {
    it("is due once a full interval has passed", () => {
        expect(isSyncDue("15m", NOW - 14 * MIN, NOW)).toBe(false);
        expect(isSyncDue("15m", NOW - 15 * MIN, NOW)).toBe(true);
        expect(isSyncDue("hourly", NOW - 59 * MIN, NOW)).toBe(false);
        expect(isSyncDue("daily", NOW - 25 * 60 * MIN, NOW)).toBe(true);
    });

    it("is due when the app has never synced", () => {
        expect(isSyncDue("hourly", 0, NOW)).toBe(true);
    });

    it("is due when the clock was set back past the last attempt", () => {
        expect(isSyncDue("hourly", NOW + 5 * MIN, NOW)).toBe(true);
    });

    it("is never due for schedules that do not repeat", () => {
        expect(isSyncDue("manual", 0, NOW)).toBe(false);
        expect(isSyncDue("open", 0, NOW)).toBe(false);
    });
});

describe("shouldSyncOn", () => {
    it("never syncs by itself on manual", () => {
        for (const trigger of ["open", "online", "visible", "tick"] as const) {
            expect(shouldSyncOn("manual", trigger, 0, NOW)).toBe(false);
        }
    });

    it("on open syncs at launch and when back online, not on visibility or the timer", () => {
        expect(shouldSyncOn("open", "open", NOW, NOW)).toBe(true);
        expect(shouldSyncOn("open", "online", NOW, NOW)).toBe(true);
        expect(shouldSyncOn("open", "visible", 0, NOW)).toBe(false);
        expect(shouldSyncOn("open", "tick", 0, NOW)).toBe(false);
    });

    it("repeating schedules sync at launch and online, and otherwise only when due", () => {
        expect(shouldSyncOn("hourly", "open", NOW, NOW)).toBe(true);
        expect(shouldSyncOn("hourly", "online", NOW, NOW)).toBe(true);
        expect(shouldSyncOn("hourly", "visible", NOW - 10 * MIN, NOW)).toBe(false);
        expect(shouldSyncOn("hourly", "visible", NOW - 61 * MIN, NOW)).toBe(true);
        expect(shouldSyncOn("15m", "tick", NOW - 16 * MIN, NOW)).toBe(true);
        expect(shouldSyncOn("15m", "tick", NOW - MIN, NOW)).toBe(false);
    });
});

describe("isSyncSchedule", () => {
    it("accepts the known schedules only", () => {
        for (const schedule of SYNC_SCHEDULES) expect(isSyncSchedule(schedule)).toBe(true);
        expect(isSyncSchedule("weekly")).toBe(false);
        expect(isSyncSchedule(15)).toBe(false);
        expect(isSyncSchedule(undefined)).toBe(false);
    });
});
