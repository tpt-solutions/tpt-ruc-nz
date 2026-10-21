import { expect, test } from "@playwright/test";

test("device segments import once, and repeats are skipped", async ({ page }) => {
    await page.goto("/");
    await page.getByRole("checkbox", { name: /understand this is an estimate tool/ }).check();
    await page.getByRole("button", { name: "Continue" }).click();
    await expect(page.getByRole("heading", { name: "Device segments" })).toBeVisible();
    await expect(page.getByText("Stored: 0 segments")).toBeVisible();

    const first = [
        { startMs: 1000, endMs: 2000, distanceM: 10 },
        { startMs: 3000, endMs: 4000, distanceM: 20 },
    ];
    await page.getByLabel("Import segments file").setInputFiles({
        name: "segments.json",
        mimeType: "application/json",
        buffer: Buffer.from(JSON.stringify(first)),
    });
    await expect(page.getByRole("status")).toHaveText("Added 2 segments. 0 were already stored.");
    await expect(page.getByText("Stored: 2 segments")).toBeVisible();

    // The second file repeats one segment and adds one new one.
    const second = [
        { startMs: 3000, endMs: 4000, distanceM: 20 },
        { startMs: 5000, endMs: 6000, distanceM: 30 },
    ];
    await page.getByLabel("Import segments file").setInputFiles({
        name: "more.json",
        mimeType: "application/json",
        buffer: Buffer.from(JSON.stringify(second)),
    });
    await expect(page.getByRole("status")).toHaveText("Added 1 segments. 1 were already stored.");
    await expect(page.getByText("Stored: 3 segments")).toBeVisible();
});

test("a file that is not a segment list is refused", async ({ page }) => {
    await page.goto("/");
    await page.getByRole("checkbox", { name: /understand this is an estimate tool/ }).check();
    await page.getByRole("button", { name: "Continue" }).click();
    await page.getByLabel("Import segments file").setInputFiles({
        name: "notes.json",
        mimeType: "application/json",
        buffer: Buffer.from('{"hello":"world"}'),
    });
    await expect(page.getByText("Could not import that file: expected a JSON array")).toBeVisible();
});
