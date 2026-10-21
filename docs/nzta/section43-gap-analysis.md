# Section 43 gap analysis

Written for: the owner, and anyone preparing an application or assessing whether to apply. This compares the current design with what approval is likely to need.

> **Read this first.** The approval criteria themselves are **not** in this repository, and this analysis did not read them from a primary source. Every criterion below marked **needs NZTA source** must be checked against the Road User Charges Act and NZTA's own distance recorder requirements before anyone relies on it. The "current state" column describes the code as it is, with file references, so it can be checked.

Sources used:

- The project's own statement of the Section 43 position: `DESIGN_SPEC.md` section 7.2 and `README.md`.
- The project's own gap list: `todo.md`, Phase 5 "NZTA Section 43 approval prep".
- The code, as described below.

## Summary

The design is a reference implementation for personal tracking and estimation. It is not built as an approved distance recorder, and it says so in the UI and README. Nothing in it is yet evidence of accuracy, tamper resistance or record integrity to the standard an approval would need. The largest gaps are:

1. **Manual trip entry** can record any distance, so a record can be typed rather than measured.
2. **No tamper evidence.** Records are not signed, and deleting trips leaves no record beyond a log line with a count.
3. **No accuracy evidence.** No field test has been run. The protocol is in `accuracy-test-protocol.md`.
4. **No hardware.** The GPS firmware is not written, and the hardware has not been built.

## Gap table

| Area | Current state | Evidence in the repo | Gap | Status |
|---|---|---|---|---|
| Legal position | Stated as not an approved recorder, in the UI and README | `DESIGN_SPEC.md` 7.2; PWA Section 43 gate (`pwa/components/Section43Gate.tsx`) | None for the disclaimer. Approval itself is not held. | Met for disclaimer |
| Distance source | GPS fixes are filtered by fix quality, HDOP, implied speed and stationary drift | `crates/tpt-ruc-core/src/trip.rs`, `Config::default`: max HDOP 5.0, max speed 60 m/s, stationary below 0.5 m/s, minimum move 5 m, gap limit 30 s | The filter values are design choices, not tested against a reference | Partial |
| Distance source, manual | The PWA lets the user type a trip's distance | `pwa/components/Trips.tsx` | A typed distance is not a measured one. Any recorder that accepts typed distances will not be approved. | **Gap** |
| Accuracy | No field or bench accuracy test run | None | Needs a test protocol, data and results | **Gap**, protocol in `accuracy-test-protocol.md` |
| Device | No device has been built or run. The GNSS UART reader is written in `firmware/esp` but has never run on hardware. | `hardware/README.md`, `firmware/esp`, `todo.md` Phase 3 | Hardware test, full device, power, enclosure, mounting and tamper protection are not done | **Gap** |
| Data path | Firmware log is CRC-checked and append-only, and recovers after power loss. Host-tested only. | `crates/tpt-ruc-firmware/src/log.rs` | Flash driver not written. Not tested on hardware. | Partial |
| Integrity of records | Backend ingest is idempotent and rejects conflicting resends. Charges are recalculated by the server. | `crates/tpt-ruc-backend/src/trips.rs` | Records are not signed. No device identity, so a record cannot be traced to a device. Per-device keys are planned, not built. | **Gap** |
| Tamper evidence | Trips can be deleted by range or by user. The only trace is a log line with a count and dates. | `crates/tpt-ruc-backend/src/data.rs`, `users.rs` | No audit trail, no tamper seal, no protection of stored records. Owner can delete records. | **Gap** |
| Security of the data path | API keys and tokens are stored as hashes. Transport is plain HTTP unless a reverse proxy is added. Rate limits apply. | `crates/tpt-ruc-backend/src/auth.rs`, `ratelimit.rs`, `docs/guides/self-hosting.md` | Transport security is the operator's job. No device authentication yet. | Partial |
| Rates | Rates are a dated, versioned table, with a version recorded on every charge | `crates/tpt-ruc-core/src/ruc_rates.rs`; `docs/nzta/rate-update-procedure.md` | Earlier tables are not in the code. Some values are assumptions, noted in `rates-research.md`. | Partial |
| Reports | CSV reports per trip and per month, for audit | `crates/tpt-ruc-backend/src/reports.rs` | Reports are generated from records that can be deleted. PDF reports not built. | Partial |
| Odometer and licence expiry | Odometer is optional and manual. Licence expiry by distance is not modelled. | `pwa/components/Vehicles.tsx`; `todo.md` Phase 1 | No odometer-based expiry or refund modelling | **Gap** |
| Off-road deductions | Not implemented. Flagged in the todo as needing Section 43 approval. | `todo.md` Phase 1 | Not built, so nothing to approve yet | Not started |
| Documentation of approval | This gap analysis, the accuracy protocol and the decision memo | `docs/nzta/` | Needs the NZTA source for every criterion | Started |

## Criteria to confirm (needs NZTA source)

The following must be read from NZTA's own text before this analysis can be completed:

- The exact accuracy requirement, and how it is measured.
- The required tamper protections, and the evidence NZTA expects.
- Whether a recorder must be tested by an approved laboratory, and who may run the test.
- The data retention, export and audit requirements for records.
- Whether a distance recorder may accept any manual entry at all.
- The application process, fees and what the application must contain.
- Whether software-only recorders can be approved, or whether the hardware must be certified.

## What this means

Until the criteria are read from NZTA's text, the team cannot say which gaps are blocking and which are not. The first action is to obtain that text, not to build more features.
