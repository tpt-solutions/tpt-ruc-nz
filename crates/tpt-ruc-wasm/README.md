# tpt-ruc-wasm

WebAssembly bindings for [`tpt-ruc-core`](../tpt-ruc-core/README.md). The PWA loads this module to run the same NZ Road User Charges maths on the phone or laptop as the backend and the device do.

> **NZTA Section 43 disclaimer.** This module is an open-source reference implementation and is **not an NZTA-approved distance recorder**. Rates are not authoritative until checked against [nzta.govt.nz](https://www.nzta.govt.nz). See the [project README](../../README.md).

## Contents

- [What it exports](#what-it-exports)
- [Building](#building)
- [Using it from JavaScript](#using-it-from-javascript)
- [Testing](#testing)
- [Notes](#notes)
- [Licence](#licence)

## What it exports

All functions use plain values or JSON strings. Money is returned in micro-dollars (1,000,000 is $1.00), as in the core crate.

| JavaScript name | Rust | Returns |
|---|---|---|
| `haversineM(lat1, lon1, lat2, lon2)` | `haversine_m` | Distance in metres. Throws on an out-of-range coordinate. |
| `billableKm(metres)` | `billable_km_js` | Billable kilometres, rounded the way RUC bills them. |
| `currentRateEffective()` | `current_rate_effective` | The effective date of the newest NZTA rate table, as `YYYY-MM-DD`. |
| `heavyLicenceExpiresOn(grossKg, year, month, day)` | `heavy_licence_expires_on_js` | The expiry date of a heavy licence bought under the rate change on that date, or `undefined` at or under the heavy limit. |
| `liability(rateType, grossKg, electric, year, month, day, metres)` | `liability` | `{ rateVersion, billedMetres, inclGstMicros, exclGstMicros, gstMicros }`. |
| `adminFeeInclGstMicros(agent)` | `admin_fee_incl_gst_micros` | The administration fee for one licence, including GST. `agent` selects the agent purchase fee. |
| `parseNmea(line)` | `parse_nmea` | One NMEA sentence as an object keyed by its type, such as `{ Gga: {...} }`. Returns `"Unsupported"` for a valid sentence this parser does not decode. Throws on a bad checksum or field. |
| `new TripTracker()` | `TripTracker::new` | A trip tracker with the default thresholds. |
| `tracker.push(fixJson)` | `TripTracker::push` | `{ status: "accepted", metres, closed }` or `{ status: "rejected", reason }`. The fix is the JSON form of the core `Fix` type. |
| `tracker.totalM()` | `TripTracker::total_m` | Total accepted distance so far, in metres. |

## Building

Install [wasm-pack](https://rustwasm.github.io/wasm-pack/) once:

```sh
cargo install wasm-pack --locked
```

Then, from the repository root:

```sh
scripts/build-wasm.sh
```

The script runs `wasm-pack build` with `--target web` and writes the package to `pwa/wasm-pkg`, where the PWA imports it. The release profile is tuned for size (`opt-level = "s"`, LTO, one codegen unit). The `wasm-opt` flags in `Cargo.toml` allow the bulk-memory and non-trapping float-to-int instructions that current browsers support.

## Using it from JavaScript

With `--target web` the module needs one `init` call before its exports are used:

```js
import init, { liability, parseNmea, TripTracker } from "./wasm-pkg/tpt_ruc_wasm.js";

await init();

const charge = liability("1", 3000, false, 2026, 10, 1, 12345);
console.log(charge.inclGstMicros / 1_000_000, "NZD incl. GST");

const tracker = new TripTracker();
const result = tracker.push(JSON.stringify(fix));
if (result.status === "accepted") {
  console.log("trip so far:", tracker.totalM(), "m");
}
```

The PWA's own wrapper is in [`pwa/`](../../pwa/). Use it as the reference for loading and error handling.

## Testing

The tests run inside Node.js, not in a browser:

```sh
cd crates/tpt-ruc-wasm
wasm-pack test --node
```

The tests live in `tests/web.rs` and exercise the exports from inside Node.

## Notes

- Errors thrown from Rust arrive as JavaScript `Error` objects with the core crate's message.
- `TripTracker.push` takes a JSON string rather than an object. This keeps the `Fix` type as the single source of truth for the field names.
- The module is built from the same core crate as the device, so the NMEA parser and trip filter behave the same on both.

## Licence

Dual licensed under [MIT](../../LICENSE-MIT) or [Apache-2.0](../../LICENSE-APACHE), at your option.
