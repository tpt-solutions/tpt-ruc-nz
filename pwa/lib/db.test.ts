import { describe, expect, it } from "vitest";
import { BACKUP_FORMAT, BACKUP_VERSION, MIGRATIONS, SCHEMA_VERSION, parseBackup } from "./db";

const validBackup = {
    format: BACKUP_FORMAT,
    version: BACKUP_VERSION,
    exportedAtMs: 1,
    vehicles: [],
    trips: [],
    segments: [],
    settings: [],
};

describe("schema versions", () => {
    it("has one migration per schema version", () => {
        expect(SCHEMA_VERSION).toBe(MIGRATIONS.length);
        expect(SCHEMA_VERSION).toBeGreaterThan(0);
    });
});

describe("parseBackup", () => {
    it("accepts a well-formed backup", () => {
        expect(parseBackup(validBackup)).toEqual(validBackup);
    });

    it("rejects non-objects", () => {
        expect(() => parseBackup(null)).toThrow(TypeError);
        expect(() => parseBackup("backup")).toThrow(TypeError);
    });

    it("rejects another app's file", () => {
        expect(() => parseBackup({ ...validBackup, format: "other" })).toThrow(
            "not a tpt-ruc backup",
        );
    });

    it("rejects an unknown version", () => {
        expect(() => parseBackup({ ...validBackup, version: 2 })).toThrow(
            "unsupported backup version",
        );
    });

    it("rejects a missing store array", () => {
        const withoutTrips = { ...validBackup, trips: undefined };
        expect(() => parseBackup(withoutTrips)).toThrow('"trips" must be an array');
    });

    it("accepts a version 1 backup written before device segments were added", () => {
        expect(parseBackup(validBackup).deviceSegments).toBeUndefined();
    });

    it("accepts device segments when present", () => {
        const withSegments = { ...validBackup, deviceSegments: [] };
        expect(parseBackup(withSegments).deviceSegments).toEqual([]);
    });

    it("rejects device segments that are not an array", () => {
        expect(() => parseBackup({ ...validBackup, deviceSegments: {} })).toThrow(
            '"deviceSegments" must be an array',
        );
    });
});
