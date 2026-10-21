// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// Page side of Periodic Background Sync: asks the browser to wake the service
// worker on the chosen schedule. This is an enhancement. Only Chromium has it,
// only for installed apps, and the browser decides the real timing, so the
// in-app schedule stays the baseline. The worker side is in ./background-sync-worker.ts.

import { scheduleIntervalMs, type SyncSchedule } from "./sync-schedule";

/** The tag the service worker listens for. Must match public/sw.js. */
export const PERIODIC_SYNC_TAG = "tpt-ruc-sync";

/** The part of the Periodic Background Sync API used here. It is not in the DOM typings. */
export interface PeriodicSyncManager {
    register(tag: string, options: { minInterval: number }): Promise<void>;
    unregister(tag: string): Promise<void>;
}

export interface BackgroundSyncRegistration {
    periodicSync?: PeriodicSyncManager;
}

/**
 * - `active`: registered with the browser.
 * - `inactive`: supported, but not wanted (upload off, or the schedule is manual or on open).
 * - `denied`: supported, but the browser has not allowed it (the app is not installed, or the
 *   permission was refused).
 * - `unsupported`: this browser or this page has no Periodic Background Sync.
 */
export type BackgroundSyncStatus = "active" | "inactive" | "denied" | "unsupported";

export interface BackgroundSyncEnv {
    /** The page's service worker registration, or undefined when there is none. */
    registration: BackgroundSyncRegistration | undefined;
    /** Reads the permission state, or throws when the browser does not know the permission. */
    permissionState: () => Promise<PermissionState>;
}

/** The smallest gap to ask the browser for, or null when background sync is not wanted. */
export function backgroundMinIntervalMs(schedule: SyncSchedule): number | null {
    return scheduleIntervalMs(schedule);
}

/**
 * Registers or removes the periodic sync to match the settings. Never throws:
 * a browser that refuses is reported as a status, not an error.
 */
export async function updatePeriodicSync(
    env: BackgroundSyncEnv,
    syncEnabled: boolean,
    schedule: SyncSchedule,
): Promise<BackgroundSyncStatus> {
    const manager = env.registration?.periodicSync;
    if (!manager) return "unsupported";

    const minInterval = backgroundMinIntervalMs(schedule);
    try {
        if (!syncEnabled || minInterval === null) {
            await manager.unregister(PERIODIC_SYNC_TAG);
            return "inactive";
        }
        let state: PermissionState;
        try {
            state = await env.permissionState();
        } catch {
            return "unsupported";
        }
        if (state !== "granted") {
            await manager.unregister(PERIODIC_SYNC_TAG);
            return "denied";
        }
        await manager.register(PERIODIC_SYNC_TAG, { minInterval });
        return "active";
    } catch {
        return "denied";
    }
}

/** The environment for the real page. Returns undefined registration outside a browser. */
export async function browserBackgroundEnv(): Promise<BackgroundSyncEnv> {
    const registration =
        typeof navigator !== "undefined" && "serviceWorker" in navigator
            ? await navigator.serviceWorker.getRegistration()
            : undefined;
    return {
        registration: registration as BackgroundSyncRegistration | undefined,
        permissionState: async () =>
            (
                await navigator.permissions.query({
                    name: "periodic-background-sync" as PermissionName,
                })
            ).state,
    };
}

/** One line for the sync screen. */
export function backgroundStatusText(status: BackgroundSyncStatus): string {
    switch (status) {
        case "active":
            return "Background upload is on. The browser decides when it runs, often less often than the schedule, and only while it allows this app to.";
        case "denied":
            return "Background upload is not allowed by this browser. It usually needs the app installed. Uploads still run while the app is open.";
        case "unsupported":
            return "This browser has no background upload. Uploads run while the app is open.";
        case "inactive":
            return "Background upload is off. Pick an interval schedule and turn upload on to use it.";
    }
}
