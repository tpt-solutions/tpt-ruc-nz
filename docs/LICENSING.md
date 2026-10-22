# Licensing and SPDX policy

## Project licence

tpt-ruc-nz is dual licensed under **MIT OR Apache-2.0**. The copyright holder is **TPT Solutions**. The full texts are in [LICENSE-MIT](../LICENSE-MIT) and [LICENSE-APACHE](../LICENSE-APACHE).

## Dependency policy

Allowed licences: MIT, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, Unlicense, CC0-1.0, Unicode-3.0, Unicode-DFS-2016. Dual-licensed crates (for example `MIT OR Apache-2.0`) are accepted through their permissive option.

Rejected:

- **Apache-2.0-only** packages and crates in the shipped or production tree. The project is not Apache-only and they break the MIT chain. Dev-only tooling is allowed when it is not shipped: Playwright is a dev dependency of the PWA and is accepted (decided 2026-10-22).
- **CC-BY and other data licences** (for example `caniuse-lite`, CC-BY-4.0).
- **MPL-2.0**, which rules out uniffi and cbindgen.
- **GPL, LGPL, AGPL** and other copyleft licences, including LGPL libvips binaries pulled in by `sharp`.
- Protobuf via `prost`, which is Apache-only.

Enforced by `cargo deny check` (`deny.toml`) and the JS licence check for `pwa/` (`pwa/scripts/check-licences.mjs`, run by `npm run licenses`). Third-party notices are generated with `cargo about` (`about.toml`) into `THIRD_PARTY_LICENSES`.

### JavaScript exceptions

None are accepted. The JS check allows the MIT, ISC, BSD, 0BSD, Unlicense, CC0, BlueOak and Python-2.0 licences, and dual licences through their permissive option. `EXCEPTIONS` in the script is empty. Any future exception needs a named package, a reason and a review.

**Production scope.** The check covers the packages that run in production: the root `dependencies`, and what they pull in through `dependencies` and `optionalDependencies`. Peer dependencies and `devDependencies` are not checked, so Playwright, Vite, Vitest and the other build and test tools are out of scope. They are not shipped in `out/`.

The PWA is built with Vite (replaced Next.js on 2026-10-22). Its production dependencies are `idb`, `react`, `react-dom`, the wasm package and their own dependencies, and all of them are MIT.

## Source file headers

Every source file starts with two comment lines:

```rust
// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions
```

Use the matching comment syntax for other languages (`//` for TypeScript, `#` for YAML and shell). Manifest files (`Cargo.toml`, `package.json`) carry `license = "MIT OR Apache-2.0"` instead.

## Hardware

Hardware designs in `hardware/` will be licensed under a permissive open-hardware licence (CERN-OHL-P or CC-BY). The final choice is an open item in `todo.md`.
