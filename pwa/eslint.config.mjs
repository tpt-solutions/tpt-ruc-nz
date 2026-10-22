// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import js from "@eslint/js";
import globals from "globals";
import reactHooks from "eslint-plugin-react-hooks";
import { defineConfig, globalIgnores } from "eslint/config";
import tseslint from "typescript-eslint";

export default defineConfig([
    globalIgnores(["public/sync-worker.js", "out/**", "wasm-pkg/**", "coverage/**"]),
    js.configs.recommended,
    tseslint.configs.recommended,
    reactHooks.configs.flat.recommended,
    {
        // The service worker runs in its own global scope, which has no window or document.
        files: ["public/sw.js"],
        languageOptions: { globals: { ...globals.serviceworker } },
    },
    {
        // The app, its service worker and its tests run in the browser, or in Node for tooling.
        files: ["**/*.{ts,tsx,mjs,mts}"],
        languageOptions: {
            globals: { ...globals.browser, ...globals.node, ...globals.serviceworker },
        },
    },
]);
