# tpt-ruc-nz

A privacy-first, open-source toolkit for tracking and estimating New Zealand Road User Charges (RUC): a shared Rust core, an ESP32 reference GPS node, a static PWA, and a self-hostable backend.

> **Important: NZTA Section 43 disclaimer**
>
> tpt-ruc-nz is an open-source reference implementation for personal tracking, estimation and fleet telemetry. **It is not an NZTA-approved distance recorder.**
>
> Under the NZ Road User Charges Act, claiming RUC deductions (such as off-road travel), or using a digital display in place of a physical licence label, requires an NZTA Approved Distance Recorder under Section 43. If you intend to use this system for official RUC licensing or tax deductions, you must apply for Section 43 approval through NZTA first.
>
> Rates in this project are not authoritative until they have been verified against [nzta.govt.nz](https://www.nzta.govt.nz). See [DESIGN_SPEC.md](DESIGN_SPEC.md).

## Status

Early development. Phase 0 (foundations) and Phase 1 (core maths) are in progress. See [todo.md](todo.md) for the full checklist.

| Component | Path | Status |
|---|---|---|
| Core: NMEA parsing, Haversine distance, coordinates | `crates/tpt-ruc-core` | Working, 27 tests |
| RUC rate engine | `crates/tpt-ruc-core` | Not started (rates need verification) |
| WASM bindings and PWA | `crates/tpt-ruc-wasm`, `pwa/` | Planned (Phase 2) |
| ESP32 firmware | `crates/tpt-ruc-firmware` | Planned (Phase 3) |
| Backend | `crates/tpt-ruc-backend` | Planned (Phase 4) |

## Repository layout

```
crates/tpt-ruc-core/   Pure Rust logic, no_std compatible
pwa/                   Progressive Web App (Phase 2)
hardware/              KiCad, BOM, enclosure (Phase 3 onward)
docs/                  User guides and NZTA compliance notes
DESIGN_SPEC.md         Design specification and locked decisions
```

## Building

Requires the stable Rust toolchain (pinned in `rust-toolchain.toml`).

```sh
cargo test --workspace

# no_std check for microcontroller and WASM targets
cargo check -p tpt-ruc-core --no-default-features --target thumbv7em-none-eabihf
```

## Licence

Dual licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
Copyright (c) 2026 TPT Solutions.

Every dependency must be MIT, BSD, ISC, Zlib, Unlicense/CC0 or dual MIT/Apache-2.0. This is enforced in CI by `cargo-deny` (see `deny.toml`).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). To report a security issue, see [SECURITY.md](SECURITY.md).
