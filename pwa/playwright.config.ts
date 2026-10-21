import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
    testDir: "e2e",
    use: {
        baseURL: "http://localhost:3000",
    },
    projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
    webServer: {
        command: "npx serve out -l 3000",
        url: "http://localhost:3000",
        reuseExistingServer: true,
    },
});
