# Native Bluetooth on iOS and Android: plan

Status: **BLOCKED.** This cannot be built or tested without devices and the device protocol bindings.

## Why it is blocked

- **Devices.** Testing needs an iPhone and an Android phone, and a GPS device that speaks the protocol. None of these are available in the build environment.
- **Protocol bindings.** The BLE protocol (`docs/ble-design.md`) is designed but not implemented. The firmware side is not written either. A native app can only be tested once the device can send data.
- **Platform code.** iOS Bluetooth uses CoreBluetooth in Swift. Android uses the platform Bluetooth LE API in Kotlin. Both are written against real hardware, so their code cannot be checked without it.

## What it is for

Web Bluetooth is not available in iOS Safari or Firefox, as `todo.md` records. The native apps must cover iPhones, and the Android app covers devices where the browser route is not enough.

## Plan

1. **Finish the device protocol first.** The firmware must send the live fix and the segment transfer described in `docs/ble-design.md`. Test it with a generic BLE scanner app before writing any native code.
2. **Write a Swift sample.** Use CoreBluetooth to scan, connect, subscribe to the live fix characteristic, and decode the payload with the same format the PWA uses (`pwa/lib/ble.ts`).
3. **Write a Kotlin sample.** Do the same with the Android Bluetooth LE API.
4. **Feed the bytes into the core.** The decoded fixes go into the core through the bindings. Keep the decoding in one place, and check that both apps produce the same output for the same captured bytes.
5. **Test with captured data first.** Replay a recorded BLE session through each app without a device. Only then test with a live device.
6. **Handle connection states.** Cover reconnects, permission prompts, the Bluetooth switch being off, and the device going out of range during a trip.

## Open decisions

- Whether the native apps are built with Swift and Kotlin only, or with a cross-platform toolkit. The licence policy must be checked for any toolkit.
- Who owns the iOS and Android accounts needed to publish the apps. Publishing is out of scope for this work.

## Done when

- A captured BLE session decodes to the same fixes in the Swift app, the Kotlin app and the PWA.
- A live device sends fixes to each app for a full drive, and the recorded trip matches a reference measurement.
