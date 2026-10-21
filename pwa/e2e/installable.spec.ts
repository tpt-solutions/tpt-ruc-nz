import { expect, test } from "@playwright/test";

// The browser offers installation when the manifest has these fields, the page has a
// registered service worker that controls it, and the icons are available.
test("the app meets the installability requirements", async ({ page, request }) => {
    const response = await request.get("/manifest.webmanifest");
    expect(response.ok()).toBeTruthy();
    const manifest = await response.json();

    expect(manifest.name).toBeTruthy();
    expect(manifest.short_name).toBeTruthy();
    expect(manifest.start_url).toBe("/");
    expect(["standalone", "fullscreen", "minimal-ui"]).toContain(manifest.display);

    const sizes = (manifest.icons as { src: string; sizes: string; type: string }[]).filter(
        (icon) => icon.type === "image/png",
    );
    expect(sizes.map((icon) => icon.sizes)).toEqual(expect.arrayContaining(["192x192", "512x512"]));
    for (const icon of sizes) {
        const iconResponse = await request.get(icon.src);
        expect(iconResponse.ok()).toBeTruthy();
        expect(iconResponse.headers()["content-type"]).toContain("image/png");
    }

    await page.goto("/");
    await page.getByRole("checkbox", { name: /understand this is an estimate tool/ }).check();
    await page.getByRole("button", { name: "Continue" }).click();
    await page.evaluate(() => navigator.serviceWorker.ready);
    await page.reload();
    await page.waitForFunction(() => navigator.serviceWorker.controller !== null);
});
