// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import { expect, test, type Page } from "@playwright/test";

async function acceptNotice(page: Page) {
    await page.goto("/");
    await page.getByRole("checkbox", { name: /understand this is an estimate tool/ }).check();
    await page.getByRole("button", { name: "Continue" }).click();
    await expect(page.getByRole("heading", { name: "Vehicles" })).toBeVisible();
}

async function addVehicle(page: Page, odometerKm: string, licenceStart: string, licenceKm: string) {
    await page.getByLabel("Name", { exact: true }).fill("Truck");
    await page.getByLabel("Vehicle RUC weight (kg)").fill("3000");
    await page.getByLabel("Odometer (km), optional").fill(odometerKm);
    await page.getByLabel("Odometer when licence bought (km), optional").fill(licenceStart);
    await page.getByLabel("Distance licence covers (km), optional").fill(licenceKm);
    await page.getByRole("button", { name: "Add vehicle" }).click();
}

test("the distance left on a licence is shown from the entered odometer", async ({ page }) => {
    await acceptNotice(page);
    await addVehicle(page, "120000", "100000", "30000");

    await expect(page.getByText("Distance left on the licence:")).toBeVisible();
    await expect(page.getByText("10,000 km", { exact: true })).toBeVisible();
});

test("a used-up distance licence says so", async ({ page }) => {
    await acceptNotice(page);
    await addVehicle(page, "140000", "100000", "30000");

    await expect(page.getByText("The distance licence is used up.")).toBeVisible();
});

test("a licence needs both of its figures", async ({ page }) => {
    await acceptNotice(page);
    await addVehicle(page, "", "100000", "");

    await expect(page.getByText("Enter the licence odometer as a whole number")).toBeVisible();
});

test("no distance is shown without an odometer reading", async ({ page }) => {
    await acceptNotice(page);
    await addVehicle(page, "", "100000", "30000");

    await expect(page.getByRole("heading", { name: "This month" })).toBeVisible();
    await expect(page.getByText("Distance left on the licence:")).toHaveCount(0);
});
