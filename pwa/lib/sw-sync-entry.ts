// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// Entry point bundled into public/sync-worker.js (see `npm run build:sw`), which
// public/sw.js loads with importScripts. It exposes one global, `tptRucSync`,
// that the `periodicsync` handler calls. Keep browser-only globals out of this
// file and its imports: a service worker has no `window` or `document`.

import { readAppSettings, writeAppSettings } from "./app-settings";
import { runBackgroundSync } from "./background-sync-worker";
import { loadDeviceKey, signText } from "./device-key";
import { indexedDbSyncStore } from "./sync-store";

declare const self: { tptRucSync?: () => Promise<unknown> };

self.tptRucSync = () =>
    runBackgroundSync({
        readSettings: readAppSettings,
        recordAttempt: async (nowMs) => {
            await writeAppSettings({ lastSyncAttemptMs: nowMs });
        },
        loadKey: loadDeviceKey,
        sign: (key, body) => signText(crypto.subtle, key, body),
        fetch: (input, init) => fetch(input, init),
        sleep: (ms) => new Promise<void>((resolve) => setTimeout(resolve, ms)),
        store: indexedDbSyncStore,
        nowMs: Date.now,
    });
