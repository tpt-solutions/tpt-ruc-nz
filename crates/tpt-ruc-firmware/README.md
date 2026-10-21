# tpt-ruc-firmware

Device logic for the tpt-ruc-nz GNSS tracker. The crate turns a raw GNSS UART byte stream into trip segments, stores them on flash, and frames them for BLE. It contains no hardware code, so almost all of it runs and is tested on the host.

> **Status: early.** The UART driver, flash driver and BLE service are not in this crate yet. The device binary is in `firmware/esp` (excluded from the workspace). It builds for the ESP32 targets but has **not been flashed or run on a board**. See [hardware/README.md](../../hardware/README.md) and [docs/ble-design.md](../../docs/ble-design.md).

## Contents

- [What it does](#what-it-does)
- [Modules](#modules)
- [Chip features](#chip-features)
- [Testing](#testing)
- [Wire formats](#wire-formats)
- [Licence](#licence)

## What it does

```
GNSS UART bytes
   -> nmea line framing      (lib.rs: LineBuffer)
   -> sentence parsing       (tpt-ruc-core)
   -> Recorder               (trip filter from tpt-ruc-core)
   -> SegmentQueue           (bounded; counts dropped segments)
   -> SegmentLog             (CRC-checked, append-only, recovers after power loss)
   -> transfer / payload     (BLE chunks and MessagePack fixes)
```

- **Line framing.** Bytes become NMEA lines. A line longer than `MAX_SENTENCE` (96 bytes) is discarded, so a noisy receiver cannot grow the buffer.
- **Fix merging.** A GGA sentence is merged with the date from RMC and the speed from VTG, then converted to Unix milliseconds.
- **Recording.** `Recorder` runs the `tpt-ruc-core` trip tracker on the device and emits finished segments.
- **Storage.** `SegmentLog` writes segments over a `LogStorage` trait. It recovers after power loss and refuses writes when full. `SectorLog` (in `flash`) is the flash-oriented version with round-robin wear levelling and power-cut tests. Both are tested with RAM-backed storage.
- **Identity.** Each device has an Ed25519 identity (`identity`), and uploads can be signed with it.
- **Control.** The phone writes commands to change the GNSS rate, the max HDOP and the minimum movement, and each command gets a response code (`control`).

## Modules

| Module | Contents |
|---|---|
| `recorder` | `Recorder`: runs the trip tracker over fixes and emits segments. |
| `queue` | `SegmentQueue`: bounded queue of finished segments. Counts drops when full. |
| `log` | `SegmentLog`, `LogStorage`, `LogError`: append-only, CRC-checked segment records. |
| `flash` | `SectorLog`, `SectorStorage`: sector-based log with round-robin wear levelling. |
| `payload` | Version 1 BLE payload for one fix, encoded as MessagePack with `rmp`. |
| `transfer` | Chunking of segment log records into BLE notifications, with sequence numbers and end flags. |
| `control` | Command parsing (`Command`, `Settings`, `response_for`) for the phone's control writes. |
| `identity` | `DeviceIdentity` (Ed25519 key from a seed), `verify`. |

`CHIP` names the chip this build targets, or `"host"` when no chip feature is set.

## Chip features

Enable exactly one chip feature for a device build. The crate refuses to compile with two.

| Feature | Target |
|---|---|
| `esp32s3` | ESP32-S3 |
| `esp32c3` | ESP32-C3 |
| `esp32c6` | ESP32-C6 |
| `esp32p4` | ESP32-P4 |

With no feature enabled, the crate builds for the host. The features only record the target. The GPIO and UART code that depends on them is not here yet.

## Testing

The host tests need no hardware:

```sh
cargo test -p tpt-ruc-firmware
```

They cover line framing and overflow, fix merging, the recorder's segment output, the queue's drop count, log recovery after a simulated power cut, the sector log's wear levelling, command parsing, and the golden bytes for the BLE payload and chunk framing. The PWA's BLE decoder is tested against the same golden bytes, so the two sides stay in step.

To check the `no_std` build for a microcontroller target:

```sh
cargo check -p tpt-ruc-firmware --no-default-features --target thumbv7em-none-eabihf
```

## Wire formats

Changes to these formats need a new version number:

- **Fix payload (version 1).** A fixed-order MessagePack array, `[version, time_ms, lat_e7, lon_e7, speed_cm_s, fix_quality]`. See `src/payload.rs`.
- **Transfer chunk (version 1).** `[seq: u16 LE][flags: u8][data...]`. Bit 0 marks the last chunk of a segment and bit 1 the last chunk of the transfer. See `src/transfer.rs`.
- **Control commands.** Command bytes and bounds are in `src/control.rs`. The table there lists the settings added for the trip filter, which must also appear in `docs/ble-design.md` before a release.

## Licence

Dual licensed under [MIT](../../LICENSE-MIT) or [Apache-2.0](../../LICENSE-APACHE), at your option.
