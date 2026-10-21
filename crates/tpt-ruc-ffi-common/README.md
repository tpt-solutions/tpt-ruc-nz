# tpt-ruc-ffi-common

The small, language-neutral API that the iOS and Android bindings share. It wraps [`tpt-ruc-core`](../tpt-ruc-core/README.md) and takes plain values (strings, integers and floats) so that Swift and Kotlin need no knowledge of the core's types.

Errors come back as text. Each binding turns that text into its own error type.

> **NZTA Section 43 disclaimer.** The apps built on these bindings are reference implementations and are **not NZTA-approved distance recorders**. See the [project README](../../README.md).

This crate is internal (`publish = false`). Use the platform crates, not this one, from an app.

## Contents

- [API](#api)
- [Money and dates](#money-and-dates)
- [Who uses it](#who-uses-it)
- [Testing](#testing)
- [Licence](#licence)

## API

| Function | Returns |
|---|---|
| `liability_incl_gst_micros(rate_type, gross_kg, electric, year, month, day, metres)` | The GST-inclusive charge for one trip, in micro-dollars. |
| `billable_km(metres)` | Distance in whole kilometres, rounded the way RUC bills it. Rejects negative and non-finite input. |

Inputs:

- `rate_type` is an NZTA vehicle type code, for example `"1"` or `"H61"`. An unknown code is an error.
- `gross_kg` is the RUC weight in kilograms.
- `electric` is true for a battery electric vehicle.
- `year`, `month` and `day` are the trip date. The rate table is chosen by this date. An impossible date is an error.
- `metres` is the distance driven.

```rust
let micros = tpt_ruc_ffi_common::liability_incl_gst_micros(
    "1", 3_000, false, 2026, 10, 1, 12_345.0,
)?;
// micros == 1_000_000 is one dollar
```

## Money and dates

Money stays in integer micro-dollars across the core. The bindings convert it at the edge:

- Swift and Kotlin receive NZD as a `Double`. It is exact to the micro-dollar, so round it to cents only for display.
- Dates are passed as three integers rather than a string, so a binding never parses a date.

## Who uses it

- [`tpt-ruc-ffi-ios`](../tpt-ruc-ffi-ios/README.md) exposes these functions to Swift through swift-bridge.
- [`tpt-ruc-ffi-android`](../tpt-ruc-ffi-android/README.md) exposes them to Kotlin through JNI.

## Testing

```sh
cargo test -p tpt-ruc-ffi-common
```

The iOS binding's tests check that its NZD value matches the micro-dollar value from this crate, so a rounding change shows up in both.

## Licence

Dual licensed under [MIT](../../LICENSE-MIT) or [Apache-2.0](../../LICENSE-APACHE), at your option.
