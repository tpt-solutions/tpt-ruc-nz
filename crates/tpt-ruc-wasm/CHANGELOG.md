# Changelog

All notable changes to `tpt-ruc-wasm` are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/). The workspace-wide history is in the [root changelog](../../CHANGELOG.md).

## [Unreleased]

### Added

- `wasm-bindgen` bindings for NMEA parsing (`parseNmea`), great-circle distance (`haversineM`), billable kilometres (`billableKm`), RUC liability (`liability`) and administration fees (`adminFeeInclGstMicros`).
- `TripTracker`, which takes JSON fixes and reports accepted and rejected samples, with `totalM` for the running distance.
- `heavyLicenceExpiresOn` and `currentRateEffective` for the heavy-licence expiry date and the effective date of the current rate table.
- Node tests run with `wasm-pack test --node`.
- `scripts/build-wasm.sh` builds the package for `pwa/`.

### Changed

- Release profile tuned for size. The wasm module is 143 KB, or 70 KB gzipped, down from 161 KB.
