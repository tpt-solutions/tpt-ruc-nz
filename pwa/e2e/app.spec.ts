import { expect, test, type Page } from "@playwright/test";

async function acceptNotice(page: Page) {
    await page.goto("/");
    await page.getByRole("checkbox", { name: /understand this is an estimate tool/ }).check();
    await page.getByRole("button", { name: "Continue" }).click();
    await expect(page.getByRole("heading", { name: "RUC calculator" })).toBeVisible();
}

test("the Section 43 notice is shown until it is accepted", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByRole("heading", { name: "Before you start" })).toBeVisible();
    await page.getByRole("checkbox", { name: /understand this is an estimate tool/ }).check();
    await page.getByRole("button", { name: "Continue" }).click();
    await expect(page.getByRole("heading", { name: "RUC calculator" })).toBeVisible();

    // The choice is kept, so the notice does not return after a reload.
    await page.reload();
    await expect(page.getByRole("heading", { name: "RUC calculator" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Before you start" })).toHaveCount(0);
});

test("a vehicle added to the profile list survives a reload", async ({ page }) => {
    await acceptNotice(page);
    await page.getByLabel("Name").fill("Hilux");
    await page.getByLabel("Vehicle RUC weight (kg)").fill("3000");
    await page.getByRole("button", { name: "Add vehicle" }).click();
    await expect(page.getByText("Hilux", { exact: true })).toBeVisible();

    await page.reload();
    await expect(page.getByText("Hilux", { exact: true })).toBeVisible();

    await page.getByRole("button", { name: "Delete Hilux" }).click();
    await expect(page.getByText("No vehicles yet.")).toBeVisible();
});

test("calculator charges 100 km for a 2,000 kg diesel at the published rate", async ({ page }) => {
    await acceptNotice(page);
    await page.getByLabel("Distance (km)").fill("100");
    await page.getByLabel("RUC weight (kg)").first().fill("2000");
    await page.getByRole("button", { name: "Calculate" }).click();

    // 100 km at $76 per 1,000 km is $7.60 including GST.
    await expect(page.getByText("$7.60")).toBeVisible();
    await expect(page.getByText("Excluding GST $6.61, GST $0.99")).toBeVisible();
});

test("calculator rejects a zero weight", async ({ page }) => {
    await acceptNotice(page);
    await page.getByLabel("RUC weight (kg)").first().fill("0");
    await page.getByRole("button", { name: "Calculate" }).click();
    await expect(page.getByText("Enter a whole-number weight above zero.")).toBeVisible();
});
