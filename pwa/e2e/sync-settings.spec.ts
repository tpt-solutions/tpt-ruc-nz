import { expect, test } from "@playwright/test";

const KEY = "test-key-0123456789abcdef0123456789abcdef";

async function passNotice(page: import("@playwright/test").Page) {
    await page.goto("/");
    await page.getByRole("checkbox", { name: /understand this is an estimate tool/ }).check();
    await page.getByRole("button", { name: "Continue" }).click();
    await expect(page.getByRole("heading", { name: "RUC calculator" })).toBeVisible();
}

test("upload is off by default, and backend settings survive a reload", async ({ page }) => {
    await passNotice(page);
    const upload = page.getByRole("checkbox", { name: "Upload trips to the backend" });
    await expect(upload).not.toBeChecked();

    await page.getByLabel("Backend address").fill("http://localhost:8080/");
    await page.getByLabel("API key").fill(KEY);
    await page.getByRole("button", { name: "Save backend settings" }).click();
    await expect(page.getByText("Saved in this browser.")).toBeVisible();

    await page.reload();
    await expect(page.getByLabel("Backend address")).toHaveValue("http://localhost:8080");
    await expect(page.getByLabel("API key")).toHaveValue(KEY);
    await expect(
        page.getByRole("checkbox", { name: "Upload trips to the backend" }),
    ).not.toBeChecked();
});

test("the upload schedule defaults to on open and survives a reload", async ({ page }) => {
    await passNotice(page);
    const schedule = page.getByLabel("Upload schedule");
    await expect(schedule).toHaveValue("open");

    await schedule.selectOption("hourly");
    await expect(page.getByText("Upload schedule saved.")).toBeVisible();

    await page.reload();
    await expect(page.getByLabel("Upload schedule")).toHaveValue("hourly");
});

test("a backend address with a path is refused before saving", async ({ page }) => {
    await passNotice(page);
    await page.getByLabel("Backend address").fill("http://localhost:8080/api");
    await page.getByRole("button", { name: "Save backend settings" }).click();
    await expect(
        page.getByRole("region", { name: "Backend and upload" }).getByRole("alert"),
    ).toContainText("must not include a path");
});

test("turning upload on needs a saved backend before Sync now works", async ({ page }) => {
    await passNotice(page);
    await page.getByRole("checkbox", { name: "Upload trips to the backend" }).check();
    await expect(page.getByText("Upload is on.")).toBeVisible();
    await expect(page.getByRole("button", { name: "Sync now" })).toBeDisabled();
});
