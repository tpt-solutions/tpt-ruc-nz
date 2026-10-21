# tpt-ruc-ffi-android

Kotlin bindings for [`tpt-ruc-core`](../tpt-ruc-core/README.md), built on the [`jni`](https://crates.io/crates/jni) crate. It exposes the RUC liability and billable-distance functions from [`tpt-ruc-ffi-common`](../tpt-ruc-ffi-common/README.md) to a Kotlin app.

> **Status: untested.** `build-android.sh` has not been run. It needs the Android NDK and `cargo-ndk`. The Rust code is unit-tested on the host.

> **NZTA Section 43 disclaimer.** An app built on these bindings is a reference implementation and is **not an NZTA-approved distance recorder**. See the [project README](../../README.md).

This crate is internal (`publish = false`). It forbids `unsafe` code everywhere except the JNI entry points, which must be exported with `#[unsafe(no_mangle)]` so the JVM can find them.

## Contents

- [Kotlin API](#kotlin-api)
- [Building](#building)
- [Using it from Kotlin](#using-it-from-kotlin)
- [Testing](#testing)
- [Licence](#licence)

## Kotlin API

The Kotlin side declares these functions as `external` in the object `com.tpt.ruc.RucNative`:

| Kotlin declaration | Returns |
|---|---|
| `external fun liabilityInclGstNzd(rateType: String, grossKg: Int, electric: Boolean, year: Int, month: Int, day: Int, metres: Double): Double` | The GST-inclusive charge for one trip, in NZD. |
| `external fun billableKm(metres: Double): Double` | Distance in kilometres, rounded the way RUC bills it. |

Money is returned as NZD in a `Double`, exact to the micro-dollar. Round it to cents for display. A bad input throws a `RuntimeException` that gives the reason.

## Building

`build-android.sh` builds `libtpt_ruc_ffi_android.so` for the common Android ABIs and copies the results into the sample app's `jniLibs` folder:

```sh
crates/tpt-ruc-ffi-android/build-android.sh
```

It needs the Android NDK and `cargo-ndk`. Check the licence of `cargo-ndk` with the owner before installing it, as the project's licence policy applies to build tools too.

## Using it from Kotlin

A working example is in [`samples/android/`](../../samples/android/). The sample loads the native library and calls the functions:

```kotlin
val charge = RucNative.liabilityInclGstNzd(
    rateType = "1", grossKg = 3000, electric = false,
    year = 2026, month = 10, day = 1, metres = 12345.0
)
println("$%.2f incl. GST".format(charge))
```

## Testing

```sh
cargo test -p tpt-ruc-ffi-android
```

The crate has one host test. The Kotlin side is not tested automatically yet.

## Licence

Dual licensed under [MIT](../../LICENSE-MIT) or [Apache-2.0](../../LICENSE-APACHE), at your option.
