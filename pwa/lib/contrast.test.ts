// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// Every text colour the app uses, against the background it is shown on. Hex
// values are Tailwind v4's neutral, red, amber and white palette. If a class
// changes in the components, update the pair here.

import { describe, expect, it } from "vitest";
import { AA_NORMAL_TEXT, contrastRatio, parseHex, relativeLuminance } from "./contrast";

const palette = {
    white: "#ffffff",
    "neutral-50": "#fafafa",
    "neutral-500": "#737373",
    "neutral-600": "#525252",
    "neutral-700": "#404040",
    "neutral-900": "#171717",
    "red-700": "#b91c1c",
    "amber-50": "#fffbeb",
    "amber-800": "#92400e",
};

/** Text colour and the background it sits on, as the components use them. */
const pairs: [text: keyof typeof palette, background: keyof typeof palette, where: string][] = [
    ["neutral-900", "neutral-50", "body text"],
    ["neutral-700", "neutral-50", "secondary text"],
    ["neutral-600", "neutral-50", "helper text"],
    ["neutral-500", "neutral-50", "small notes"],
    ["red-700", "neutral-50", "error messages"],
    ["neutral-900", "amber-50", "Section 43 notice"],
    ["amber-800", "amber-50", "storage warning"],
    ["white", "neutral-900", "primary buttons"],
    ["white", "neutral-700", "primary buttons on hover"],
];

describe("contrast", () => {
    it("matches WCAG's reference values", () => {
        // Black on white is the maximum, 21:1. Identical colours give 1:1.
        expect(contrastRatio("#000000", "#ffffff")).toBeCloseTo(21, 5);
        expect(contrastRatio("#777777", "#777777")).toBeCloseTo(1, 5);
    });

    it("parses short and long hex forms", () => {
        expect(parseHex("#fff")).toEqual([255, 255, 255]);
        expect(parseHex("#0a0a0a")).toEqual([10, 10, 10]);
        expect(() => parseHex("#zzz")).toThrow("not a hex colour");
    });

    it("gives white more luminance than black", () => {
        expect(relativeLuminance("#ffffff")).toBeGreaterThan(relativeLuminance("#000000"));
    });

    for (const [text, background, where] of pairs) {
        it(`${where}: ${text} on ${background} meets AA for normal text`, () => {
            const ratio = contrastRatio(palette[text], palette[background]);
            expect(
                ratio,
                `${text} on ${background} is ${ratio.toFixed(2)}:1`,
            ).toBeGreaterThanOrEqual(AA_NORMAL_TEXT);
        });
    }
});
