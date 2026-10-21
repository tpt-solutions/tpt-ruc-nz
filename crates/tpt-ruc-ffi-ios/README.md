# tpt-ruc-ffi-ios

Swift bindings for [`tpt-ruc-core`](../tpt-ruc-core/README.md), built with [swift-bridge](https://github.com/chinedufn/swift-bridge). It exposes the RUC liability and billable-distance functions from [`tpt-ruc-ffi-common`](../tpt-ruc-ffi-common/README.md) to a Swift app.

> **Status: untested.** `build-ios.sh` has not been run. It needs a Mac with Xcode, and the steps are written from the swift-bridge and Apple documentation. The Rust code is unit-tested on the host.

> **NZTA Section 43 disclaimer.** An app built on these bindings is a reference implementation and is **not an NZTA-approved distance recorder**. See the [project README](../../README.md).

This crate is internal (`publish = false`).

## Contents

- [Swift API](#swift-api)
- [Building](#building)
- [Using it from Swift](#using-it-from-swift)
- [Testing](#testing)
- [Licence](#licence)

## Swift API

| Swift function | Returns |
|---|---|
| `liability_incl_gst_nzd(rateType, grossKg, electric, year, month, day, metres)` | The GST-inclusive charge for one trip, in NZD. Throws on bad input. |
| `billable_km(metres)` | Distance in kilometres, rounded the way RUC bills it. Throws on bad input. |

The arguments are positional. swift-bridge turns a Rust `Result` with a `String` error into a throwing Swift function. Money comes back as a `Double` in NZD, exact to the micro-dollar. Round it to cents for display.

## Building

`build-ios.sh` builds the device and simulator libraries and packages them as an XCFramework that an Xcode project can link:

```sh
crates/tpt-ruc-ffi-ios/build-ios.sh
```

The sample's README in [`samples/ios/`](../../samples/ios/) describes how the generated swift-bridge glue is linked into an Xcode project.

## Using it from Swift

[`samples/ios/RucExample.swift`](../../samples/ios/RucExample.swift) shows the calls and the error handling:

```swift
let charge = try liability_incl_gst_nzd("1", 3_000, false, 2026, 10, 1, 12_345)
print(String(format: "Charge including GST: $%.2f", charge))
```

## Testing

```sh
cargo test -p tpt-ruc-ffi-ios
```

The two host tests include a check that the dollar values match the micro-dollar values from `tpt-ruc-ffi-common`. The Swift side is not tested automatically yet.

## Licence

Dual licensed under [MIT](../../LICENSE-MIT) or [Apache-2.0](../../LICENSE-APACHE), at your option.
