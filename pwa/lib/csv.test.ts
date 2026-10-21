// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import { describe, expect, it } from "vitest";
import { csvField, localIsoDate, tripsToCsv, TRIP_CSV_HEADER } from "./csv";

describe("csvField", () => {
    it("leaves plain values alone", () => {
        expect(csvField("Hilux")).toBe("Hilux");
    });

    it("quotes values with commas, quotes or line breaks", () => {
        expect(csvField("Truck, big")).toBe('"Truck, big"');
        expect(csvField('say "hi"')).toBe('"say ""hi"""');
        expect(csvField("a\nb")).toBe('"a\nb"');
    });

    it("neutralises spreadsheet formulas", () => {
        expect(csvField('=HYPERLINK("x")')).toBe('"\'=HYPERLINK(""x"")"');
        expect(csvField("+1")).toBe("'+1");
        expect(csvField("-2")).toBe("'-2");
        expect(csvField("@cmd")).toBe("'@cmd");
    });
});

describe("tripsToCsv", () => {
    it("writes a header and one row per trip, with km to three decimals", () => {
        const csv = tripsToCsv([
            { vehicle: "Hilux", date: "2026-10-01", distanceM: 12_345.6 },
            { vehicle: "Truck, big", date: "2026-10-02", distanceM: 500 },
        ]);
        expect(csv).toBe(
            [TRIP_CSV_HEADER, "Hilux,2026-10-01,12.346", '"Truck, big",2026-10-02,0.500', ""].join(
                "\r\n",
            ),
        );
    });

    it("writes only the header for no trips", () => {
        expect(tripsToCsv([])).toBe(`${TRIP_CSV_HEADER}\r\n`);
    });
});

describe("localIsoDate", () => {
    it("formats a local date with zero padding", () => {
        expect(localIsoDate(new Date(2026, 0, 5, 12).getTime())).toBe("2026-01-05");
    });
});
