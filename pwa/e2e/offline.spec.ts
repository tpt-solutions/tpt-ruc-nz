import { expect, test } from "@playwright/test";

test("the shell opens offline after one online visit", async ({ page, context }) => {
    await page.goto("/");
    await page.getByRole("checkbox", { name: /understand this is an estimate tool/ }).check();
    await page.getByRole("button", { name: "Continue" }).click();
    await expect(page.getByRole("heading", { level: 1, name: "tpt-ruc-nz" })).toBeVisible();

    // Wait until the service worker controls the page, then reload so it serves the shell.
    await page.evaluate(() => navigator.serviceWorker.ready);
    await page.reload();
    await page.waitForFunction(() => navigator.serviceWorker.controller !== null);

    await context.setOffline(true);
    await page.reload();
    await expect(page.getByRole("heading", { level: 1, name: "tpt-ruc-nz" })).toBeVisible();
});
