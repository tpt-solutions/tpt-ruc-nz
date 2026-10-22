// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions
//
// Checks the production dependency tree against the project licence policy
// (docs/LICENSING.md). Every package must be on the allow list, or match a
// named exception with a recorded reason. Exceptions are printed on each run
// so they stay visible.

import { readFileSync } from "node:fs";
import { join } from "node:path";
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

/**
 * Exceptions to the allow list. Each one needs a reason and a review.
 * None are accepted: Apache-2.0-only, LGPL and CC-BY packages break the MIT chain.
 */
const EXCEPTIONS = [];

function isAllowed(licence) {
    if (ALLOWED.has(licence)) return true;
    // A dual licence is accepted through its permissive option (for example MIT OR Apache-2.0).
    const parts = licence.split(/\s+OR\s+/).map((p) => p.trim().replace(/[()]/g, ""));
    return parts.some((p) => ALLOWED.has(p));
}

/**
 * Names of the packages that run in production: the root's `dependencies`, and
 * everything they pull in through `dependencies` and `optionalDependencies`.
 * Peer dependencies are not followed, so test tooling that a package lists as an
 * optional peer, such as Playwright under some frameworks, is not counted. Packages are read from the top of
 * node_modules by name, which is enough here because a nested copy has the same
 * name as the top-level one.
 */
function productionNames() {
    const root = JSON.parse(readFileSync(join(process.cwd(), "package.json"), "utf8"));
    const names = new Set();
    const queue = Object.keys(root.dependencies ?? {});
    while (queue.length > 0) {
        const name = queue.shift();
        if (names.has(name)) continue;
        names.add(name);
        let manifest;
        try {
            manifest = JSON.parse(
                readFileSync(join(process.cwd(), "node_modules", name, "package.json"), "utf8"),
            );
        } catch {
            // Not installed for this platform, such as an optional native binary.
            continue;
        }
        queue.push(...Object.keys(manifest.dependencies ?? {}));
        queue.push(...Object.keys(manifest.optionalDependencies ?? {}));
    }
    return names;
}

const production = productionNames();

checker.init({ start: process.cwd(), production: true }, (error, allPackages) => {
    if (error) {
        console.error(error);
        process.exit(1);
    }

    const failures = [];
    const used = new Map();
    const packages = Object.fromEntries(
        Object.entries(allPackages).filter(([key]) =>
            production.has(key.slice(0, key.lastIndexOf("@"))),
        ),
    );

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
