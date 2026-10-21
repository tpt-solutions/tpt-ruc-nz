import { expect, test } from "@playwright/test";

test("calculator charges 100 km for a 2,000 kg diesel at the published rate", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByRole("button", { name: "Calculate" })).toBeVisible();

    await page.getByLabel("Distance (km)").fill("100");
    await page.getByLabel("RUC weight (kg)").fill("2000");
    await page.getByRole("button", { name: "Calculate" }).click();

    // 100 km at $76 per 1,000 km is $7.60 including GST.
    await expect(page.getByText("$7.60")).toBeVisible();
    await expect(page.getByText("Excluding GST $6.61, GST $0.99")).toBeVisible();
});

test("calculator rejects a zero weight", async ({ page }) => {
    await page.goto("/");
    await page.getByLabel("RUC weight (kg)").fill("0");
    await page.getByRole("button", { name: "Calculate" }).click();
    await expect(page.getByText("Enter a whole-number weight above zero.")).toBeVisible();
});
