// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import { openDB, type DBSchema, type IDBPDatabase } from "idb";

export const DB_NAME = "tpt-ruc";

export interface Vehicle {
    id: string;
    name: string;
    /** NZTA RUC vehicle type code, for example "1". */
    rateType: string;
    /** RUC weight in kilograms. */
    grossKg: number;
    electric: boolean;
}

export interface Trip {
    id: string;
    vehicleId: string;
    startMs: number;
    endMs: number;
    /** Accumulated distance in metres. Rounded to billable km only when charged. */
    distanceM: number;
}

export interface Segment {
    id: string;
    tripId: string;
    startMs: number;
    endMs: number;
    distanceM: number;
}

export interface Setting {
    key: string;
    value: unknown;
}

export interface DeviceKey {
    id: string;
    /** Public key only. The private key stays in WebCrypto and is never stored here. */
    publicKeyBase64: string;
    createdMs: number;
}

export interface RucSchema extends DBSchema {
    vehicles: { key: string; value: Vehicle };
    trips: { key: string; value: Trip; indexes: { byVehicle: string } };
    segments: { key: string; value: Segment; indexes: { byTrip: string } };
    settings: { key: string; value: Setting };
    deviceKeys: { key: string; value: DeviceKey };
}

type Db = IDBPDatabase<RucSchema>;

/**
 * Schema migrations, in order. Migration `i` takes the database from version
 * `i` to `i + 1`. Never edit an entry that has shipped; append a new one.
 */
export const MIGRATIONS: ReadonlyArray<(db: Db) => void> = [
    (db) => {
        db.createObjectStore("vehicles", { keyPath: "id" });
        const trips = db.createObjectStore("trips", { keyPath: "id" });
        trips.createIndex("byVehicle", "vehicleId");
        const segments = db.createObjectStore("segments", { keyPath: "id" });
        segments.createIndex("byTrip", "tripId");
        db.createObjectStore("settings", { keyPath: "key" });
        db.createObjectStore("deviceKeys", { keyPath: "id" });
    },
];

export const SCHEMA_VERSION = MIGRATIONS.length;

/** Opens the local database, running any pending migrations. */
export function openRucDb(): Promise<Db> {
    return openDB<RucSchema>(DB_NAME, SCHEMA_VERSION, {
        upgrade(db, oldVersion) {
            for (let version = oldVersion; version < SCHEMA_VERSION; version++) {
                MIGRATIONS[version](db);
            }
        },
    });
}

/** Asks the browser to keep this origin's data under storage pressure. */
export async function requestPersistentStorage(): Promise<boolean> {
    try {
        if (!navigator.storage?.persist) return false;
        return await navigator.storage.persist();
    } catch {
        return false;
    }
}

// Backup format. Plain JSON, so a user can keep it and restore it on any device.

export const BACKUP_FORMAT = "tpt-ruc-backup";
export const BACKUP_VERSION = 1;

export interface Backup {
    format: typeof BACKUP_FORMAT;
    version: typeof BACKUP_VERSION;
    exportedAtMs: number;
    vehicles: Vehicle[];
    trips: Trip[];
    segments: Segment[];
    settings: Setting[];
}

/** Reads every store into a backup object. */
export async function exportBackup(db: Db, nowMs: number = Date.now()): Promise<Backup> {
    const [vehicles, trips, segments, settings] = await Promise.all([
        db.getAll("vehicles"),
        db.getAll("trips"),
        db.getAll("segments"),
        db.getAll("settings"),
    ]);
    return {
        format: BACKUP_FORMAT,
        version: BACKUP_VERSION,
        exportedAtMs: nowMs,
        vehicles,
        trips,
        segments,
        settings,
    };
}

/**
 * Checks that an unknown value is a backup this app can restore. Throws a
 * `TypeError` describing the first problem. It does not check every field.
 */
export function parseBackup(input: unknown): Backup {
    if (typeof input !== "object" || input === null) {
        throw new TypeError("backup must be a JSON object");
    }
    const candidate = input as Record<string, unknown>;
    if (candidate.format !== BACKUP_FORMAT) {
        throw new TypeError("not a tpt-ruc backup");
    }
    if (candidate.version !== BACKUP_VERSION) {
        throw new TypeError(`unsupported backup version: ${String(candidate.version)}`);
    }
    for (const key of ["vehicles", "trips", "segments", "settings"] as const) {
        if (!Array.isArray(candidate[key])) {
            throw new TypeError(`backup field "${key}" must be an array`);
        }
    }
    return input as Backup;
}

/** Writes a backup into the database. Records with the same key are replaced, so restoring twice is safe. */
export async function restoreBackup(db: Db, backup: Backup): Promise<void> {
    const tx = db.transaction(["vehicles", "trips", "segments", "settings"], "readwrite");
    await Promise.all([
        ...backup.vehicles.map((v) => tx.objectStore("vehicles").put(v)),
        ...backup.trips.map((t) => tx.objectStore("trips").put(t)),
        ...backup.segments.map((s) => tx.objectStore("segments").put(s)),
        ...backup.settings.map((s) => tx.objectStore("settings").put(s)),
        tx.done,
    ]);
}

// Shared database handle and typed helpers used by the UI.

let dbPromise: Promise<Db> | undefined;

/** Returns one shared connection, opened on first use. */
export function getDb(): Promise<Db> {
    dbPromise ??= openRucDb();
    return dbPromise;
}

export async function listVehicles(): Promise<Vehicle[]> {
    const db = await getDb();
    const vehicles = await db.getAll("vehicles");
    return vehicles.sort((a, b) => a.name.localeCompare(b.name));
}

export async function putVehicle(vehicle: Vehicle): Promise<void> {
    const db = await getDb();
    await db.put("vehicles", vehicle);
}

export async function deleteVehicle(id: string): Promise<void> {
    const db = await getDb();
    await db.delete("vehicles", id);
}

export async function getSetting<T>(key: string): Promise<T | undefined> {
    const db = await getDb();
    const setting = await db.get("settings", key);
    return setting?.value as T | undefined;
}

export async function setSetting(key: string, value: unknown): Promise<void> {
    const db = await getDb();
    await db.put("settings", { key, value });
}

/** Settings key recording that the user has accepted the Section 43 notice. */
export const SECTION_43_ACK = "section43Acknowledged";

/** Returns a new identifier. Falls back when `randomUUID` is unavailable. */
export function newId(): string {
    return (
        globalThis.crypto?.randomUUID?.() ??
        `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`
    );
}
