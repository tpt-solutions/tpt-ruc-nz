// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// When the app syncs by itself. Pure, so the rules can be tested without a browser.
// None of this runs unless upload is on; the caller checks that first.

export const SYNC_SCHEDULES = ["manual", "open", "15m", "hourly", "daily"] as const;
export type SyncSchedule = (typeof SYNC_SCHEDULES)[number];

export const DEFAULT_SYNC_SCHEDULE: SyncSchedule = "open";

/** What woke the app up to consider a sync. */
export type SyncTrigger = "open" | "online" | "visible" | "tick";

/** Labels for the settings screen. */
export const SCHEDULE_LABELS: Record<SyncSchedule, string> = {
    manual: "Manual only (Sync now button)",
    open: "When the app opens or comes back online",
    "15m": "Every 15 minutes",
    hourly: "Every hour",
    daily: "Once a day",
};

const MINUTE_MS = 60_000;

export function isSyncSchedule(value: unknown): value is SyncSchedule {
    return typeof value === "string" && (SYNC_SCHEDULES as readonly string[]).includes(value);
}

/** The repeat interval, or null for schedules that do not repeat. */
export function scheduleIntervalMs(schedule: SyncSchedule): number | null {
    switch (schedule) {
        case "15m":
            return 15 * MINUTE_MS;
        case "hourly":
            return 60 * MINUTE_MS;
        case "daily":
            return 24 * 60 * MINUTE_MS;
        default:
            return null;
    }
}

/** True when the schedule repeats and the last attempt is at least one interval old. */
export function isSyncDue(schedule: SyncSchedule, lastAttemptMs: number, nowMs: number): boolean {
    const interval = scheduleIntervalMs(schedule);
    if (interval === null) return false;
    // A clock set back leaves lastAttemptMs in the future; treat that as due rather than stuck.
    return lastAttemptMs > nowMs || nowMs - lastAttemptMs >= interval;
}

/**
 * Whether to start a sync for this trigger.
 * - manual: never.
 * - open: on launch and when the browser comes back online.
 * - repeating schedules: on launch and on coming back online (the data is likely waiting),
 *   and on visibility changes and the page timer only when an interval has passed.
 */
export function shouldSyncOn(
    schedule: SyncSchedule,
    trigger: SyncTrigger,
    lastAttemptMs: number,
    nowMs: number,
): boolean {
    if (schedule === "manual") return false;
    if (trigger === "open" || trigger === "online") return true;
    return isSyncDue(schedule, lastAttemptMs, nowMs);
}
