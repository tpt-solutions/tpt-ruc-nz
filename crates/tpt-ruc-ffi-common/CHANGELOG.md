# Changelog

All notable changes to `tpt-ruc-ffi-common` are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/). The workspace-wide history is in the [root changelog](../../CHANGELOG.md).

## [Unreleased]

### Added

- Language-neutral API for the mobile bindings: `liability_incl_gst_micros`, which returns the GST-inclusive charge in micro-dollars, and `billable_km`.
- Inputs are plain values (strings, integers and floats). Impossible dates and unknown vehicle type codes are rejected with a message.
