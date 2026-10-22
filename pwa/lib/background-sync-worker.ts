// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// Service worker side of Periodic Background Sync. It runs the same sync engine
// as the page, with no page open. It does nothing unless the user turned upload
// on and picked an interval schedule, so nothing leaves the device by surprise.

import type { AppSettings } from "./app-settings";
import type { DeviceKey } from "./db";
import { scheduleIntervalMs } from "./sync-schedule";
import { syncPending, type SyncReport, type SyncStore } from "./sync";
import type { UploadSigner } from "./device-upload";

export interface BackgroundDeps {
    readSettings(): Promise<AppSettings>;
    recordAttempt(nowMs: number): Promise<void>;
    loadKey(): Promise<DeviceKey | undefined>;
    /** Builds the signer for this key and settings. Called only when the key can sign. */
    signer(key: DeviceKey, settings: AppSettings): UploadSigner;
    fetch: typeof fetch;
    sleep(ms: number): Promise<void>;
    store: SyncStore;
    nowMs(): number;
}

export type BackgroundResult =
    | { ran: false; reason: "upload off" | "not scheduled" | "not set up" | "nothing waiting" }
    | { ran: true; report: SyncReport };

/** Runs one background sync, or says why it did not. */
export async function runBackgroundSync(deps: BackgroundDeps): Promise<BackgroundResult> {
    const settings = await deps.readSettings();
    if (!settings.syncEnabled) return { ran: false, reason: "upload off" };
    if (scheduleIntervalMs(settings.syncSchedule) === null) {
        return { ran: false, reason: "not scheduled" };
    }
    if (!settings.backendUrl || !settings.apiKey) return { ran: false, reason: "not set up" };
    if ((await deps.store.listPending()).length === 0) {
        return { ran: false, reason: "nothing waiting" };
    }

    await deps.recordAttempt(deps.nowMs());
    const key = await deps.loadKey();
    const report = await syncPending(
        {
            baseUrl: settings.backendUrl,
            apiKey: settings.apiKey,
            fetch: deps.fetch,
            sign: key?.privateKey ? deps.signer(key, settings) : undefined,
            sleep: deps.sleep,
        },
        deps.store,
        deps.nowMs,
    );
    return { ran: true, report };
}
