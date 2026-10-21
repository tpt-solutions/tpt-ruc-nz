# Over-the-air update plan

Written for: the firmware team and the owner.

Status: **decision: defer OTA until after v1.** Nothing is built, and no board has been flashed yet.

## Decision

Version 1 ships without over-the-air (OTA) updates. A firmware fix is made by reflashing the device over USB or by replacing it. The field fleet is small, the devices are installed in vehicles that the owner can reach, and the risk of a bad update reaching a truck on a road is larger than the cost of a visit.

## Why defer

- **No signing infrastructure yet.** A signed update needs a firmware signing key, held somewhere safe, with a documented rotation. That key is not chosen, and the device identity key (`docs/pairing.md`) must not be reused for it.
- **No board to test on.** A rollback path cannot be claimed until it has survived real power cuts on the ESP32 flash layout. The flash log (`flash.rs`) is only tested on the host.
- **Radio time.** A firmware image is hundreds of kilobytes. Sent as BLE notifications at the transfer rate in `docs/ble-design.md`, that is long enough to conflict with the trip recording that the device is there to do.
- **The trip data must not be lost.** An update that resets the device during a trip must not lose the segments in the flash log. That needs a measured flash layout first.

## Minimum requirements if OTA is adopted later

1. **Signed images.** Every image is signed with a firmware key that is separate from the device identity key. The bootloader checks the signature before it boots. Unsigned or badly signed images are refused.
2. **Two slots with rollback.** The new image goes to the inactive slot. The bootloader switches only after the image has booted and confirmed itself. A failed boot reverts to the old slot.
3. **Anti-rollback.** A version counter in protected storage stops an older signed image from being installed over a newer one, so a known-bad release cannot be forced back.
4. **Data safety.** The update must not erase the segment log. The log's partition is separate from the image slots, and the update checks that its partition is intact before it starts.
5. **Interrupted updates resume or revert.** A dropped BLE link or a power cut leaves the device on a working image, never a half-written one.
6. **Consent and timing.** The phone asks the owner before it starts, and the update runs only while the vehicle is not moving.
7. **Tests.** Host tests for the signature and anti-rollback logic. On-board tests for the power-cut cases, which need hardware.

## Open items

- Choose the firmware signing key and where it is kept.
- Choose the ESP32 bootloader and partition table (esp-hal and esp-bootloader support to be checked).
- Decide whether updates are sent over BLE, or fetched by the phone from the backend and then sent over BLE.
