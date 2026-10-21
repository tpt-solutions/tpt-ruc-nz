import { expect, test } from "@playwright/test";

test("home page shows the title and the Section 43 notice", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByRole("heading", { level: 1, name: "tpt-ruc-nz" })).toBeVisible();
    await expect(page.getByText("Not an NZTA-approved distance recorder")).toBeVisible();
});

test("manifest is served", async ({ request }) => {
    const response = await request.get("/manifest.webmanifest");
    expect(response.ok()).toBeTruthy();
    const manifest = await response.json();
    expect(manifest.name).toBe("tpt-ruc-nz");
});
