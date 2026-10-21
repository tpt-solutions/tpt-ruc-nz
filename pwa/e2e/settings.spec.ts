import { expect, test } from "@playwright/test";

test("the Section 43 notice can be shown again from settings", async ({ page }) => {
    await page.goto("/");
    await page.getByRole("checkbox", { name: /understand this is an estimate tool/ }).check();
    await page.getByRole("button", { name: "Continue" }).click();
    await expect(page.getByRole("heading", { name: "RUC calculator" })).toBeVisible();

    await page.getByRole("button", { name: "Show the Section 43 notice again" }).click();
    await expect(page.getByRole("heading", { name: "Before you start" })).toBeVisible();
});
