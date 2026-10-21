import { expect, test } from "@playwright/test";

test("the storage status shows usage and the persistence state", async ({ page }) => {
    await page.goto("/");
    await page.getByRole("checkbox", { name: /understand this is an estimate tool/ }).check();
    await page.getByRole("button", { name: "Continue" }).click();
    await expect(page.getByText(/Storage in use: .* of .*\./)).toBeVisible();
});
