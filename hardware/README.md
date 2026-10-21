# Hardware

Written for: the person building the bench rig and the firmware team.

Status: **best effort, not yet verified on hardware.** The choices below are recommendations. The wiring has not been built or tested, and the firmware UART driver has not been written.

## Bench choices (recommended)

| Item | Choice | Why |
|---|---|---|
| Primary MCU | ESP32-S3-DevKitC-1 (Xtensa) | The spec's main firmware target. Has Wi-Fi and BLE, and enough RAM for trip buffering. |
| Second MCU | ESP32-C3-DevKitM-1 (RISC-V) | The RISC-V variant, to prove the single-crate, chip-feature-flag design. |
| GNSS receiver | u-blox NEO-M8N breakout with an active antenna | Widely documented, NMEA over UART, multi-constellation. Quectel L80 is the fallback. |
| UART settings | 9600 baud, 8N1 | The NEO-M8N default. Start here, then raise to 115200 if the update rate needs it. |
| Update rate | 1 Hz, with GGA, RMC, VTG and GSA enabled | Enough for distance accumulation at road speeds. Raise to 5 Hz later if needed. |

## Pin map (bench, proposed)

GNSS UART is `UART1`. Any free GPIO pair works, but the pins below avoid the boot-strapping pins.

| Signal | ESP32-S3 (DevKitC-1) | ESP32-C3 (DevKitM-1) |
|---|---|---|
| GNSS TX → MCU RX | GPIO16 | GPIO4 |
| MCU TX → GNSS RX | GPIO17 | GPIO5 |
| GNSS 3V3 | 3V3 | 3V3 |
| GND | GND | GND |

The GNSS breakout runs from the 3.3 V rail. Do not feed it 5 V. Share one ground between the MCU and the breakout.

```
 USB-C (5 V) ──> DevKit ──3V3──> NEO-M8N breakout (VCC)
                  │  GPIO16 <── TX          antenna ──> active GNSS antenna
                  │  GPIO17 ──> RX
                  └──GND────────GND
```

## Bench BOM (draft)

See [`bench-bom.csv`](bench-bom.csv). Prices are left blank, to be quoted by the buyer.

## Open items

- [ ] Order the boards and confirm the NEO-M8N breakout's UART level (3.3 V) before wiring.
- [ ] Measure the real sentence rate and cold-start time at 9600 baud and 1 Hz.
- [ ] Build the breadboard wiring and capture a log of GGA and RMC from the receiver.
- [ ] Write the esp-hal UART reader, feeding `tpt-ruc-firmware::GnssReader`.
- [ ] Decide the flash log format (LittleFS or log-structured) and the BLE GATT layout. Both are still open.
