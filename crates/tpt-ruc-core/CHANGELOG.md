# Changelog

All notable changes to `tpt-ruc-core` are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/). The workspace-wide history is in the [root changelog](../../CHANGELOG.md).

## [Unreleased]

### Added

- Cargo workspace member, `no_std` compatible with a `std` feature that is on by default.
- Validated `Latitude`, `Longitude` and `Coordinate` types. Out-of-range values are rejected at construction.
- NMEA 0183 parsing for GGA, RMC, VTG and GSA sentences, with checksum verification and multi-constellation talker IDs.
- Haversine great-circle distance, using `libm` so it works without `std`.
- `trip` module with a `Tracker` that filters fixes by fix quality, HDOP, implied jumps and stationary drift. Trips split into `Segment`s across signal gaps, with distances accumulated in metres and `billable_km` for the rounded figure.
- Duplicate-fix rejection and optional position smoothing in the tracker.
- Optional `serde` feature (on by default) with derives for `Coordinate`, `Fix`, `Config`, `Segment` and related types. Coordinates are validated on decode.
- `Fix` version 1 JSON golden file (`tests/golden/fix_v1.json`) and wire-format tests.
- Property tests: the NMEA parser must not panic on arbitrary input, and the trip total never decreases.
- `ruc` module: liability from the NZTA type code, RUC weight band, date and billable distance, in integer micro-dollars. GST-inclusive charges split into excl-GST and GST, and historical tables are selected by effective date.
- `ruc_rates` module with the NZTA distance-licence rates (powered, unpowered, cranes, vintage, motor caravans, overweight and H types), retrieved 2026-10-21 and effective from 2026-08-06.
- Electric vehicles owe nothing before `EV_LIABLE_FROM` (1 April 2024).
- Administration fees for online and agent purchases, and `split_gst` for GST-inclusive amounts.
- Additional overweight licences for powered, crane and combination vehicles, priced per km by extra weight (`additional_liability`).
- Heavy licence expiry one month after a rate change (`heavy_licence_expires_on`) and overlap-licence detection (`needs_overlap_licence`).
- `licence` module: licence status, remaining kilometres, overlap-licence distance, pro-rata refunds, and an off-road deduction that is refused unless Section 43 approval is flagged.
