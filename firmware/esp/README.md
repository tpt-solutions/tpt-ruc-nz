# tpt-ruc-esp

Device binary for the tracker. It reads the GNSS receiver on UART1 and records distance with `tpt-ruc-firmware`. Finished segments go to a flash log. The phone takes them over BLE, and the device serves live fixes and accepts control writes over the same link.

Status: **the code builds and links. Nothing has been flashed or run on a board.** The BLE link, the flash driver and the watchdog are unverified on hardware. The host tests in `tpt-ruc-firmware` cover the logic. They do not cover the radio, the flash timing, or the chip's behaviour.

## Layout

- `src/main.rs`: boot, pins, and the tasks. The `ble` feature runs an embassy executor with two tasks, GNSS and BLE.
- `src/device.rs`: flash and partition lookup, the device seed, the shared device state, and the GNSS transmitter.
- `src/ble.rs`: the GATT server (trouble-host), advertising, and per-connection work.
- `partitions.csv`: the partition table. It has a `tpt-ruc` data region for the seed and the log.

## Build

This crate is outside the host workspace, because each chip has its own target. The `ble` feature is on by default. It is off on the ESP32-P4, which has no radio.

- ESP32-C6 (RISC-V, the default, with BLE): `cargo build --release`
- ESP32-P4 (RISC-V, no BLE): `cargo build --release --no-default-features --features esp32p4 --target riscv32imafc-unknown-none-elf`
- ESP32-S3 (Xtensa, with BLE): install the toolchain with `espup install --targets esp32s3`, then
  `cargo +esp build --release --no-default-features --features esp32s3,ble --target xtensa-esp32s3-none-elf -Zbuild-std=core,alloc`

The C6 and P4 release builds were run on the build machine (Windows 11) and linked. **The S3 build was not run.** The espup toolchain is not installed on that machine. The S3 link warned that it cannot find the `_start` entry symbol, the same warning as the esp-hal template. Check the S3 image before relying on it.

## Flash and monitor

Flash with the partition table, so the `tpt-ruc` region exists:

`espflash flash --partition-table partitions.csv --monitor target/riscv32imac-unknown-none-elf/release/tpt-ruc-esp` (C6), or the matching path for S3.

The device refuses to start if the region is missing. It also refuses to start if the seed record is damaged, rather than making a new identity.

## Storage and identity

- The `tpt-ruc` region's first 4 KiB sector holds the device seed. It is written once, with the magic byte last. A power cut during first boot makes a new seed on the next boot, because the seed had not been handed out yet.
- The seed comes from the chip's TRNG. On the P4 it comes from the plain `Rng`, which is a weaker source. Treat P4 keys as test keys.
- The seed is stored in plain flash. Anyone who can read the flash, through JTAG or a flash dump, can read the seed. Enable flash encryption or a read-protected eFuse before any release. This is open in `todo.md`.
- The rest of the region is the trip log, in 4 KiB sectors.

## BLE

The stack is trouble-host 0.7.0 on esp-radio 1.0.0-beta.1, with embassy-executor 0.10.0 and esp-rtos 0.4.0. Versions are pinned. The design note asked for trouble-host 0.8.0, but 0.8.0 needs bt-hci 0.10. esp-radio 1.0.0-beta.1 uses bt-hci 0.9, so the two cannot share a controller. See `docs/ble-design.md`.

The advertised name is `tpt-ruc-` and the last two key bytes in hex. The GATT layout is in `docs/ble-design.md`. The MTU request of 247 is not made by this code. The stack uses the MTU the phone negotiates, which is 23 unless the phone asks for more. The chunk size follows the MTU. The transfer rate has not been measured.

## Pins

Pins match `hardware/README.md`. ESP32-S3: GNSS RX on GPIO16, TX on GPIO17. ESP32-C6: GNSS RX on GPIO4, TX on GPIO5. ESP32-P4: GPIO16 and GPIO17 are placeholders, not yet confirmed. Check them against the board before wiring.

## Watchdog and power

The watchdog is TIMG1, with a 10 second timeout. It is fed when GNSS bytes arrive, so a silent receiver resets the device. Light sleep is not used. It would need a wake plan for the UART and the BLE link, and that has not been designed.

## Memory

The BLE build uses a 72 KB `esp-alloc` heap, because the BLE stack allocates. The P4 build uses 8 KB, and only links the allocator symbols.
