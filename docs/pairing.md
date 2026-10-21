# Pairing and device identity

Written for: the firmware team, the PWA team and the backend team.

Status: **decided 2026-10-22**, following the recommendation below. Nothing here is built. The device has no display or keyboard, and no board has run the BLE stack, so every claim about the radio behaviour below is unverified.

## What is already decided

- Each device has its own Ed25519 key pair (`todo.md`, Decisions locked in: "Per-device ed25519 keys").
- The device signs what it hands to the phone. The phone and backend check the signature with the public key the device registered (`POST /v1/devices`).
- The seed for the key pair comes from the chip's random number generator on first boot, and is stored on the device. The firmware reads it through `identity::SeedSource`, which is not implemented on the chip yet.
- In version 1 of the GATT layout, anyone nearby can connect and read the live fix or request segments (`docs/ble-design.md`, Security). Pairing is the fix for that.

## What pairing has to achieve

1. Stop a stranger from reading the live fix, requesting segments, or writing control commands.
2. Bind a phone to one device, so a second phone cannot quietly take over.
3. Let the backend trust that a segment came from this device. That is the signature's job, and it does not depend on the BLE link.

Point 3 is the important one. The BLE link is encrypted after pairing, but the signature is what protects the data once it has left the device.

## Recommendation

Use two layers.

**Layer 1: BLE bonding with LE Secure Connections.** Pair with the "Just Works" model, using LE Secure Connections and bonding. The link is encrypted, so passive sniffing gets nothing. "Just Works" has no man-in-the-middle protection, so layer 2 is needed.

**Layer 2: enrolment with a device secret.** Each device gets a random 128-bit enrolment secret when it is provisioned. The secret is printed on a label inside the enclosure, or in a QR code. The owner enters it once in the PWA, or scans it. The first phone that presents it is bonded, and the device accepts control writes only from bonded phones that know the secret. A second phone must present the secret again to replace the first.

The secret is stored on the device and on the backend (as a hash), never in the PWA. The enrolment is refused after a set number of failures, using the same counting approach as the API key checks in `ratelimit.rs`.

## Why not the other options

- **Passkey entry (a six-digit PIN).** The device has no display. A fixed PIN printed on the case is no better than the enrolment secret, and it is weaker because it is short.
- **Numeric comparison.** Needs a display on the device for both sides to show the same number. The device has none.
- **Open pairing, with no secret.** This is the version 1 state. Anyone nearby can bond.

## Decisions on the open questions

1. **Label or QR code: yes.** The enrolment secret is printed on a label inside the enclosure, with a QR code of the same secret. The recommendation allows either, and both are used here.
2. **Lost phone: the admin can clear a device's bond, but a replacement phone still needs the enrolment secret.** Nothing takes over a device without the secret. The recommendation did not choose a rule for this, so this is the chosen default.
3. **Raw GPS: no change.** Bonding does not grant raw GPS. It stays opt-in, as `docs/privacy.md` says.

## Work this implies, in order

- On-chip seed generation and storage through `SeedSource` (esp-hal RNG and a protected flash partition). Needs a board.
- Choose the BLE stack. `docs/ble-design.md` recommends `trouble-host`. Needs a board.
- Bonding and enrolment in the firmware, then the PWA enrolment screen.
- Backend: store the enrolment secret's hash with the device, and check it on first bond.
- Test against a real phone on Chromium and Android, since iOS Safari and Firefox have no Web Bluetooth (`todo.md`).
