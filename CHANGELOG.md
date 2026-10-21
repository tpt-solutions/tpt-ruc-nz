# Changelog

All notable changes to this project are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/). Releases are managed as described in [docs/RELEASING.md](docs/RELEASING.md).

## [Unreleased]

### Added

- Repository foundations: dual MIT OR Apache-2.0 licence, `DESIGN_SPEC.md`, README with the NZTA Section 43 disclaimer, contributing and security policies, and licence policy.
- Cargo workspace with `tpt-ruc-core`, a `no_std` compatible crate with a `std` feature.
- `tpt-ruc-core`: validated `Latitude`, `Longitude` and `Coordinate` types.
- `tpt-ruc-core`: NMEA 0183 parsing for GGA, RMC, VTG and GSA sentences, with checksum verification and multi-constellation talker IDs.
- `tpt-ruc-core`: Haversine great-circle distance using `libm`.
- `tpt-ruc-core`: `trip` module with a `Tracker` that filters fixes by fix quality, HDOP, implied jumps and stationary drift, and splits trips into `Segment`s across signal gaps. Distances accumulate in metres, with `billable_km` for the final rounded figure.
- `tpt-ruc-core`: optional `serde` feature (on by default) with derives for `Coordinate`, `Fix`, `Config`, `Segment` and related types. Coordinates are validated on decode.
- `tpt-ruc-core`: `Fix` version 1 JSON golden file (`tests/golden/fix_v1.json`) and wire-format tests.
- `tpt-ruc-core`: property tests for the NMEA parser (no panics on arbitrary input) and for the trip tracker (total never decreases).
- `tpt-ruc-core`: `ruc` module: liability from NZTA type code, RUC weight band, date and billable distance, in integer micro-dollars. GST-inclusive charges are split into excl-GST and GST, and historical tables are selected by effective date.
- `tpt-ruc-core`: `ruc_rates` module with the NZTA distance-licence rates (powered, unpowered, cranes, vintage, motor caravans, overweight and H types) retrieved 2026-10-21, effective from 2026-08-06. Electric vehicles owe nothing before `EV_LIABLE_FROM` (1 April 2024). Administration fees for online and agent purchases, and `split_gst` for GST-inclusive amounts. Additional overweight licences for powered, crane and combination vehicles, priced per km by extra weight (`additional_liability`). Heavy licence expiry one month after a rate change (`heavy_licence_expires_on`) and overlap-licence detection (`needs_overlap_licence`).
- `tpt-ruc-wasm`: wasm-bindgen bindings for NMEA parsing, distance, RUC liability, administration fees, billable kilometres and a trip tracker that takes JSON fixes. Tests run in Node with `wasm-pack test --node`, and `scripts/build-wasm.sh` builds the package for `pwa/`.
- Third-party licence notice (`THIRD_PARTY_LICENSES.html`) generated with cargo-about, and a CI job that fails on unaccepted licences.
- `pwa/`: Next.js 16 static-export PWA scaffold with TypeScript, Tailwind CSS 4, ESLint, Prettier, Vitest and Playwright. Includes a web app manifest, a home page with the Section 43 notice, and a JS licence check with named exceptions.
- `pwa/`: offline shell service worker (stale-while-revalidate, tested offline in Chromium), and an IndexedDB storage module with versioned migrations and backup validation.
- `pwa/`: RUC calculator on the home page, backed by the wasm liability function, with end-to-end tests.
- `pwa/`: Section 43 first-run acknowledgement that gates the app, vehicle profiles (create, list, delete) in IndexedDB, and end-to-end tests for each.
- `pwa/`: vehicle editing, manual trip entry with a trip list and CSV export, and PNG app icons. Covered by unit and end-to-end tests.
- `pwa/`: backup export and restore from a JSON file, with an end-to-end round trip and a test that refuses a file that is not a backup.
- `pwa/`: optional manual odometer reading per vehicle, with validation and end-to-end tests.
- `pwa/`: storage status with a persistence request and an 80% quota warning, and an offline test that runs the WASM calculator from the cache.
- `pwa/`: end-to-end check of the installability requirements (manifest fields, PNG icons, and a controlling service worker).
- `pwa/`: settings section that shows the Section 43 notice again, with an end-to-end test.
- Release profile tuned for size (LTO, one codegen unit, `opt-level = "s"`). The wasm module is 143 KB, or 70 KB gzipped, down from 161 KB.
- `pwa/`: trip detail view with the estimated RUC charge, priced on the trip's date with the wasm module, and an end-to-end test.
- CI: a `pwa` job runs lint, format, unit tests, the licence check, the build and the end-to-end tests.
- `tpt-ruc-firmware`: device-side GNSS logic, host-tested: NMEA line framing with overflow handling, GGA merged with the RMC date and speed, and conversion to Unix milliseconds. No hardware code yet.
- `hardware/`: bench notes with the recommended boards, GNSS receiver, UART settings and pin map, and a BOM draft. Not yet verified on hardware.
- CI: formatting, clippy, tests, `no_std` builds for thumbv7em, riscv32imc and wasm32, and `cargo-deny`.
