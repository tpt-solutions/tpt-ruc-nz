# RUC rates: source and status

Written for: engineers and estimators maintaining the rate table in `crates/tpt-ruc-core/src/ruc_rates.rs`.

## Source

- NZTA, "RUC rates and transaction fees", retrieved 2026-10-21: <https://www.nzta.govt.nz/vehicles/road-user-charges/ruc-rates-and-transaction-fees>
- The page itself was not reachable from the build environment. The text was supplied by the maintainer and transcribed from that copy.
- Secondary cross-check: Calculate NZ, <https://www.calculate.co.nz/reference/nz-road-user-charges.php>, which agrees on the light, plug-in hybrid and heavy figures checked.

## What the page establishes

- Rates are **dollars per 1,000 km, GST inclusive**. The engine calculates the GST-inclusive charge and splits GST out of it.
- Rates are keyed by **vehicle type code** (for example `1`, `12`, `H61`), then by weight band. Trailers are licensed separately under their own type codes, so there is no trailer surcharge.
- Types 408, 430 and 433 are marked "From 6 August 2026".
- Overweight licences are keyed by permit weight.

## What the page does not establish

- **Effective date of the rest of the table.** Only the three types above carry a date. `ruc_rates.rs` treats the whole table as effective from 2026-08-06. Trips dated earlier return `NoRateTable`, not a guessed rate, until the previous table is transcribed.
- **EV liability start date:** 1 April 2024, per the maintainer. Modelled as `EV_LIABLE_FROM` in `ruc.rs`. Electric vehicles owe nothing before that date. Electric vehicles use type 1 at the same rate as diesel.
- **Additional licences** for overweight permits are transcribed in `ruc_rates.rs` (`ADDITIONAL_LICENCES`, `additional_liability`). Two assumptions are not on the page: a part of 1,000 kg counts as a full step, and the weight matches the smallest row at or above it.
- **Transaction fees.** The page gives $12.44 online (NZTA app) and $13.71 at agent counters, both incl GST. Modelled as `admin_fee_incl_gst_micros` in `ruc_rates.rs`.
- **Type 699 and 799 cranes.** The page lists them with no rate, so they correctly return `NoMatchingRule`.

## Open items

- [x] Earlier tables are out of scope (maintainer decision, 2026). Trips before 2026-08-06 return `NoRateTable`.
- [x] EV start date set to 1 April 2024 (maintainer's statement). Check it against NZTA when the page is next reviewed.
- [x] Additional-licence tables transcribed. Confirm the two assumptions above with NZTA.
- [x] Transaction fees added (`PurchaseMethod`, `admin_fee_incl_gst_micros`).
- [ ] Re-check the table on a schedule. NZTA says heavy vehicle licences expire one month after any rate increase, which the product will need to model.
