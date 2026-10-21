// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import { describe, expect, it } from "vitest";
import type { Trip } from "./db";
import { tripStatusText } from "./trip-status";

const base: Trip = { id: "t", vehicleId: "v", startMs: 0, endMs: 1, distanceM: 1 };

describe("tripStatusText", () => {
    it("says a trip without a source was entered by hand and is waiting", () => {
        expect(tripStatusText(base)).toBe("Entered by hand. Waiting to upload.");
    });

    it("names a device trip and its upload state", () => {
        expect(
            tripStatusText({ ...base, source: "device", sync: { state: "synced", syncedMs: 1 } }),
        ).toBe("Recorded by device. Uploaded.");
    });

    it("explains a conflict and a rejection with the backend's message", () => {
        expect(
            tripStatusText({ ...base, sync: { state: "conflict", message: "already stored" } }),
        ).toContain("Conflict on the backend: already stored");
        expect(
            tripStatusText({ ...base, sync: { state: "rejected", message: "no rates" } }),
        ).toContain("Not uploaded: no rates");
    });
});
