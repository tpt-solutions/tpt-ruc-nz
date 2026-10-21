# tpt-ruc-nz – Project Checklist

Owner: TPT Solutions · Licence: MIT OR Apache-2.0 (dual) · Source of truth: `spec.txt`

## Decisions locked in

- [x] Dual licensed **MIT OR Apache-2.0**; copyright holder **TPT Solutions**
- [x] **No Apache-2.0-only dependencies**. Every dep must be MIT, BSD, ISC, Zlib, Unlicense/CC0, or dual MIT/Apache. MPL-2.0 is also excluded (so no uniffi, no cbindgen)
- [x] Protobuf (prost, Apache-only) is out. BLE/sync wire format is **MessagePack** (rmp-serde)
- [x] Firmware targets: **ESP32-S3 (Xtensa)** and **a RISC-V ESP (C3/C6)**, one crate with chip feature flags. `thumbv7em-none-eabihf` is used only as a `no_std` portability check for core, not as a firmware target (the spec's target for ESP32-S3 was wrong)
- [x] Mobile: **swift-bridge (iOS) + jni crate (Android)**, replacing UniFFI
- [x] iOS Safari / Firefox lack Web Bluetooth: accepted limitation, documented. iOS BLE comes via the native app
- [x] RUC scope: **light + heavy vehicles, full NZTA types**. Rates are a versioned data table, not hardcoded
- [x] Backend: SQLite **and** PostgreSQL, fleet management, CSV/PDF reports, eRUC export, all in Phase 4
- [x] Auth: single-owner API-key mode (default for self-host) **plus** org/user/role mode for fleets. Per-device ed25519 keys
- [x] Sync security: TLS + ed25519 signed payloads. Summaries only by default; raw GPS sync is opt-in
- [x] eRUC export: generic documented JSON/CSV format plus an adapter trait now. Real provider adapters wait for provider API docs (**you supply**)
- [x] PWA: Next.js **static export** + service worker (no Node server). Backend can optionally serve it
- [x] GitHub org **TPT Solutions**, GitHub Actions CI, images on ghcr.io
- [x] Extras in scope: hardware track (enclosure, BOM, power, LiPo shutdown) and NZTA Section 43 approval prep

## Open items to resolve (need your input)

- [ ] Confirm current NZTA RUC rates and vehicle types. Rates now transcribed from the NZTA page (retrieved 2026-10-21). EV liability start date is 1 April 2024 (modelled). Additional overweight licences are transcribed (two assumptions to confirm with NZTA). Earlier rate tables are out of scope. See `docs/nzta/rates-research.md`
- [ ] Provide eRUC provider API docs and names when available (Phase 5)
- [ ] Confirm exact GitHub org slug and repo name (replaces `your-username` in the compose example)
- [ ] Decide the production domain for the PWA/API

---

## Phase 0 – Foundations & Governance (before Week 1)

- [ ] Create GitHub org/repo `TPT Solutions/tpt-ruc-nz`
- [x] Add `LICENSE-MIT` and `LICENSE-APACHE` (copyright TPT Solutions)
- [x] Add SPDX `MIT OR Apache-2.0` to every `Cargo.toml` and `package.json` (`Cargo.toml` done; `package.json` arrives in Phase 2)
- [x] Add a licence header/SPDX policy for source files
- [x] Move `spec.txt` to `DESIGN_SPEC.md` and update it for decisions above (firmware target, MessagePack, FFI choice)
- [x] Create the monorepo skeleton: `crates/`, `pwa/`, `hardware/`, `docs/`
- [x] Write `README.md` with a **prominent NZTA Section 43 disclaimer**
- [x] Add `.gitignore`, `.editorconfig`, `rust-toolchain.toml`
- [x] Add `cargo-deny` config (allow MIT/BSD/ISC/Zlib/Unicode/CC0; allow Apache-2.0 only on dual-licensed crates; deny MPL/GPL/AGPL; flag Apache-only crates)
- [x] Add a JS licence check for `pwa/` (`pwa/scripts/check-licences.mjs`, with named exceptions in docs/LICENSING.md for review)
- [x] Add a `THIRD_PARTY_LICENSES` generation step (`about.hbs` template, `THIRD_PARTY_LICENSES.html` generated, and a CI job runs `cargo about generate` so an unaccepted licence fails the build)
- [x] Add `CONTRIBUTING.md`, `SECURITY.md`, and a DCO or CLA decision
- [x] CI baseline (GitHub Actions): fmt, clippy, test, `cargo deny`, caching
- [x] Dependabot or Renovate for Cargo and npm
- [ ] Set up toolchains: Rust stable, wasm-pack, `espup` (Xtensa), RISC-V target, thumbv7em target (Rust stable plus wasm32, thumbv7em and riscv32imc targets installed; wasm-pack and espup still to install)
- [x] Choose and record the versioning/release strategy (release-please or cargo-dist)

## Phase 1 – The Core & Math (Weeks 1-2)

### Workspace
- [x] Root `Cargo.toml` workspace with shared lint/dep config
- [x] Create the crate `tpt-ruc-core` (`no_std` + `alloc`, `std` feature) (no_std; `alloc` not needed yet)

### NMEA parsing
- [x] Choose the parser: a permissively licensed crate (audit) or write our own `no_std` one
- [x] Parse GGA (fix quality, satellites, HDOP, altitude)
- [x] Parse RMC (time, date, position, speed, course, validity)
- [x] Parse VTG (speed/course) and GSA (fix type/DOP)
- [x] Checksum validation and malformed/partial sentence handling
- [x] Support multi-constellation talkers (GP, GN, GL, GA, BD)
- [x] Coordinate structs with a validated lat/lon type
- [x] Accept JSON GPS input as well as NMEA (hardware agnostic): `Fix` deserialises from JSON, with missing HDOP/speed read as `None`

### Distance
- [x] Haversine implementation using `libm` (no_std floats)
- [x] Fix-quality, HDOP and speed filters (reject jumps and drift when stationary)
- [x] Trip segment model: start, stop, accumulated distance, gaps
- [x] Decide metres vs km precision and rounding rules (RUC bills in km): accumulate in metres, round once on the total to whole metres via `billable_km`
- [ ] Optional smoothing/dedupe of points

### RUC engine
- [x] Vehicle model: NZTA RUC type code plus RUC weight. The type code encodes axles, configuration and powertrain, and the caller chooses it. Petrol exemption is the caller's decision (no RUC charged)
- [x] Rates as a **versioned, data-driven table** with effective dates. The NZTA table from the 2026 page is in `ruc_rates.rs`, effective from 2026-08-06 (see the open item below)
- [x] Light vehicle liability (<=3.5t): type 1 and 12 bands, from the NZTA page
- [x] Heavy vehicle liability: weight bands per type, from the NZTA page. Trailers are separate licences, so there is no trailer surcharge. Additional overweight licences are in `ruc_rates.rs`
- [x] GST handling: NZTA rates are GST-inclusive, so the engine computes the inclusive charge and splits GST out, rounding half up
- [ ] Licence purchase, odometer-based licence expiry and refund modelling (done for the rate-change rules: heavy licences expire one month after a rate change, and `needs_overlap_licence` flags purchases in that month. Odometer-based expiry, the overlap licence's distance, and refund credits are still to do)
- [ ] Off-road/exempt distance deductions (flagged "requires Section 43 approval")
- [x] Historical-rate lookups for backdated trips (`rate_for` picks the table in force on the trip date)
- [ ] Verify all rates and rules against NZTA and cite them in docs (powered, unpowered, crane, vintage, motor caravan, overweight and H-type distance rates transcribed from the 2026 page; source and open items in `docs/nzta/rates-research.md`)

### Serialisation
- [ ] `serde` derives for all models (`no_std`-friendly) (done for `Coordinate`, `Latitude`, `Longitude`, `FixQuality`, `Fix`, `Config`, `Segment` behind the `serde` feature; NMEA sentence structs and RUC models still to do)
- [x] JSON via `serde_json` (std feature): `Fix` and `Config` round-trip in tests; the library itself does not link `serde_json`
- [ ] MessagePack via `rmp-serde` (verify `no_std` support, else alternative) (round-trips verified on host; `rmp-serde` is std-only, so firmware needs an alternative)
- [ ] Define and version the BLE payload schema: timestamp, lat, lon, speed, fix quality (JSON form of `Fix` is pinned as v1 in `tests/golden/fix_v1.json`; the binary BLE layout is not yet defined)
- [x] Golden-file tests for wire formats (`Fix` v1 JSON; MessagePack round-trip)

### Quality
- [x] Unit tests for NMEA, Haversine, filters and RUC (known-distance fixtures; RUC mechanics tests use fixture rates, and `ruc_rates` tests check the NZTA values and weight-band invariants)
- [ ] Property tests (proptest) and fuzzing (cargo-fuzz) for the parser (proptest done: no-panic parser tests and tracker invariants; cargo-fuzz not yet set up)
- [ ] Real-world NMEA log fixtures
- [x] CI: `cargo check` for core on `wasm32-unknown-unknown` and `thumbv7em-none-eabihf`
- [ ] CI: `no_std` build check and coverage report (no_std check done in CI; coverage still to add)
- [x] Rustdoc on the public API and a CHANGELOG entry
- [ ] Phase 1 exit: tag `v0.1.0`, all CI green

## Phase 2 – The PWA & WASM (Weeks 3-5)

### WASM
- [x] Create the `tpt-ruc-wasm` bindings crate (wasm-bindgen 0.2; checks on host and wasm32)
- [x] Expose parse, distance, RUC calc and (de)serialise functions (`parseNmea`, `haversineM`, `liability`, `adminFeeInclGstMicros`, `billableKm`, and `TripTracker` with JSON fixes)
- [x] Build script producing an npm package consumed by `pwa/` (`scripts/build-wasm.sh` writes `pwa/wasm-pkg`, which is git-ignored)
- [x] Size/perf optimisation (size-focused release profile with LTO and one codegen unit; wasm-opt allows bulk memory. The module is 161 KB before and 143 KB after, or 70 KB gzipped. Speed not measured)
- [x] Rust-side wasm tests (`wasm-bindgen-test`, run with `wasm-pack test --node crates/tpt-ruc-wasm`)

### PWA scaffold
- [x] Next.js (TypeScript) with `output: export`, Tailwind CSS (`pwa/`, Next 16.4.2, Tailwind 4.3.3, static build tested)
- [x] Lint/format/test setup (ESLint, Prettier, Vitest, Playwright; `npm run lint`, `format:check`, `test`, `test:e2e`, all passing locally)
- [x] PWA manifest, icons, installability (manifest, PNG icons at 192 and 512 px, and a service worker controlling the page are checked end to end; the browser's install prompt itself is not automated)
- [x] Service worker for offline-first shell + WASM caching (hand-written `public/sw.js`, no dependency; an end-to-end test runs the calculator offline, so the WASM module comes from the cache)
- [x] Dependency licence audit passes in CI (JS check passes locally, with the exceptions accepted in docs/LICENSING.md; the `pwa` CI job has not yet run on GitHub)

### Storage
- [ ] IndexedDB schema: vehicles, trips, segments, settings, device keys (defined in `lib/db.ts` with versioned migrations, using `idb` 8.0.4, ISC)
- [x] Storage layer with migrations (`idb`, ISC; migrations and the backup round trip are tested in Chromium)
- [x] Export/import of local data (backup) (download and restore from file, tested end to end; a non-backup file is refused)
- [x] Persistent storage request and quota handling (asks for persistent storage, shows usage against the quota, and warns at 80%; tested in Chromium)

### Features
- [x] Vehicle profiles (CRUD) (create, edit, list and delete, tested end to end)
- [x] Manual odometer entry (optional per vehicle, whole km, shown in the vehicle list; tested end to end)
- [x] Basic RUC calculator UI (distance in, liability out) (home page; runs the wasm liability function, with an end-to-end test for a known charge)
- [ ] Dashboard: current liability, distance until licence expiry
- [ ] Trip history list and detail views (a manually entered trip list, with delete, and a trip detail view with the estimated charge on the trip's date, are done and tested; GPS-recorded trips are still to do)
- [x] CSV export (trips as `vehicle,date,distance_km`, with formula-injection protection and an end-to-end download test)
- [x] Persistent **Section 43 disclaimer** in the UI (first-run acknowledgement gates the app; the footer is on every page)
- [ ] Settings: units, rate-table version, privacy/sync toggles (off by default) (a settings section with "show the Section 43 notice again" is done and tested; units, rate-table version and sync toggles are still to do)
- [ ] Accessibility pass (keyboard, contrast, screen reader labels)
- [ ] Deploy the static site via GitHub Pages or Cloudflare Pages from CI
- [ ] Phase 2 exit: tag `v0.2.0`, usable offline as a manual RUC tracker

## Phase 3 – The Hardware MVP (Weeks 6-8)

### Bench hardware
- [x] Select dev boards: ESP32-S3-DevKitC-1 and ESP32-C3-DevKitM-1 (recommended in `hardware/README.md`; not yet bought or tested)
- [x] Select GNSS breakout: u-blox NEO-M8N, 9600 baud 8N1, 1 Hz (recommended; confirm on hardware)
- [x] Breadboard wiring diagram and pin map for both chips (proposed in `hardware/README.md`; not yet wired)
- [x] Start the BOM draft in `hardware/` (`hardware/bench-bom.csv`, prices to be quoted)

### Firmware
- [x] Create the `tpt-ruc-firmware` crate with chip feature flags (`esp32s3`, `esp32c3`, `esp32c6`; enabling two is a compile error. The esp-hal driver is separate and not yet written)
- [x] Toolchain config for Xtensa (espup) and RISC-V (espup installed the S3 toolchain; `firmware/esp/.cargo/config.toml` holds the per-chip targets. Not yet in CI)
- [ ] Verify the esp-hal and BLE stack (`esp-wifi`/`trouble` or `bleps`) licences (MIT/Apache dual)
- [x] GNSS UART reader task feeding `tpt-ruc-core` (`firmware/esp` reads UART1 at 9600 baud and runs the recorder. It links for ESP32-C6, ESP32-S3 and ESP32-P4, but none has been flashed or run yet)
- [x] Distance accumulation on-device using the core (`Recorder` runs the core tracker on the device; host-tested)
- [ ] Flash storage for trip segments (LittleFS or log-structured; wear levelling; power-fail safe)
- [ ] Data-loss prevention when BLE is disconnected (a bounded in-RAM segment queue that counts drops is done and tested; flash persistence and the BLE sender are still to do)
- [ ] Custom BLE GATT service and UUID design (live data, trip sync, device info, time sync)
- [x] MessagePack payload encoding of timestamp/lat/lon/speed/fix quality (version 1 array, pinned by golden bytes; `tpt-ruc-firmware::payload`)
- [ ] Chunking and MTU negotiation for trip history transfer
- [ ] Pairing/bonding approach and device identity (per-device ed25519 keypair stored on device)
- [ ] OTA update plan (decide now or defer)
- [ ] Low-power modes and watchdog
- [ ] Firmware config (GNSS rate, filters) via BLE
- [x] Host-side tests for the firmware logic (21 tests: GNSS framing, fix merging, recorder, segment queue, payload)

### PWA ↔ ESP32
- [ ] Web Bluetooth: scan, connect, reconnect, subscribe to notifications
- [ ] Pass BLE bytes into the WASM core and update the live UI
- [ ] Download stored trip segments from the device and merge into IndexedDB (dedupe)
- [ ] Connection state UX, error handling and permissions flow
- [ ] Capability detection and **graceful message for iOS Safari/Firefox** (no Web Bluetooth)
- [ ] Document supported browsers/platforms
- [ ] Real-world drive tests, with comparison against a known odometer/route
- [ ] Phase 3 exit: tag `v0.3.0`, end-to-end device → PWA demo

## Phase 4 – The Backend & Sync (Weeks 9-11)

### Backend core
- [ ] Create the `tpt-ruc-backend` crate (Axum, tokio, tower)
- [ ] DB layer supporting **SQLite and PostgreSQL** (sqlx or similar; licence check)
- [ ] Migrations for both databases; CI tests against both
- [ ] Config (env + file), structured logging (`RUST_LOG`), health/readiness endpoints
- [ ] OpenAPI spec (verify generator licence) and versioned API (`/v1`)
- [ ] Rate limiting, request size limits, CORS for the PWA

### Auth & multi-tenancy
- [ ] Single-owner mode: one API key, vehicles under one owner
- [ ] Org mode: users, orgs, roles (admin/manager/driver)
- [ ] Password hashing (argon2) and session/token strategy
- [ ] Per-device ed25519 key registration and revocation
- [ ] Signature verification of sync payloads; replay protection (nonce/sequence)
- [ ] Mode switch and docs for upgrading single-owner → org

### Ingest & fleet
- [ ] Ingest endpoints (REST) for summaries and batched trips; optional WebSocket
- [ ] **Summaries-only by default**; explicit opt-in endpoint for raw GPS points
- [ ] Idempotent uploads and conflict handling
- [ ] Vehicles, drivers, device assignment
- [ ] Fleet aggregation: per vehicle/month/org, liability, licence expiry alerts
- [ ] Re-validate distance and liability server-side using `tpt-ruc-core`
- [ ] Data retention and deletion endpoints (privacy: export/delete my data)

### Reports & eRUC
- [ ] CSV reports formatted for NZTA auditing
- [ ] PDF reports (licence-checked PDF crate, e.g. printpdf or genpdf)
- [ ] Define the generic eRUC export schema (JSON/CSV) and document it in `docs/`
- [ ] `ErucProvider` adapter trait and a generic exporter
- [ ] Push-to-provider job queue with retries and audit log
- [ ] Placeholder tasks per real provider (blocked on you supplying API docs)

### PWA sync
- [ ] Sync settings UI (backend URL, API key/login, opt-in toggles)
- [ ] Device key generation (WebCrypto ed25519, or a WASM fallback) and secure storage
- [ ] Sync engine: IndexedDB → backend with batching, retries and offline queue
- [ ] Sign payloads; show sync status and errors
- [ ] Fleet views in the PWA (if online/org mode)

### Deployment
- [ ] Multi-stage Dockerfile (small image, non-root, healthcheck)
- [ ] Multi-arch images (amd64 + arm64 for Raspberry Pi/NAS), published to ghcr.io from CI
- [ ] `docker-compose.yml` (SQLite default; Postgres profile) with the `./data` volume
- [ ] Optional bundled PWA static files served by the backend
- [ ] Backups guidance, TLS/reverse-proxy guide (Caddy/Traefik example)
- [ ] Integration tests: PWA ↔ backend end-to-end
- [ ] Security review: input validation, auth, dependency audit (`cargo audit`)
- [ ] Phase 4 exit: tag `v0.4.0`, `docker compose up` works on a Raspberry Pi

## Phase 5 – Hardware Refinement & Mobile FFI (Weeks 12+)

### Custom PCB
- [ ] Requirements: ESP32-S3 + GNSS, 12V/24V input, OBD-II and hardwire options
- [ ] Power design: automotive buck regulator, transient/reverse-polarity protection
- [ ] LiPo backup + charger and brown-out/clean-shutdown detection (ties into firmware flush-to-flash)
- [ ] GNSS antenna design and placement (active antenna connector)
- [ ] Schematic in KiCad
- [ ] PCB layout, DRC/ERC clean
- [ ] Gerbers, BOM and pick-and-place output generated in CI
- [ ] Prototype order, assembly and bring-up
- [ ] Firmware board-support for the custom PCB (S3 and RISC-V variant decision)
- [ ] Thermal/vibration/automotive voltage testing
- [ ] Rev B fixes
- [ ] Enclosure design (3D STLs), mounting and cable strain relief
- [ ] Hardware licence decision for designs (e.g. CERN-OHL-P or CC-BY, compatible with the project's permissive stance)
- [ ] Assembly/build instructions in `docs/`

### Mobile bindings (no MPL tooling)
- [ ] Create `tpt-ruc-ffi-ios` using **swift-bridge** (verify licence)
- [ ] Create `tpt-ruc-ffi-android` using the **jni** crate
- [ ] Build scripts for xcframework and Android `.so` (cargo-ndk or similar; licence check)
- [ ] Minimal sample apps (Swift, Kotlin) calling the core
- [ ] Native BLE on iOS/Android feeding the core (fills the iOS Web Bluetooth gap)
- [ ] CI builds for the iOS and Android targets

### eRUC providers
- [ ] Review the provider API docs you supply
- [ ] Implement an adapter per provider with sandbox tests
- [ ] Provider onboarding docs

### NZTA Section 43 approval prep
- [ ] Read Section 43 and the NZTA distance recorder approval requirements; record them in `docs/nzta/`
- [ ] Gap analysis: current design vs approval criteria (accuracy, tamper evidence, security, data integrity)
- [ ] Define accuracy test protocol and test route evidence
- [ ] Collect test results and calibration evidence
- [ ] Draft the application pack and technical documentation
- [ ] Decide whether to apply (outside the open-source stance), and who owns the application
- [ ] Keep disclaimers in README/UI until approval is granted

## Phase 6 – Release & Ongoing

- [ ] User guides: PWA, hardware build, self-hosting, FAQ
- [ ] `docs/` site (mdBook or similar, licence-checked)
- [ ] NZTA compliance doc and rate-table update procedure (how to ship new rates)
- [ ] Privacy statement describing the local-first/zero-knowledge model
- [ ] Final licence audit (`cargo deny`, JS audit, third-party notices) and `v1.0.0` tag
- [ ] Release automation: binaries, Docker images, firmware `.bin`, PWA bundle
- [ ] Maintenance plan: dependency updates, NZTA rate-change monitoring, issue triage
