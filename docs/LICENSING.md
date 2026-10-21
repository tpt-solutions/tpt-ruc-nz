# Licensing and SPDX policy

## Project licence

tpt-ruc-nz is dual licensed under **MIT OR Apache-2.0**. The copyright holder is **TPT Solutions**. The full texts are in [LICENSE-MIT](../LICENSE-MIT) and [LICENSE-APACHE](../LICENSE-APACHE).

## Dependency policy

Allowed licences: MIT, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, Unlicense, CC0-1.0, Unicode-3.0, Unicode-DFS-2016. Dual-licensed crates (for example `MIT OR Apache-2.0`) are accepted through their permissive option.

Rejected:

- **Apache-2.0-only** crates, because the project is not Apache-only and they are not in the allow list.
- **MPL-2.0**, which rules out uniffi and cbindgen.
- **GPL, LGPL, AGPL** and other copyleft licences.
- Protobuf via `prost`, which is Apache-only.

Enforced by `cargo deny check` (`deny.toml`) and the JS licence check for `pwa/` (`pwa/scripts/check-licences.mjs`, run by `npm run licenses`). Third-party notices are generated with `cargo about` (`about.toml`) into `THIRD_PARTY_LICENSES`.

### JavaScript exceptions (accepted)

The JS check allows the MIT, ISC, BSD, 0BSD, Unlicense, CC0, BlueOak and Python-2.0 licences, and dual licences through their permissive option. It also accepts these named exceptions, each with a reason in the script:

- **`sharp` and `@img/*`** (Apache-2.0, LGPL-3.0 libvips binaries): optional image optimiser of Next.js. Not used by the static export and not in `out/`. The LGPL binary is not shipped, but it is in the install tree. Removing it would clear the exception.
- **`@swc/helpers`** (Apache-2.0): a runtime helper that Next.js uses in its output. It ships in the bundle, so this needs an explicit decision.
- **`detect-libc`** (Apache-2.0): platform detection for the sharp install.
- **Playwright** (Apache-2.0), **`baseline-browser-mapping`** (Apache-2.0) and **`caniuse-lite`** (CC-BY-4.0): build-time and test tooling, not shipped.

These exceptions were accepted on the maintainer's instruction to follow the recommendations. Remove the `sharp` exception once Next.js no longer installs it.

## Source file headers

Every source file starts with two comment lines:

```rust
// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions
```

Use the matching comment syntax for other languages (`//` for TypeScript, `#` for YAML and shell). Manifest files (`Cargo.toml`, `package.json`) carry `license = "MIT OR Apache-2.0"` instead.

## Hardware

Hardware designs in `hardware/` will be licensed under a permissive open-hardware licence (CERN-OHL-P or CC-BY). The final choice is an open item in `todo.md`.
