# Changelog

All notable changes to this project are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/). Releases are managed as described in [docs/RELEASING.md](docs/RELEASING.md).

## [Unreleased]

### Added

- Repository foundations: dual MIT OR Apache-2.0 licence, `DESIGN_SPEC.md`, README with the NZTA Section 43 disclaimer, contributing and security policies, and licence policy.
- Cargo workspace with `tpt-ruc-core`, a `no_std` compatible crate with a `std` feature.
- `tpt-ruc-core`: validated `Latitude`, `Longitude` and `Coordinate` types.
- `tpt-ruc-core`: NMEA 0183 parsing for GGA, RMC, VTG and GSA sentences, with checksum verification and multi-constellation talker IDs.
- `tpt-ruc-core`: Haversine great-circle distance using `libm`.
- CI: formatting, clippy, tests, `no_std` builds for thumbv7em, riscv32imc and wasm32, and `cargo-deny`.
