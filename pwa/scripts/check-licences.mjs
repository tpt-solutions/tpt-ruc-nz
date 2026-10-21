// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions
//
// Checks the production dependency tree against the project licence policy
// (docs/LICENSING.md). Every package must be on the allow list, or match a
// named exception with a recorded reason. Exceptions are printed on each run
// so they stay visible.

import checker from "license-checker";

const ALLOWED = new Set([
    "MIT",
    "ISC",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "0BSD",
    "Unlicense",
    "CC0-1.0",
    "BlueOak-1.0.0",
    "Python-2.0",
]);

/** Exceptions to the allow list. Each one needs a reason and a review. */
const EXCEPTIONS = [
    {
        match: (name) => name === "sharp" || name.startsWith("@img/sharp"),
        licences: ["Apache-2.0", "Apache-2.0 AND LGPL-3.0-or-later"],
        reason: "Optional image optimiser of Next.js. Unused, because the PWA is a static export with unoptimised images, so it is not in out/. Needs removing from the install tree.",
    },
    {
        match: (name) => name === "@swc/helpers",
        licences: ["Apache-2.0"],
        reason: "Runtime helper that Next.js uses in its output. Needs confirming against the policy.",
    },
    {
        match: (name) => name === "detect-libc",
        licences: ["Apache-2.0"],
        reason: "Platform detection used by the sharp install.",
    },
    {
        match: (name) => name.startsWith("@playwright/") || name.startsWith("playwright"),
        licences: ["Apache-2.0"],
        reason: "End-to-end test tooling. Not shipped.",
    },
    {
        match: (name) => name === "baseline-browser-mapping",
        licences: ["Apache-2.0"],
        reason: "Build-time browser data used by Next.js. Not shipped.",
    },
    {
        match: (name) => name === "caniuse-lite",
        licences: ["CC-BY-4.0"],
        reason: "Build-time browser support data. Not shipped.",
    },
];

function isAllowed(licence) {
    if (ALLOWED.has(licence)) return true;
    // A dual licence is accepted through its permissive option (for example MIT OR Apache-2.0).
    const parts = licence.split(/\s+OR\s+/).map((p) => p.trim().replace(/[()]/g, ""));
    return parts.some((p) => ALLOWED.has(p));
}

checker.init({ start: process.cwd(), production: true }, (error, packages) => {
    if (error) {
        console.error(error);
        process.exit(1);
    }

    const failures = [];
    const used = new Map();

    for (const [key, info] of Object.entries(packages)) {
        if (info.private) continue;
        const name = key.slice(0, key.lastIndexOf("@"));
        const licence = String(info.licenses);
        if (isAllowed(licence)) continue;

        const exception = EXCEPTIONS.find((e) => e.match(name) && e.licences.includes(licence));
        if (exception) {
            used.set(exception, [...(used.get(exception) ?? []), key]);
            continue;
        }
        failures.push(`${key}: ${licence}`);
    }

    if (used.size > 0) {
        console.log("Exceptions in use:");
        for (const [exception, keys] of used) {
            console.log(
                `  ${exception.licences.join(" or ")} (${keys.length} packages): ${exception.reason}`,
            );
        }
    }

    if (failures.length > 0) {
        console.error("Licences not permitted by docs/LICENSING.md:");
        for (const failure of failures) console.error(`  ${failure}`);
        process.exit(1);
    }

    console.log(`All ${Object.keys(packages).length} production packages pass the licence policy.`);
});
