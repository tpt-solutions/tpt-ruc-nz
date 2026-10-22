import { expect, test, type Page } from "@playwright/test";
import { spawn, type ChildProcess } from "node:child_process";
import { existsSync, mkdtempSync } from "node:fs";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

// Drives the sync panel against a real backend process. Build the binary first
// (`cargo build -p tpt-ruc-backend` from the repository root). Without it the
// suite is skipped, as the vitest integration suite is.

const BINARY = resolve(
    "..",
    "target",
    "debug",
    process.platform === "win32" ? "tpt-ruc-backend.exe" : "tpt-ruc-backend",
);
const OWNER_KEY = "browser-test-key-0123456789abcdef0123456789";
const WRONG_KEY = "not-the-right-key-0123456789abcdef0123456789";
const haveBinary = existsSync(BINARY);

/** Picks a free local port, so parallel runs do not collide. */
function freePort(): Promise<number> {
    return new Promise((resolvePort, reject) => {
        const server = createServer();
        server.unref();
        server.on("error", reject);
        server.listen(0, "127.0.0.1", () => {
            const address = server.address();
            const port = typeof address === "object" && address ? address.port : 0;
            server.close(() => resolvePort(port));
        });
    });
}

async function waitForHealth(baseUrl: string): Promise<void> {
    const deadline = Date.now() + 15_000;
    while (Date.now() < deadline) {
        try {
            const response = await fetch(`${baseUrl}/healthz`);
            if (response.ok) return;
        } catch {
            // Not listening yet.
        }
        await new Promise((r) => setTimeout(r, 100));
    }
    throw new Error("the backend did not start");
}

/** Starts a backend with its own database. The page origin is allowed through CORS. */
async function startBackend(): Promise<{ child: ChildProcess; baseUrl: string }> {
    const port = await freePort();
    const baseUrl = `http://127.0.0.1:${port}`;
    const dataDir = mkdtempSync(join(tmpdir(), "tpt-ruc-e2e-"));
    const child = spawn(BINARY, [], {
        env: {
            ...process.env,
            TPT_RUC_BIND: `127.0.0.1:${port}`,
            TPT_RUC_DATABASE_URL: `sqlite://${join(dataDir, "tpt.db")}`,
            TPT_RUC_API_KEY: OWNER_KEY,
            TPT_RUC_CORS_ORIGINS: "http://localhost:3000",
            RUST_LOG: "warn",
        },
        stdio: "ignore",
    });
    await waitForHealth(baseUrl);
    return { child, baseUrl };
}

async function passNotice(page: Page) {
    await page.goto("/");
    await page.getByRole("checkbox", { name: /understand this is an estimate tool/ }).check();
    await page.getByRole("button", { name: "Continue" }).click();
    await expect(page.getByRole("heading", { name: "RUC calculator" })).toBeVisible();
}

async function addVehicle(page: Page, name: string) {
    await page.getByLabel("Name").fill(name);
    await page.getByLabel("Vehicle RUC weight (kg)").fill("3000");
    await page.getByRole("button", { name: "Add vehicle" }).click();
    await expect(page.getByRole("listitem").filter({ hasText: name }).first()).toBeVisible();
}

async function addTrip(page: Page, km: string) {
    await page.getByLabel("Distance (km)").last().fill(km);
    await page.getByRole("button", { name: "Add trip" }).click();
    await expect(page.getByText(`${km}00 km`)).toBeVisible();
}

async function enterBackend(page: Page, baseUrl: string, key: string) {
    await page.getByLabel("Backend address").fill(baseUrl);
    await page.getByLabel("API key").fill(key);
    await page.getByRole("button", { name: "Save backend settings" }).click();
    await expect(page.getByText("Saved in this browser.")).toBeVisible();
}

/** Trips the backend holds, read with the owner key. */
async function storedTripCount(baseUrl: string): Promise<number> {
    const response = await fetch(`${baseUrl}/v1/trips`, {
        headers: { Authorization: `Bearer ${OWNER_KEY}` },
    });
    const body = (await response.json()) as { trips: unknown[] };
    return body.trips.length;
}

test.describe.serial("sync against a real backend", () => {
    test.skip(!haveBinary, "build the backend first: cargo build -p tpt-ruc-backend");

    let child: ChildProcess | undefined;
    let baseUrl = "";

    test.beforeAll(async () => {
        ({ child, baseUrl } = await startBackend());
    });

    test.afterAll(() => {
        child?.kill();
    });

    test("a trip uploads with the owner key and the backend stores it", async ({ page }) => {
        await passNotice(page);
        await addVehicle(page, "Hilux");
        await addTrip(page, "12.5");
        await enterBackend(page, baseUrl, OWNER_KEY);

        await page.getByRole("checkbox", { name: "Upload trips to the backend" }).check();
        await expect(page.getByText("Upload is on.")).toBeVisible();
        await page.getByRole("button", { name: "Sync now" }).click();

        await expect(
            page.getByText("Last sync: 1 uploaded, 0 conflicts, 0 rejected."),
        ).toBeVisible();
        await expect(page.getByText("Waiting to upload: 0")).toBeVisible();
        expect(await storedTripCount(baseUrl)).toBe(1);
    });

    test("a manager's token uploads signed trips", async ({ page }) => {
        const created = await fetch(`${baseUrl}/v1/users`, {
            method: "POST",
            headers: { Authorization: `Bearer ${OWNER_KEY}`, "Content-Type": "application/json" },
            body: JSON.stringify({ name: "Manager", role: "manager" }),
        });
        const { token } = (await created.json()) as { token: string };
        const before = await storedTripCount(baseUrl);

        await passNotice(page);
        await addVehicle(page, "Ranger");
        await addTrip(page, "3.0");
        // A user's token registers this browser's device key, so the uploads are signed.
        await enterBackend(page, baseUrl, token);

        await page.getByRole("checkbox", { name: "Upload trips to the backend" }).check();
        await expect(page.getByText("Upload is on.")).toBeVisible();
        await expect(page.getByText(/uploads are signed with this device's key/)).toBeVisible();
        await page.getByRole("button", { name: "Sync now" }).click();

        await expect(
            page.getByText("Last sync: 1 uploaded, 0 conflicts, 0 rejected."),
        ).toBeVisible();
        expect(await storedTripCount(baseUrl)).toBe(before + 1);
    });

    test("a refused key shows the reason and keeps the trip on this device", async ({ page }) => {
        const before = await storedTripCount(baseUrl);

        await passNotice(page);
        await addVehicle(page, "Navara");
        await addTrip(page, "7.0");
        await enterBackend(page, baseUrl, WRONG_KEY);

        await page.getByRole("checkbox", { name: "Upload trips to the backend" }).check();
        await expect(page.getByText("Upload is on.")).toBeVisible();
        await page.getByRole("button", { name: "Sync now" }).click();

        await expect(
            page.getByRole("alert").filter({ hasText: "The backend refused the API key" }),
        ).toBeVisible();
        await expect(page.getByText("Waiting to upload: 1")).toBeVisible();
        expect(await storedTripCount(baseUrl)).toBe(before);
    });
});
