// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

/** One row of the trip export. */
export interface TripCsvRow {
    vehicle: string;
    /** Local calendar date, `YYYY-MM-DD`. */
    date: string;
    /** Distance in whole metres. Converted to km with three decimals, once, here. */
    distanceM: number;
}

export const TRIP_CSV_HEADER = "vehicle,date,distance_km";

/**
 * Quotes a CSV field when it contains a delimiter, quote or line break. A value
 * that starts with `=`, `+`, `-` or `@` gets a leading apostrophe, so a
 * spreadsheet shows it as text instead of running it as a formula.
 */
export function csvField(value: string): string {
    const safe = /^[=+\-@]/.test(value) ? `'${value}` : value;
    return /[",\r\n]/.test(safe) ? `"${safe.replace(/"/g, '""')}"` : safe;
}

/** Formats trips as CSV, with a header row and CRLF line endings. */
export function tripsToCsv(rows: TripCsvRow[]): string {
    const lines = [TRIP_CSV_HEADER];
    for (const row of rows) {
        const km = (Math.round(row.distanceM) / 1000).toFixed(3);
        lines.push([csvField(row.vehicle), csvField(row.date), km].join(","));
    }
    return lines.join("\r\n") + "\r\n";
}

/** Formats a timestamp as a local calendar date, `YYYY-MM-DD`. */
export function localIsoDate(ms: number): string {
    const d = new Date(ms);
    const month = String(d.getMonth() + 1).padStart(2, "0");
    const day = String(d.getDate()).padStart(2, "0");
    return `${d.getFullYear()}-${month}-${day}`;
}
