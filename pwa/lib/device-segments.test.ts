// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import { describe, expect, it } from "vitest";
import { parseSegmentsJson, partitionNew, segmentKey } from "./device-segments";

const a = { startMs: 1000, endMs: 2000, distanceM: 10 };
const b = { startMs: 3000, endMs: 4000, distanceM: 20 };

describe("segmentKey", () => {
    it("identifies a segment by its start and end time", () => {
        expect(segmentKey(a)).toBe("1000-2000");
        expect(segmentKey({ startMs: a.startMs, endMs: a.endMs })).toBe("1000-2000");
    });
});

describe("partitionNew", () => {
    it("keeps only segments not already stored", () => {
        const existing = new Set([segmentKey(a)]);
        const { fresh, duplicates } = partitionNew(existing, [a, b]);
        expect(fresh.map((s) => s.id)).toEqual([segmentKey(b)]);
        expect(duplicates).toBe(1);
    });

    it("counts a segment repeated within one import as a duplicate", () => {
        const { fresh, duplicates } = partitionNew(new Set(), [a, a, b]);
        expect(fresh).toHaveLength(2);
        expect(duplicates).toBe(1);
    });

    it("stores nothing when every segment is already known", () => {
        const { fresh, duplicates } = partitionNew(new Set([segmentKey(a)]), [a]);
        expect(fresh).toEqual([]);
        expect(duplicates).toBe(1);
    });
});

describe("parseSegmentsJson", () => {
    it("reads an array of segments", () => {
        expect(parseSegmentsJson(JSON.stringify([a, b]))).toEqual([a, b]);
    });

    it("refuses a file that is not an array", () => {
        expect(() => parseSegmentsJson('{"startMs":1}')).toThrow("expected a JSON array");
    });

    it("refuses a segment with a missing or non-numeric field", () => {
        expect(() => parseSegmentsJson(JSON.stringify([{ startMs: 1, endMs: 2 }]))).toThrow(
            'invalid "distanceM"',
        );
        expect(() =>
            parseSegmentsJson(JSON.stringify([{ startMs: "1", endMs: 2, distanceM: 1 }])),
        ).toThrow('invalid "startMs"');
    });

    it("refuses a segment that ends before it starts", () => {
        expect(() =>
            parseSegmentsJson(JSON.stringify([{ startMs: 5, endMs: 2, distanceM: 1 }])),
        ).toThrow("ends before it starts");
    });
});
