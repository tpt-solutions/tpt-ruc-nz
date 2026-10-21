// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import { describe, expect, it } from "vitest";
import { MIGRATIONS, SCHEMA_VERSION } from "./db";

/** Records the stores and indexes the migrations create, in order. */
function recordingDb() {
    const stores: string[] = [];
    const indexes: string[] = [];
    const db = {
        createObjectStore(name: string) {
            stores.push(name);
            return {
                createIndex(index: string) {
                    indexes.push(`${name}.${index}`);
                },
            };
        },
    };
    return { db: db as unknown as Parameters<(typeof MIGRATIONS)[number]>[0], stores, indexes };
}

describe("schema migrations", () => {
    it("creates every store the app uses, including device keys", () => {
        const { db, stores, indexes } = recordingDb();
        for (const migrate of MIGRATIONS) migrate(db);
        expect(stores.sort()).toEqual(
            ["deviceKeys", "deviceSegments", "segments", "settings", "trips", "vehicles"].sort(),
        );
        expect(indexes.sort()).toEqual(["segments.byTrip", "trips.byVehicle"].sort());
    });

    it("runs one migration per schema version, from the first version up", () => {
        expect(SCHEMA_VERSION).toBe(MIGRATIONS.length);
        expect(SCHEMA_VERSION).toBeGreaterThanOrEqual(2);
    });

    it("can be applied to an upgraded database from any earlier version", () => {
        // An install at version 1 gets only the later migrations, which must not
        // recreate stores that already exist.
        const { db, stores } = recordingDb();
        for (const migrate of MIGRATIONS.slice(1)) migrate(db);
        expect(stores).toEqual(["deviceSegments"]);
    });
});
