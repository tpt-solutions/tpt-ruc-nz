# Mobile bindings

Written for: developers building the iOS or Android app around the core.

Status: the Rust crates compile and their tests pass on the host. **The iOS and Android builds are untested.** The build scripts have not been run.

## The API

Both platforms expose the same two functions. They take plain values and return plain values or a clear error.

| Function | Input | Output |
|---|---|---|
| `liability_incl_gst_nzd` | vehicle type code, weight in kg, electric flag, trip date, distance in metres | GST-inclusive charge in NZD |
| `billable_km` | distance in metres | distance in kilometres, rounded the way RUC bills it |

Money comes back as NZD in a floating-point number, exact to the micro-dollar. Round it to cents for display.

Errors:

- **Swift:** the function throws, with the reason as its message.
- **Kotlin:** the function throws a `RuntimeException`, with the reason as its message.

Bad inputs include an unknown vehicle type code, a date that does not exist, a date with no rate table, and a negative distance.

## Crates

| Crate | Purpose | Status |
|---|---|---|
| `crates/tpt-ruc-ffi-common` | The shared wrapper around the core. No platform code. | Tested on host |
| `crates/tpt-ruc-ffi-ios` | Swift bindings through swift-bridge | Compiles on host. Glue generation is not written. |
| `crates/tpt-ruc-ffi-android` | Kotlin bindings through the jni crate | Compiles on host. Exported symbols checked in the built library. |

## Dependencies and licences

- `swift-bridge` 0.1.59: declares `Apache-2.0/MIT`. Passes the licence policy through the MIT option.
- `jni` 0.22.4: `MIT OR Apache-2.0`.
- Both pass `cargo deny` with the project's allow list.

The Android crate allows `unsafe_code`, because the JNI entry points must use `#[unsafe(no_mangle)]`. The exception is limited to that crate, and the other lints still apply.

## Building

- iOS: `crates/tpt-ruc-ffi-ios/build-ios.sh`. Untested, and it needs a Mac with Xcode. The Swift glue generation step is still to be written.
- Android: `crates/tpt-ruc-ffi-android/build-android.sh`. Untested, and it needs the Android NDK and `cargo-ndk`. Check the `cargo-ndk` licence before installing it, because it is a build tool, not a dependency.

## Not included

- NMEA parsing. The core has it, but the first version of the mobile API does not expose it.
- Trip storage. The apps must store their own data.
- Bluetooth. See [`mobile-ble-plan.md`](mobile-ble-plan.md).
