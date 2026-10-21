# tpt-ruc-core

Pure Rust maths for New Zealand Road User Charges (RUC): NMEA 0183 parsing, validated coordinates, great-circle distance, trip tracking, and RUC liability from the NZTA rate tables.

`tpt-ruc-core` is the shared logic behind the tpt-ruc-nz project. The PWA (through `tpt-ruc-wasm`), the self-hosted backend, the ESP32 firmware and the iOS and Android bindings all use it, so every platform gives the same answer for the same input. It has no I/O and no hardware code.

> **NZTA Section 43 disclaimer.** This crate is an open-source reference implementation. It is **not an NZTA-approved distance recorder**. Claiming RUC deductions, or using a digital display in place of a licence label, requires Section 43 approval from NZTA. Rates are not authoritative until they have been checked against [nzta.govt.nz](https://www.nzta.govt.nz). See the [project README](../../README.md) and [DESIGN_SPEC.md](../../DESIGN_SPEC.md).

## Contents

- [Features](#features)
- [Installation](#installation)
- [Quick start](#quick-start)
- [Modules](#modules)
- [Cargo features and `no_std`](#cargo-features-and-no_std)
- [Testing](#testing)
- [Stability](#stability)
- [Licence](#licence)

## Features

| Area | Module | What it does |
|---|---|---|
| Coordinates | `coord` | `Latitude`, `Longitude` and `Coordinate` types that reject out-of-range values at construction. |
| NMEA parsing | `nmea` | `parse_sentence` decodes GGA, RMC, VTG and GSA with checksum checks. It accepts multi-constellation talker IDs such as GN. |
| Distance | `distance` | `haversine_m` gives the great-circle distance in metres, using `libm` so it works without `std`. |
| Trip tracking | `trip` | `Tracker` filters fixes by fix quality, HDOP, implied speed jumps and stationary drift. It splits trips into `Segment`s across signal gaps and accumulates the distance in metres. `billable_km` gives the rounded billable figure. |
| RUC liability | `ruc` | `liability` returns the charge for a vehicle type code, weight band, electric flag and trip date, in integer micro-dollars. GST-inclusive amounts split into excl-GST and GST with `split_gst`. Rates are chosen by effective date. |
| Rate tables | `ruc_rates` | The NZTA distance-licence schedule, effective from 2026-08-06 and retrieved 2026-10-21. It covers powered, unpowered, crane, vintage, motor caravan, overweight and H types, plus administration fees for online and agent purchases. |
| Licences | `licence` | Licence status and remaining kilometres, overlap-licence distance, pro-rata refunds, and billable distance after an off-road exemption. An exemption is refused unless Section 43 approval is flagged. |

Electric vehicles owe nothing before `EV_LIABLE_FROM` (1 April 2024). Heavy licences bought before a rate change expire one month after it (`heavy_licence_expires_on`), and `needs_overlap_licence` reports when two licences overlap.

## Installation

```toml
[dependencies]
tpt-ruc-core = "0.1"
```

For a microcontroller or WebAssembly build, turn off the default features:

```toml
[dependencies]
tpt-ruc-core = { version = "0.1", default-features = false }
```

## Quick start

### Distance between two points

```rust
use tpt_ruc_core::{haversine_m, Coordinate};

let wellington = Coordinate::new(-41.2865, 174.7762).unwrap();
let auckland = Coordinate::new(-36.8485, 174.7633).unwrap();

let metres = haversine_m(wellington, auckland);
```

### RUC liability for one trip

```rust
use tpt_ruc_core::ruc_rates::NZTA_SCHEDULE;
use tpt_ruc_core::{liability, RateDate, Vehicle};

let vehicle = Vehicle {
    rate_type: "1",
    gross_kg: 3_000,
    electric: false,
};
let date = RateDate { year: 2026, month: 10, day: 1 };

let charge = liability(NZTA_SCHEDULE, &vehicle, date, 12_345.0)?;
// charge.incl_gst_micros: 1_000_000 is one dollar
```

### Tracking a trip from GNSS fixes

`Tracker` takes `Fix` values. Each fix carries a position, a timestamp, the NMEA fix quality and the HDOP. Build the `Tracker` with a `Config` (its defaults are sensible for a car), then call `push` for each fix. `push` returns `Accepted` or `Rejection`, and `total_m` gives the running distance.

```rust
use tpt_ruc_core::{Config, Tracker};

let mut tracker = Tracker::new(Config::default());
// for each fix from the receiver:
//     let _ = tracker.push(fix);
let total_metres = tracker.total_m();
```

## Modules

```
coord       Latitude, Longitude, Coordinate, CoordinateError
distance    haversine_m, EARTH_RADIUS_M
nmea        parse_sentence, Sentence, FixQuality, ParseError
trip        Tracker, Config, Fix, Segment, Accepted, Rejection, billable_km
ruc         liability, Vehicle, RateDate, RateTable, Rule, split_gst, heavy_licence_expires_on
ruc_rates   NZTA_SCHEDULE and the administration fee tables
licence     DistanceLicence, LicenceStatus, status, overlap_licence_km, billable_after_exemption
```

The most common items are re-exported at the crate root.

## Cargo features and `no_std`

| Feature | Default | Effect |
|---|---|---|
| `std` | yes | Standard library support for host use. Without it the crate is `#![no_std]`. |
| `serde` | yes | `Serialize` and `Deserialize` for `Coordinate`, `Fix`, `Config`, `Segment` and related types. Coordinates are validated on decode. |

The crate does not use `alloc` and forbids `unsafe` code. CI builds it for `thumbv7em-none-eabihf`, `riscv32imc-unknown-none-elf` and `wasm32-unknown-unknown`:

```sh
cargo check -p tpt-ruc-core --no-default-features --target thumbv7em-none-eabihf
```

## Testing

```sh
cargo test -p tpt-ruc-core
```

The tests cover:

- **Unit tests** in each module, for coordinate limits, distance, parsing and RUC rules.
- **NMEA fixtures** (`tests/nmea_fixtures.rs`). Every file in `tests/fixtures/` must parse line by line. The only fixture is labelled synthetic. See [tests/fixtures/README.md](tests/fixtures/README.md) for the captures still needed from real hardware.
- **Property tests** (`tests/properties.rs`). The NMEA parser must not panic on arbitrary input, and the trip total must never decrease.
- **Wire-format tests** (`tests/wire.rs`), checked against the `Fix` version 1 golden file in `tests/golden/fix_v1.json`.

## Stability

The crate is pre-1.0 and its API may change. Under semantic versioning for `0.x`, a breaking change bumps the minor version. The `Fix` wire format is versioned, and any change to it needs a new version number. Rate tables change through a dated data release and not through a code change, so check the `ruc_rates` effective date before relying on a result.

See [CHANGELOG.md](CHANGELOG.md) for what has changed.

## Licence

Dual licensed under [MIT](../../LICENSE-MIT) or [Apache-2.0](../../LICENSE-APACHE), at your option.
