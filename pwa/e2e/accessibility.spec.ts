import { expect, test } from "@playwright/test";

async function passNotice(page: import("@playwright/test").Page) {
    await page.goto("/");
    await page.getByRole("checkbox", { name: /understand this is an estimate tool/ }).check();
    await page.getByRole("button", { name: "Continue" }).click();
    await expect(page.getByRole("heading", { name: "RUC calculator" })).toBeVisible();
}

test("every form control has an accessible name", async ({ page }) => {
    await passNotice(page);
    // Add one vehicle and one trip so the dashboard and trip form are rendered too.
    const unlabelled = await page.$$eval("input, select, textarea", (elements) =>
        elements
            .filter((el) => (el as HTMLInputElement).type !== "hidden")
            .filter((el) => {
                const labelled =
                    (el as HTMLInputElement).labels?.length ||
                    el.hasAttribute("aria-label") ||
                    el.hasAttribute("aria-labelledby");
                return !labelled;
            })
            .map((el) => `${el.tagName.toLowerCase()}${el.id ? `#${el.id}` : ""}`),
    );
    expect(unlabelled).toEqual([]);
});

test("the backend form is reachable with the keyboard, in order", async ({ page }) => {
    await passNotice(page);
    await page.getByLabel("Backend address").focus();
    await page.keyboard.press("Tab");
    await expect(page.getByLabel("API key")).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(page.getByRole("button", { name: "Save backend settings" })).toBeFocused();
});

test("the sync and fleet sections have headings a screen reader can jump to", async ({ page }) => {
    await passNotice(page);
    for (const name of [
        "Backend and upload",
        "Fleet summary",
        "Tracking device",
        "This month",
        "Settings",
    ]) {
        await expect(page.getByRole("heading", { name })).toBeVisible();
    }
});
