// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// WCAG 2.x contrast ratio. Used by the test that checks every text colour the
// app uses against its background. Axe-core is not a declared dependency, so
// this check is kept in the project.

/** Parses `#rgb` or `#rrggbb` into 0 to 255 channels. */
export function parseHex(hex: string): [number, number, number] {
    const digits = hex.replace(/^#/, "");
    const full =
        digits.length === 3
            ? digits
                  .split("")
                  .map((c) => c + c)
                  .join("")
            : digits;
    if (!/^[0-9a-fA-F]{6}$/.test(full)) throw new Error(`not a hex colour: ${hex}`);
    return [0, 2, 4].map((i) => parseInt(full.slice(i, i + 2), 16)) as [number, number, number];
}

/** Relative luminance, as defined by WCAG 2.x. */
export function relativeLuminance(hex: string): number {
    const channels = parseHex(hex).map((value) => {
        const s = value / 255;
        return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
    }) as [number, number, number];
    const [r, g, b] = channels;
    return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** Contrast ratio between two colours, from 1 (none) to 21 (black on white). */
export function contrastRatio(foreground: string, background: string): number {
    const a = relativeLuminance(foreground);
    const b = relativeLuminance(background);
    const [lighter, darker] = a > b ? [a, b] : [b, a];
    return (lighter + 0.05) / (darker + 0.05);
}

/** The minimum ratio for normal-size text under WCAG 2.x level AA. */
export const AA_NORMAL_TEXT = 4.5;
