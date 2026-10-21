import { expect, test } from "@playwright/test";

test("the shell and the wasm calculator work offline after one online visit", async ({
    page,
    context,
}) => {
    await page.goto("/");
    await page.getByRole("checkbox", { name: /understand this is an estimate tool/ }).check();
    await page.getByRole("button", { name: "Continue" }).click();
    await expect(page.getByRole("heading", { level: 1, name: "tpt-ruc-nz" })).toBeVisible();

    // Wait until the service worker controls the page, then reload so it serves the shell
    // and caches the WASM module on the way.
    await page.evaluate(() => navigator.serviceWorker.ready);
    await page.reload();
    await page.waitForFunction(() => navigator.serviceWorker.controller !== null);
    await expect(page.getByRole("button", { name: "Calculate" }).first()).toBeVisible();

    await context.setOffline(true);
    await page.reload();
    await expect(page.getByRole("heading", { level: 1, name: "tpt-ruc-nz" })).toBeVisible();

    // The calculator needs the WASM module. It must load from the cache.
    await page.getByLabel("Distance (km)", { exact: true }).first().fill("100");
    await page.getByRole("button", { name: "Calculate" }).first().click();
    await expect(page.getByText("$7.60")).toBeVisible();
});
