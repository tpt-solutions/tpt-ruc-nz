import { describe, expect, it } from "vitest";
import { formatMicros } from "./money";

describe("formatMicros", () => {
    it("formats whole and fractional dollars", () => {
        expect(formatMicros(0)).toBe("$0.00");
        expect(formatMicros(1_000_000)).toBe("$1.00");
        expect(formatMicros(1_234_500)).toBe("$1.23");
    });

    it("rounds to the nearest cent", () => {
        expect(formatMicros(1_235_000)).toBe("$1.24");
        expect(formatMicros(1_004_999)).toBe("$1.00");
    });

    it("formats thousands and negative amounts", () => {
        expect(formatMicros(1_234_567_000_000)).toBe("$1,234,567.00");
        expect(formatMicros(-2_500_000)).toBe("-$2.50");
    });

    it("rejects fractional micro-dollars", () => {
        expect(() => formatMicros(1.5)).toThrow(RangeError);
    });
});
