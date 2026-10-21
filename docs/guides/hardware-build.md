# Building the bench GPS rig

Written for: the person building the breadboard test setup for the GPS receiver.

> **Status: untested.** The wiring below has not been built or measured. The firmware's GPS reader is written but has never run on a board, so this guide stops at a terminal check. Everything marked *untested* is a plan, not a result.

This guide uses the choices in [`hardware/README.md`](../../hardware/README.md) and the parts in [`hardware/bench-bom.csv`](../../hardware/bench-bom.csv).

## Parts

| Part | Notes |
|---|---|
| ESP32-S3-DevKitC-1 | Main board. Xtensa. |
| ESP32-C3-DevKitM-1 | Optional second board. RISC-V. |
| u-blox NEO-M8N breakout with an active antenna | Check the breakout runs at 3.3 V before you connect it. |
| Active GNSS antenna | Its connector must match the breakout. |
| USB-C data cable | One per board. A charge-only cable will not work. |
| Jumper wires, breadboard | Male to female jumper wires for the GPS link. |

## Before you connect anything

1. Check the breakout's voltage. It must be 3.3 V. Do not connect a 5 V breakout to a 3.3 V pin, and do not feed 5 V into the board.
2. Check the antenna connector matches the breakout's.
3. Unplug the board from USB while you wire it.

## Wiring

Use the pins in `hardware/README.md`. For the ESP32-S3:

| Signal | DevKit pin | Breakout pin |
|---|---|---|
| Receiver sends data to the board | GPIO16 (RX) | TX |
| Board sends data to the receiver | GPIO17 (TX) | RX |
| Power | 3V3 | VCC |
| Ground | GND | GND |

Connect ground first. Share one ground between the board and the breakout. The antenna plugs into the breakout, not the board.

## Check the receiver before the firmware

You can check the wiring without any firmware. Connect the board's USB port, then use any serial terminal at **9600 baud, 8 data bits, no parity, 1 stop bit**. The receiver sends text sentences. Look for lines that start with `$GN` or `$GP`, such as `$GNGGA` and `$GNRMC`.

- **No text at all.** Check the power and the TX and RX wires. Swapped TX and RX is the most common fault.
- **Text, but nonsense characters.** The baud rate is wrong. Try 115200.
- **Sentences, but `V` or no fix.** The receiver needs a clear view of the sky. Stand outside, and wait a few minutes after a cold start.

To read the sentences on a computer, use a terminal such as PuTTY on Windows, or `screen` on Mac and Linux. Pick the serial port the board shows up on.

## Open items (not yet tested)

- [ ] Confirm the NEO-M8N breakout's UART level on the actual board.
- [ ] Measure the real sentence rate and time to first fix at 9600 baud and 1 Hz.
- [ ] Capture a log of GGA and RMC sentences from the receiver, and keep it with the test notes.
- [ ] Flash the existing UART reader in `firmware/esp` to a board, and check it against the terminal log.

## What is not covered here

- **Enclosure, PCB and 3D files.** BLOCKED. No CAD or PCB tool is available in the build environment, so these files are not in the repository. See [`docs/pcb-build.md`](../pcb-build.md).
- **Power from a vehicle.** Not designed yet. The bench rig runs from USB only.
- **Safety.** The bench rig is for testing on a desk. Do not install it in a vehicle until the power design and its protections are done.
