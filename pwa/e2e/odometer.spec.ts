import { expect, test, type Page } from "@playwright/test";

async function acceptNotice(page: Page) {
    await page.goto("/");
    await page.getByRole("checkbox", { name: /understand this is an estimate tool/ }).check();
    await page.getByRole("button", { name: "Continue" }).click();
    await expect(page.getByRole("heading", { name: "Vehicles" })).toBeVisible();
}

test("an odometer reading is kept with the vehicle", async ({ page }) => {
    await acceptNotice(page);
    await page.getByLabel("Name", { exact: true }).fill("Van");
    await page.getByLabel("Vehicle RUC weight (kg)").fill("3000");
    await page.getByLabel("Odometer (km), optional").fill("123456");
    await page.getByRole("button", { name: "Add vehicle" }).click();

    const item = page.getByRole("listitem").filter({ hasText: "Van" }).first();
    await expect(item).toContainText("odometer 123,456 km");

    await page.reload();
    await expect(page.getByRole("listitem").filter({ hasText: "Van" }).first()).toContainText(
        "odometer 123,456 km",
    );
});

test("a fractional odometer reading is refused", async ({ page }) => {
    await acceptNotice(page);
    await page.getByLabel("Name", { exact: true }).fill("Van");
    await page.getByLabel("Odometer (km), optional").fill("12.5");
    await page.getByRole("button", { name: "Add vehicle" }).click();
    await expect(page.getByText("Enter the odometer as a whole number")).toBeVisible();
});
