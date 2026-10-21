// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

/** Share of the browser quota above which the app warns that space is running out. */
export const QUOTA_WARNING_RATIO = 0.8;

/** Formats a byte count for display, for example `1.5 MB`. */
export function formatBytes(bytes: number): string {
    if (!Number.isFinite(bytes) || bytes < 0) return "unknown";
    const units = ["B", "KB", "MB", "GB"];
    let value = bytes;
    let unit = 0;
    while (value >= 1024 && unit < units.length - 1) {
        value /= 1024;
        unit++;
    }
    const digits = unit === 0 ? 0 : 1;
    return `${value.toFixed(digits)} ${units[unit]}`;
}

/** True when usage is at or above the warning share of the quota. */
export function isNearQuota(usage: number, quota: number): boolean {
    if (!Number.isFinite(usage) || !Number.isFinite(quota) || quota <= 0) return false;
    return usage / quota >= QUOTA_WARNING_RATIO;
}
