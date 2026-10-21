// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import { describe, expect, it, vi } from "vitest";
import {
    PERIODIC_SYNC_TAG,
    backgroundMinIntervalMs,
    backgroundStatusText,
    updatePeriodicSync,
    type BackgroundSyncEnv,
} from "./background-sync";

function fakeManager() {
    return {
        register: vi.fn(async () => {}),
        unregister: vi.fn(async () => {}),
    };
}

function env(overrides: Partial<BackgroundSyncEnv> = {}, manager = fakeManager()) {
    const full: BackgroundSyncEnv = {
        registration: { periodicSync: manager },
        permissionState: async () => "granted",
        ...overrides,
    };
    return { full, manager };
}

describe("backgroundMinIntervalMs", () => {
    it("asks for the schedule's interval and nothing for manual or on open", () => {
        expect(backgroundMinIntervalMs("15m")).toBe(900_000);
        expect(backgroundMinIntervalMs("hourly")).toBe(3_600_000);
        expect(backgroundMinIntervalMs("daily")).toBe(86_400_000);
        expect(backgroundMinIntervalMs("manual")).toBeNull();
        expect(backgroundMinIntervalMs("open")).toBeNull();
    });
});

describe("updatePeriodicSync", () => {
    it("registers the tag with the schedule's interval when upload is on", async () => {
        const { full, manager } = env();
        expect(await updatePeriodicSync(full, true, "hourly")).toBe("active");
        expect(manager.register).toHaveBeenCalledWith(PERIODIC_SYNC_TAG, {
            minInterval: 3_600_000,
        });
    });

    it("removes the registration when upload is off or the schedule does not repeat", async () => {
        const off = env();
        expect(await updatePeriodicSync(off.full, false, "hourly")).toBe("inactive");
        expect(off.manager.unregister).toHaveBeenCalledWith(PERIODIC_SYNC_TAG);
        expect(off.manager.register).not.toHaveBeenCalled();

        const manual = env();
        expect(await updatePeriodicSync(manual.full, true, "manual")).toBe("inactive");
        const open = env();
        expect(await updatePeriodicSync(open.full, true, "open")).toBe("inactive");
        expect(open.manager.register).not.toHaveBeenCalled();
    });

    it("reports unsupported without a registration or without periodicSync", async () => {
        expect(
            await updatePeriodicSync(
                { registration: undefined, permissionState: async () => "granted" },
                true,
                "hourly",
            ),
        ).toBe("unsupported");
        expect(
            await updatePeriodicSync(
                { registration: {}, permissionState: async () => "granted" },
                true,
                "hourly",
            ),
        ).toBe("unsupported");
    });

    it("reports unsupported when the browser does not know the permission", async () => {
        const { full, manager } = env({
            permissionState: async () => {
                throw new TypeError("unknown permission");
            },
        });
        expect(await updatePeriodicSync(full, true, "hourly")).toBe("unsupported");
        expect(manager.register).not.toHaveBeenCalled();
    });

    it("reports denied, and does not register, when the permission is not granted", async () => {
        const { full, manager } = env({ permissionState: async () => "denied" });
        expect(await updatePeriodicSync(full, true, "hourly")).toBe("denied");
        expect(manager.register).not.toHaveBeenCalled();
    });

    it("reports denied instead of throwing when the browser refuses to register", async () => {
        const manager = fakeManager();
        manager.register.mockRejectedValue(new DOMException("no", "NotAllowedError"));
        const { full } = env({}, manager);
        expect(await updatePeriodicSync(full, true, "daily")).toBe("denied");
    });
});

describe("backgroundStatusText", () => {
    it("has a line for every status", () => {
        for (const status of ["active", "inactive", "denied", "unsupported"] as const) {
            expect(backgroundStatusText(status).length).toBeGreaterThan(10);
        }
    });
});
