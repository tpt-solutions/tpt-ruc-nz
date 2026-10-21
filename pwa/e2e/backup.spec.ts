import { expect, test, type Page } from "@playwright/test";
import { readFile } from "node:fs/promises";

async function acceptNotice(page: Page) {
    await page.goto("/");
    await page.getByRole("checkbox", { name: /understand this is an estimate tool/ }).check();
    await page.getByRole("button", { name: "Continue" }).click();
    await expect(page.getByRole("heading", { name: "Vehicles" })).toBeVisible();
}

test("a backup restores vehicles and trips after they are deleted", async ({ page }) => {
    await acceptNotice(page);

    // Create a vehicle and a trip to back up.
    await page.getByLabel("Name").fill("Backup Ute");
    await page.getByLabel("Vehicle RUC weight (kg)").fill("3000");
    await page.getByRole("button", { name: "Add vehicle" }).click();
    await expect(page.getByRole("listitem").filter({ hasText: "Backup Ute" })).toBeVisible();
    await page.getByLabel("Distance (km)").last().fill("7");
    await page.getByRole("button", { name: "Add trip" }).click();
    await expect(page.getByText("7.000 km")).toBeVisible();

    // Export the backup.
    const exportPromise = page.waitForEvent("download");
    await page.getByRole("button", { name: "Export backup" }).click();
    const exported = await exportPromise;
    const backupPath = await exported.path();
    const backupText = await readFile(backupPath, "utf8");
    expect(JSON.parse(backupText).vehicles.map((v: { name: string }) => v.name)).toContain(
        "Backup Ute",
    );

    // Delete the vehicle, which leaves its trip orphaned.
    await page.getByRole("button", { name: "Delete Backup Ute" }).click();
    await expect(page.getByText("No vehicles yet.")).toBeVisible();

    // Restore from the exported file.
    await page.getByLabel("Restore from file").setInputFiles(backupPath);
    await expect(page.getByRole("status")).toContainText("Restored 1 vehicles and 1 trips.");
    await expect(page.getByRole("listitem").filter({ hasText: "Backup Ute" })).toBeVisible();
    await expect(page.getByText("7.000 km")).toBeVisible();
});

test("restoring a file that is not a backup shows an error", async ({ page }) => {
    await acceptNotice(page);
    await page.getByLabel("Restore from file").setInputFiles({
        name: "notes.json",
        mimeType: "application/json",
        buffer: Buffer.from('{"hello":"world"}'),
    });
    await expect(page.getByText("Could not restore that file: not a tpt-ruc backup")).toBeVisible();
});
