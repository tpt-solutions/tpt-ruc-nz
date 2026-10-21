# Changelog

All notable changes to `tpt-ruc-firmware` are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/). The workspace-wide history is in the [root changelog](../../CHANGELOG.md).

## [Unreleased]

### Added

- Device-side GNSS logic, host-tested: NMEA line framing with overflow handling, GGA merged with the RMC date and speed, and conversion to Unix milliseconds. No hardware code yet.
- Chip feature flags (`esp32s3`, `esp32c3`, `esp32c6`, `esp32p4`). Enabling more than one is a compile error.
- `Recorder`, which runs the `tpt-ruc-core` trip tracker on the device.
- A bounded `SegmentQueue` that counts dropped segments when full.
- Version 1 MessagePack BLE payload for one fix (`rmp`, `no_std`).
- Append-only, CRC-checked `SegmentLog` over a `LogStorage` trait. It recovers after power loss and refuses writes when full. Tested with RAM-backed storage.
- `SectorLog` for NOR flash, with round-robin wear levelling and power-cut tests.
- A per-device Ed25519 identity (`DeviceIdentity`) and `verify`.
- Host-side BLE transfer chunking with sequence numbers and end flags, and control-command parsing for GNSS rate, max HDOP and minimum movement.

### Notes

- The ESP-IDF or esp-hal UART driver, the flash driver and the BLE service are not in this crate yet. The device binary in `firmware/esp` is excluded from the workspace and has not been flashed or run on a board.
