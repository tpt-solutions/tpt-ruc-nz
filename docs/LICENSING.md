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

Enforced by `cargo deny check` (`deny.toml`) and the JS licence allow-list for `pwa/` (added in Phase 2). Third-party notices are generated with `cargo about` (`about.toml`) into `THIRD_PARTY_LICENSES`.

## Source file headers

Every source file starts with two comment lines:

```rust
// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions
```

Use the matching comment syntax for other languages (`//` for TypeScript, `#` for YAML and shell). Manifest files (`Cargo.toml`, `package.json`) carry `license = "MIT OR Apache-2.0"` instead.

## Hardware

Hardware designs in `hardware/` will be licensed under a permissive open-hardware licence (CERN-OHL-P or CC-BY). The final choice is an open item in `todo.md`.
