import { expect, test } from "@playwright/test";

test("a trip shows its estimated charge on the trip's date", async ({ page }) => {
    await page.goto("/");
    await page.getByRole("checkbox", { name: /understand this is an estimate tool/ }).check();
    await page.getByRole("button", { name: "Continue" }).click();
    await expect(page.getByRole("heading", { name: "Vehicles" })).toBeVisible();

    // A type 1 vehicle at 2,000 kg, which is $76 per 1,000 km including GST.
    await page.getByLabel("Name", { exact: true }).fill("Hilux");
    await page.getByLabel("Vehicle RUC weight (kg)").fill("2000");
    await page.getByRole("button", { name: "Add vehicle" }).click();
    await expect(page.getByRole("listitem").filter({ hasText: "Hilux" }).first()).toBeVisible();

    await page.getByLabel("Distance (km)", { exact: true }).last().fill("100");
    await page.getByRole("button", { name: "Add trip" }).click();
    await expect(page.getByText("100.000 km")).toBeVisible();

    await page.getByRole("button", { name: "Details" }).click();
    // 100 km at $76 per 1,000 km is $7.60 including GST.
    await expect(page.getByText("Estimated charge including GST:")).toBeVisible();
    await expect(page.getByText("$7.60", { exact: true })).toBeVisible();

    await page.getByRole("button", { name: "Close details" }).click();
    await expect(page.getByText("Trip details")).toHaveCount(0);
});
