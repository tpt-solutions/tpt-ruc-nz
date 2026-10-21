// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import { describe, expect, it } from "vitest";
import { formatBytes, isNearQuota } from "./storage";

describe("formatBytes", () => {
    it("formats bytes, kilobytes, megabytes and gigabytes", () => {
        expect(formatBytes(512)).toBe("512 B");
        expect(formatBytes(1536)).toBe("1.5 KB");
        expect(formatBytes(3 * 1024 * 1024)).toBe("3.0 MB");
        expect(formatBytes(2 * 1024 ** 3)).toBe("2.0 GB");
    });

    it("reports unusable input as unknown", () => {
        expect(formatBytes(-1)).toBe("unknown");
        expect(formatBytes(Number.NaN)).toBe("unknown");
    });
});

describe("isNearQuota", () => {
    it("warns at 80% of the quota and above", () => {
        expect(isNearQuota(79, 100)).toBe(false);
        expect(isNearQuota(80, 100)).toBe(true);
        expect(isNearQuota(100, 100)).toBe(true);
    });

    it("does not warn when the quota is unknown", () => {
        expect(isNearQuota(10, 0)).toBe(false);
        expect(isNearQuota(Number.NaN, 100)).toBe(false);
    });
});
