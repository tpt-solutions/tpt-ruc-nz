# BLE design: stack and GATT layout (version 1)

Written for: the firmware team and the PWA team.

Status: **design, not yet implemented.** These are recommendations. The GATT layout below is the contract the PWA will code against, so change it only with a version bump.

## Stack

Recommended: `trouble-host` 0.8.0 (Apache-2.0 OR MIT) on `esp-radio` 1.0.0-beta.1 (MIT OR Apache-2.0), with `embassy-executor` 0.10.0 (MIT OR Apache-2.0) to run the tasks.

Why:
- Every crate is dual MIT or Apache-2.0, so it passes the device licence policy with the MIT option.
- It is an async, no_std host stack that runs on the same esp-hal used for the GNSS reader.
- The alternative, `bleps`, is smaller but less actively maintained. Check that before choosing it.

Open: `esp-radio` is a beta, so pin an exact version. Expect API changes before the stack is stable.

## Roles

- The device is the GATT server and advertises under the name `tpt-ruc-XXXX`, where `XXXX` is the last four hex digits of the device ID.
- The PWA is the client. It connects with Web Bluetooth where the browser supports it. iOS Safari and Firefox do not support Web Bluetooth. That limitation is accepted and documented in `todo.md`.

## Service and characteristics

Service UUID: `7b0e0001-5f4c-4a51-9c3d-7a6b5c4d3e2f` (a random UUID, reserved for this project). Each characteristic is 128-bit, with `...0001` through `...0004` in the first group.

| Characteristic | UUID suffix | Properties | Contents |
|---|---|---|---|
| Live fix | `...0002` | notify | One version 1 payload (see `crates/tpt-ruc-firmware/src/payload.rs`). Sent once per accepted fix, at most 1 Hz. |
| Segment transfer | `...0003` | notify | Chunks of the segment log (see below). |
| Control | `...0004` | write, notify | Commands from the PWA. Responses come back on the same characteristic. |
| Device info | `...0005` | read | Firmware version, chip name, payload version, and the log's used and total bytes. |

### Segment transfer

The phone asks for segments, and the device sends them in order, oldest first. A segment is sent only after the phone acknowledges the one before it, so a dropped connection loses nothing.

Chunk format, one notification per chunk:

`[seq: u16 LE][flags: u8][data...]`

- `seq` counts chunks within one transfer, from 0.
- `flags` bit 0 is set on the last chunk of a segment. Bit 1 is set on the last chunk of the transfer.
- `data` is the record bytes from the segment log, unchanged, so the phone can check them against the same CRC.
- The chunk size is the negotiated ATT MTU minus 3 bytes for the ATT header, minus 3 bytes for this header.

### Control commands

Written as a single byte command, followed by arguments.

| Command | Byte | Arguments | Response |
|---|---|---|---|
| Request segments | `0x01` | none | Starts a segment transfer. |
| Acknowledge segment | `0x02` | `u32 LE` segment count | Lets the device drop the acknowledged segments from its queue. |
| Set GNSS rate | `0x10` | `u8` Hz (1 or 5) | Acknowledged with `0x80`. |

Responses are `0x80` for success and `0x8F` for an unknown or invalid command.

## Security

- Pairing and bonding are not in version 1. The device accepts any client that connects while it is advertising. Anyone nearby can read the live fix and request segments. Treat this as a development stage only.
- Before a release build, the decision in `todo.md` applies: pairing approach and per-device ed25519 keys. Signing the segment records is planned for that step.

## Versioning

The payload version (currently 1) and this GATT layout are versioned separately. Add a new characteristic for new data. Never change the meaning of an existing one.

## Open items

- [ ] Choose the MTU negotiation target. The default ATT MTU is 23, which gives 17-byte chunks. Request 247 after connection.
- [ ] Decide the pairing approach (see `todo.md`).
- [ ] Measure the transfer rate on the C6 and S3 once the stack runs.
- [ ] Write the PWA client against the characteristic table above.
