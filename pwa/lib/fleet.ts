// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// Client for the backend's `GET /v1/fleet/summary`. The backend formats money as
// NZD text, so the figures are shown as the server gives them. Only the shape is
// checked here, so a changed response fails loudly instead of showing blanks.

/** One vehicle's totals for one month, as the backend reports them. */
export interface FleetRow {
    month: string;
    vehicleId: string | null;
    vehicleName: string | null;
    rateType: string;
    trips: number;
    billedKm: string;
    chargeExclGstNzd: string;
    gstNzd: string;
    chargeInclGstNzd: string;
}

export interface FleetDeps {
    baseUrl: string;
    apiKey: string;
    fetch: typeof fetch;
}

const TEXT_FIELDS = [
    "month",
    "rate_type",
    "billed_km",
    "charge_excl_gst_nzd",
    "gst_nzd",
    "charge_incl_gst_nzd",
] as const;

/** Reads the summary body. Throws a TypeError naming the first field that does not fit. */
export function parseFleetSummary(body: unknown): FleetRow[] {
    if (
        typeof body !== "object" ||
        body === null ||
        !Array.isArray((body as { rows?: unknown }).rows)
    ) {
        throw new TypeError("the fleet summary has no rows");
    }
    const rows = (body as { rows: unknown[] }).rows;
    return rows.map((raw, index) => {
        const row = raw as Record<string, unknown>;
        for (const field of TEXT_FIELDS) {
            if (typeof row[field] !== "string") {
                throw new TypeError(`fleet row ${index} has no text "${field}"`);
            }
        }
        if (typeof row.trips !== "number") {
            throw new TypeError(`fleet row ${index} has no count "trips"`);
        }
        const vehicleId = row.vehicle_id;
        const vehicleName = row.vehicle_name;
        if (vehicleId !== null && typeof vehicleId !== "string") {
            throw new TypeError(`fleet row ${index} has an invalid "vehicle_id"`);
        }
        if (vehicleName !== null && typeof vehicleName !== "string") {
            throw new TypeError(`fleet row ${index} has an invalid "vehicle_name"`);
        }
        return {
            month: row.month as string,
            vehicleId,
            vehicleName,
            rateType: row.rate_type as string,
            trips: row.trips,
            billedKm: row.billed_km as string,
            chargeExclGstNzd: row.charge_excl_gst_nzd as string,
            gstNzd: row.gst_nzd as string,
            chargeInclGstNzd: row.charge_incl_gst_nzd as string,
        };
    });
}

/** Fetches the fleet summary for an inclusive date range, `YYYY-MM-DD`. */
export async function fetchFleetSummary(
    deps: FleetDeps,
    from: string,
    to: string,
): Promise<FleetRow[]> {
    const query = new URLSearchParams({ from, to });
    const response = await deps.fetch(`${deps.baseUrl}/v1/fleet/summary?${query}`, {
        headers: { Authorization: `Bearer ${deps.apiKey}` },
    });
    if (response.status === 401) throw new Error("The backend refused the API key.");
    if (response.status === 403) {
        throw new Error("The fleet summary needs a manager or admin key.");
    }
    if (!response.ok) throw new Error(`The backend answered HTTP ${response.status}.`);
    return parseFleetSummary(await response.json());
}
