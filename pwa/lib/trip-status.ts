// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import type { Trip } from "./db";

/** Where the trip came from and where it is in the upload, as one line of text. */
export function tripStatusText(trip: Trip): string {
    const origin = trip.source === "device" ? "Recorded by device" : "Entered by hand";
    const sync = trip.sync;
    if (!sync) return `${origin}. Waiting to upload.`;
    if (sync.state === "synced") return `${origin}. Uploaded.`;
    if (sync.state === "conflict")
        return `${origin}. Conflict on the backend: ${sync.message ?? "see Settings"}.`;
    return `${origin}. Not uploaded: ${sync.message ?? "the backend refused it"}.`;
}
