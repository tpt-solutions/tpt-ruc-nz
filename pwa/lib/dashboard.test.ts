// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import { describe, expect, it } from "vitest";
import type { Trip } from "./db";
import { daysUntil, startOfMonthMs, totalCharges, tripsThisMonth } from "./dashboard";

const NOW = new Date(2026, 9, 21, 9).getTime(); // 21 October 2026, local time

function trip(id: string, vehicleId: string, date: Date, distanceM = 1_000): Trip {
    return { id, vehicleId, startMs: date.getTime(), endMs: date.getTime() + 1, distanceM };
}

describe("startOfMonthMs", () => {
    it("is midnight on the first of the local month", () => {
        expect(startOfMonthMs(NOW)).toBe(new Date(2026, 9, 1).getTime());
    });
});

describe("tripsThisMonth", () => {
    const trips = [
        trip("sep", "v1", new Date(2026, 8, 30, 23)),
        trip("oct-early", "v1", new Date(2026, 9, 1, 0, 5)),
        trip("oct-late", "v1", new Date(2026, 9, 20)),
        trip("oct-other-vehicle", "v2", new Date(2026, 9, 5)),
        trip("nov", "v1", new Date(2026, 10, 1)),
    ];

    it("keeps only this vehicle's trips from the current month, oldest first", () => {
        expect(tripsThisMonth(trips, "v1", NOW).map((t) => t.id)).toEqual([
            "oct-early",
            "oct-late",
        ]);
    });

    it("returns nothing for a vehicle with no trips this month", () => {
        expect(tripsThisMonth(trips, "v9", NOW)).toEqual([]);
    });
});

describe("totalCharges", () => {
    it("adds distance and the three charge parts, one estimate per trip", () => {
        const trips = [
            trip("a", "v1", new Date(2026, 9, 1), 1_500),
            trip("b", "v1", new Date(2026, 9, 2), 2_500),
        ];
        const totals = totalCharges(trips, [
            { inclGstMicros: 1_000, exclGstMicros: 870, gstMicros: 130, rateVersion: "r" },
            { inclGstMicros: 2_000, exclGstMicros: 1_739, gstMicros: 261, rateVersion: "r" },
        ]);
        expect(totals).toEqual({
            trips: 2,
            distanceM: 4_000,
            inclGstMicros: 3_000,
            exclGstMicros: 2_609,
            gstMicros: 391,
        });
    });

    it("counts a trip with no estimate in the distance but not the charge", () => {
        const totals = totalCharges([trip("a", "v1", new Date(2026, 9, 1), 500)], []);
        expect(totals).toEqual({
            trips: 1,
            distanceM: 500,
            inclGstMicros: 0,
            exclGstMicros: 0,
            gstMicros: 0,
        });
    });
});

describe("daysUntil", () => {
    const oct22 = new Date(2026, 9, 22, 15).getTime();

    it("counts whole calendar days, ignoring the time of day", () => {
        expect(daysUntil("2026-10-22", oct22)).toBe(0);
        expect(daysUntil("2026-10-23", oct22)).toBe(1);
        expect(daysUntil("2026-11-01", oct22)).toBe(10);
    });

    it("is negative once the date has passed", () => {
        expect(daysUntil("2026-09-06", oct22)).toBe(-46);
    });
});
