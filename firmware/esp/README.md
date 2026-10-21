# tpt-ruc-esp

Device binary for the tracker. It reads the GNSS receiver on UART1, records distance with `tpt-ruc-firmware`, and prints each finished trip segment on the USB-Serial-JTAG console.

Status: **best effort, not yet flashed or verified on a board.**

## Build

This crate is outside the host workspace, because each chip has its own target. All three chips below link as release binaries on the build machine (Windows 11, esp-hal 1.2.2). None has been flashed or run on a board yet.

- ESP32-C6 (RISC-V, the default): `cargo build --release`
- ESP32-P4 (RISC-V): `cargo build --release --no-default-features --features esp32p4 --target riscv32imafc-unknown-none-elf`
- ESP32-S3 (Xtensa): install the toolchain with `espup install --targets esp32s3`, then
  `cargo +esp build --release --no-default-features --features esp32s3 --target xtensa-esp32s3-none-elf -Zbuild-std=core,alloc`

The S3 link prints a warning that it cannot find the `_start` entry symbol. The esp-hal template uses `-nostartfiles`, which causes it. Check the image on the board before relying on it.

## Flash and monitor

`espflash flash --monitor target/riscv32imac-unknown-none-elf/release/tpt-ruc-esp` (C6), or the matching path for S3.

## Pins

Pins match `hardware/README.md`. ESP32-S3: GNSS RX on GPIO16, TX on GPIO17. ESP32-C6: GNSS RX on GPIO4, TX on GPIO5. ESP32-P4: GPIO16 and GPIO17 are placeholders, not yet confirmed. Check them against the board before wiring.

## Memory

The code allocates nothing. The 8 KB `esp-alloc` heap exists only because a dependency links the allocator symbols.
