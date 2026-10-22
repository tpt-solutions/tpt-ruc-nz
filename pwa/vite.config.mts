// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import react from "@vitejs/plugin-react";
import { fileURLToPath, URL } from "node:url";
import { defineConfig } from "vite";

// Static build: the PWA is served as plain files, with no Node server.
// The output goes to `out/`, where the Playwright server and the backend look for it.
export default defineConfig({
    plugins: [react()],
    resolve: {
        alias: { "@": fileURLToPath(new URL(".", import.meta.url)) },
    },
    build: {
        outDir: "out",
        emptyOutDir: true,
    },
});
