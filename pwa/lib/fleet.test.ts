// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import { describe, expect, it } from "vitest";
import { fetchFleetSummary, parseFleetSummary } from "./fleet";

const row = {
    month: "2026-10",
    vehicle_id: "v1",
    vehicle_name: "Truck",
    rate_type: "1",
    trips: 2,
    billed_km: "12.345",
    charge_excl_gst_nzd: "0.82",
    gst_nzd: "0.12",
    charge_incl_gst_nzd: "0.94",
};

describe("parseFleetSummary", () => {
    it("maps the backend's fields to the client's names", () => {
        const [parsed] = parseFleetSummary({ rows: [row] });
        expect(parsed).toEqual({
            month: "2026-10",
            vehicleId: "v1",
            vehicleName: "Truck",
            rateType: "1",
            trips: 2,
            billedKm: "12.345",
            chargeExclGstNzd: "0.82",
            gstNzd: "0.12",
            chargeInclGstNzd: "0.94",
        });
    });

    it("accepts trips recorded with an inline vehicle, which have no vehicle id", () => {
        const [parsed] = parseFleetSummary({
            rows: [{ ...row, vehicle_id: null, vehicle_name: null }],
        });
        expect(parsed.vehicleId).toBeNull();
        expect(parsed.vehicleName).toBeNull();
    });

    it("rejects a response with no rows array", () => {
        expect(() => parseFleetSummary({})).toThrow("no rows");
        expect(() => parseFleetSummary(null)).toThrow("no rows");
    });

    it("names the first field that does not fit", () => {
        expect(() => parseFleetSummary({ rows: [{ ...row, trips: "2" }] })).toThrow('"trips"');
        expect(() => parseFleetSummary({ rows: [{ ...row, gst_nzd: 1 }] })).toThrow('"gst_nzd"');
        expect(() => parseFleetSummary({ rows: [{ ...row, vehicle_id: 7 }] })).toThrow(
            '"vehicle_id"',
        );
    });
});

describe("fetchFleetSummary", () => {
    it("asks for the date range with the key as a Bearer token", async () => {
        let requested = "";
        let auth = "";
        const fetchFn = (async (url: string, init: RequestInit) => {
            requested = url;
            auth = (init.headers as Record<string, string>).Authorization;
            return new Response(JSON.stringify({ rows: [row] }), { status: 200 });
        }) as unknown as typeof fetch;
        const rows = await fetchFleetSummary(
            { baseUrl: "http://backend.test", apiKey: "k", fetch: fetchFn },
            "2026-10-01",
            "2026-10-31",
        );
        expect(requested).toBe(
            "http://backend.test/v1/fleet/summary?from=2026-10-01&to=2026-10-31",
        );
        expect(auth).toBe("Bearer k");
        expect(rows).toHaveLength(1);
    });

    it("explains a driver key being refused for the fleet view", async () => {
        const fetchFn = (async () =>
            new Response("{}", { status: 403 })) as unknown as typeof fetch;
        await expect(
            fetchFleetSummary(
                { baseUrl: "http://b", apiKey: "k", fetch: fetchFn },
                "2026-10-01",
                "2026-10-31",
            ),
        ).rejects.toThrow("manager or admin");
    });
});
