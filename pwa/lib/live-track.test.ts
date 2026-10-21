// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import type { LiveFix } from "./ble";
import { FIX_QUALITY_NAMES, feedTracker, liveFixToJson, type TrackerLike } from "./live-track";

const GOLDEN = "../crates/tpt-ruc-core/tests/golden/fix_v1.json";

function fix(overrides: Partial<LiveFix> = {}): LiveFix {
    return {
        version: 1,
        timeMs: 1_700_000_000_000,
        latDegrees: -41.2865,
        lonDegrees: 174.7762,
        speedMps: 12.5,
        fixQuality: 1,
        ...overrides,
    };
}

describe("liveFixToJson", () => {
    it("has the same field names and types as the core's version 1 Fix golden file", () => {
        const golden = JSON.parse(readFileSync(GOLDEN, "utf8")) as Record<string, unknown>;
        const json = JSON.parse(liveFixToJson(fix()) as string) as Record<string, unknown>;
        expect(Object.keys(json).sort()).toEqual(Object.keys(golden).sort());
        expect(json.coord).toEqual({ lat: -41.2865, lon: 174.7762 });
        expect(json.fix).toBe(golden.fix);
    });

    it("maps the firmware's quality codes to the core's names, in order", () => {
        expect(FIX_QUALITY_NAMES).toHaveLength(9);
        expect(FIX_QUALITY_NAMES[0]).toBe("NoFix");
        expect(FIX_QUALITY_NAMES[1]).toBe("Gps");
        expect(FIX_QUALITY_NAMES[8]).toBe("Simulation");
        const json = JSON.parse(liveFixToJson(fix({ fixQuality: 4 })) as string);
        expect(json.fix).toBe("RtkFixed");
    });

    it("returns undefined for a quality code it does not know", () => {
        expect(liveFixToJson(fix({ fixQuality: 42 }))).toBeUndefined();
    });

    it("keeps a missing speed as null", () => {
        const json = JSON.parse(liveFixToJson(fix({ speedMps: null })) as string);
        expect(json.speed_mps).toBeNull();
    });
});

describe("feedTracker", () => {
    /** A tracker that accepts everything and sums one metre per fix. */
    function countingTracker(): TrackerLike & { pushed: string[] } {
        const pushed: string[] = [];
        let total = 0;
        return {
            pushed,
            push(json: string) {
                pushed.push(json);
                total += 1;
                return { status: "accepted", metres: 1 };
            },
            totalM: () => total,
        };
    }

    it("passes the converted fix to the tracker and returns its running total", () => {
        const tracker = countingTracker();
        const result = feedTracker(tracker, fix());
        expect(tracker.pushed).toHaveLength(1);
        expect(result.decision.status).toBe("accepted");
        expect(result.totalM).toBe(1);
    });

    it("reports an unknown quality as rejected without calling the tracker", () => {
        const tracker = countingTracker();
        const result = feedTracker(tracker, fix({ fixQuality: 99 }));
        expect(tracker.pushed).toHaveLength(0);
        expect(result.decision).toEqual({ status: "rejected", reason: "unknown fix quality" });
    });
});
