import { expect, test, type Page } from "@playwright/test";
import { readFile } from "node:fs/promises";

async function acceptNoticeAndAddVehicle(page: Page, name: string) {
    await page.goto("/");
    await page.getByRole("checkbox", { name: /understand this is an estimate tool/ }).check();
    await page.getByRole("button", { name: "Continue" }).click();
    await expect(page.getByRole("heading", { name: "Vehicles" })).toBeVisible();
    await page.getByLabel("Name").fill(name);
    await page.getByLabel("Vehicle RUC weight (kg)").fill("3000");
    await page.getByRole("button", { name: "Add vehicle" }).click();
    await expect(vehicleItem(page, name)).toBeVisible();
}

/** The vehicle list entry, which the trip form's dropdown also names. */
function vehicleItem(page: Page, name: string) {
    return page.getByRole("listitem").filter({ hasText: name }).first();
}

function localToday(): string {
    const d = new Date();
    const month = String(d.getMonth() + 1).padStart(2, "0");
    const day = String(d.getDate()).padStart(2, "0");
    return `${d.getFullYear()}-${month}-${day}`;
}

test("a vehicle can be edited and the change survives a reload", async ({ page }) => {
    await acceptNoticeAndAddVehicle(page, "Hilux");
    await page.getByRole("button", { name: "Edit Hilux" }).click();
    await page.getByLabel("Name").fill("Ranger");
    await page.getByRole("button", { name: "Save changes" }).click();
    await expect(vehicleItem(page, "Ranger")).toBeVisible();
    await expect(page.getByRole("listitem").filter({ hasText: "Hilux" })).toHaveCount(0);

    await page.reload();
    await expect(vehicleItem(page, "Ranger")).toBeVisible();
});

test("a trip is recorded and exported as CSV", async ({ page }) => {
    await acceptNoticeAndAddVehicle(page, "Hilux");
    await page.getByLabel("Distance (km)").last().fill("12.5");
    await page.getByRole("button", { name: "Add trip" }).click();
    await expect(page.getByText("12.500 km")).toBeVisible();

    const downloadPromise = page.waitForEvent("download");
    await page.getByRole("button", { name: "Download CSV" }).click();
    const download = await downloadPromise;
    expect(download.suggestedFilename()).toBe("tpt-ruc-trips.csv");

    const path = await download.path();
    const csv = await readFile(path, "utf8");
    expect(csv).toBe(`vehicle,date,distance_km\r\nHilux,${localToday()},12.500\r\n`);
});
