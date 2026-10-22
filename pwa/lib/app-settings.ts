// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import { getSetting, setSetting } from "./db";
import { DEFAULT_SYNC_SCHEDULE, isSyncSchedule, type SyncSchedule } from "./sync-schedule";

/** Settings key the whole settings object is stored under in the settings store. */
export const APP_SETTINGS_KEY = "app";

/**
 * User settings. Sync is off until the user turns it on, so nothing leaves the
 * device by default. The API key is kept in this browser's database, and the
 * settings screen warns the user about that.
 */
export interface AppSettings {
    /** Distances are always shown in kilometres. Stored so a future unit can be added. */
    units: "km";
    /** Upload trips to the backend. Off by default. */
    syncEnabled: boolean;
    /** Base address of the backend, for example `http://localhost:8080`. Empty when unset. */
    backendUrl: string;
    /** The owner's or a user's API key, sent as a Bearer token. Empty when unset. */
    apiKey: string;
    /** When the app syncs by itself. Only used while `syncEnabled` is true. */
    syncSchedule: SyncSchedule;
    /** Epoch ms of the last sync attempt, or 0. Lets interval schedules survive a reload. */
    lastSyncAttemptMs: number;
    /** The backend's ID for this browser's signing key. Empty until registered. */
    deviceId: string;
    /** The local key the registration was made with. Empty until registered. */
    deviceKeyId: string;
    /** The backend the registration was made with. A new backend means a new registration. */
    deviceBackendUrl: string;
    /** The last sequence number used for a signed upload. The backend needs each one to rise. */
    deviceSequence: number;
}

export const DEFAULT_SETTINGS: AppSettings = {
    units: "km",
    syncEnabled: false,
    backendUrl: "",
    apiKey: "",
    syncSchedule: DEFAULT_SYNC_SCHEDULE,
    lastSyncAttemptMs: 0,
    deviceId: "",
    deviceKeyId: "",
    deviceBackendUrl: "",
    deviceSequence: 0,
};

/**
 * Combines stored settings with the defaults. Values of the wrong type are
 * ignored rather than trusted, so an old or damaged record cannot break the app.
 */
export function mergeSettings(stored: unknown): AppSettings {
    const record = (typeof stored === "object" && stored !== null ? stored : {}) as Record<
        string,
        unknown
    >;
    return {
        units: "km",
        syncEnabled: record.syncEnabled === true,
        backendUrl: typeof record.backendUrl === "string" ? record.backendUrl : "",
        apiKey: typeof record.apiKey === "string" ? record.apiKey : "",
        syncSchedule: isSyncSchedule(record.syncSchedule)
            ? record.syncSchedule
            : DEFAULT_SYNC_SCHEDULE,
        lastSyncAttemptMs:
            typeof record.lastSyncAttemptMs === "number" &&
            Number.isFinite(record.lastSyncAttemptMs) &&
            record.lastSyncAttemptMs > 0
                ? record.lastSyncAttemptMs
                : 0,
        deviceId: typeof record.deviceId === "string" ? record.deviceId : "",
        deviceKeyId: typeof record.deviceKeyId === "string" ? record.deviceKeyId : "",
        deviceBackendUrl:
            typeof record.deviceBackendUrl === "string" ? record.deviceBackendUrl : "",
        deviceSequence:
            typeof record.deviceSequence === "number" &&
            Number.isSafeInteger(record.deviceSequence) &&
            record.deviceSequence >= 0
                ? record.deviceSequence
                : 0,
    };
}

/**
 * Checks a backend address. Only `http:` and `https:` are accepted, and a path
 * is not, since the API lives at the root. Returns the normalised address with
 * no trailing slash, or an empty string when the field is blank.
 */
export function normaliseBackendUrl(text: string): string {
    const trimmed = text.trim();
    if (trimmed === "") return "";
    let url: URL;
    try {
        url = new URL(trimmed);
    } catch {
        throw new Error("Enter a full address, such as http://localhost:8080");
    }
    if (url.protocol !== "http:" && url.protocol !== "https:") {
        throw new Error("The backend address must start with http:// or https://");
    }
    if (url.pathname !== "/" || url.search !== "" || url.hash !== "") {
        throw new Error("The backend address must not include a path, query or fragment");
    }
    return url.origin;
}

export async function readAppSettings(): Promise<AppSettings> {
    return mergeSettings(await getSetting(APP_SETTINGS_KEY));
}

/** Saves the given fields, keeping the rest. Returns the full settings as saved. */
export async function writeAppSettings(patch: Partial<AppSettings>): Promise<AppSettings> {
    const next = mergeSettings({ ...(await readAppSettings()), ...patch });
    await setSetting(APP_SETTINGS_KEY, next);
    return next;
}
