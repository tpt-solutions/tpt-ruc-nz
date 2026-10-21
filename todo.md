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
- [x] Optional smoothing/dedupe of points (exact repeat epochs are rejected as `Duplicate`; median smoothing is optional, off by default, window capped at 5)

### RUC engine
- [x] Vehicle model: NZTA RUC type code plus RUC weight. The type code encodes axles, configuration and powertrain, and the caller chooses it. Petrol exemption is the caller's decision (no RUC charged)
- [x] Rates as a **versioned, data-driven table** with effective dates. The NZTA table from the 2026 page is in `ruc_rates.rs`, effective from 2026-08-06 (see the open item below)
- [x] Light vehicle liability (<=3.5t): type 1 and 12 bands, from the NZTA page
- [x] Heavy vehicle liability: weight bands per type, from the NZTA page. Trailers are separate licences, so there is no trailer surcharge. Additional overweight licences are in `ruc_rates.rs`
- [x] GST handling: NZTA rates are GST-inclusive, so the engine computes the inclusive charge and splits GST out, rounding half up
- [x] Licence purchase, odometer-based licence expiry and refund modelling (`licence.rs` models odometer expiry, date limits, the overlap licence and a pro-rata refund. The overlap and refund rules are a model marked **requires verification** against NZTA)
- [x] Off-road/exempt distance deductions (flagged "requires Section 43 approval") (`billable_after_exemption` refuses unless the `section43_approved` flag is true)
- [x] Historical-rate lookups for backdated trips (`rate_for` picks the table in force on the trip date)
- [ ] Verify all rates and rules against NZTA and cite them in docs (powered, unpowered, crane, vintage, motor caravan, overweight and H-type distance rates transcribed from the 2026 page; source and open items in `docs/nzta/rates-research.md`)

### Serialisation
- [x] `serde` derives for all models (`no_std`-friendly) (behind the `serde` feature: coordinates, NMEA types and sentences, `Fix`, `Config`, `Segment`, and the RUC models. RUC models are serialise-only because they hold `'static` borrows; `PurchaseMethod` round-trips. Wire tests pin `Liability` and `Vehicle`)
- [x] JSON via `serde_json` (std feature): `Fix` and `Config` round-trip in tests; the library itself does not link `serde_json`
- [x] MessagePack via `rmp-serde` (verify `no_std` support, else alternative) (firmware encodes with the `no_std` `rmp` crate, pinned by golden bytes; `rmp-serde` stays host-only)
- [x] Define and version the BLE payload schema: timestamp, lat, lon, speed, fix quality (JSON form of `Fix` is pinned as v1 in `tests/golden/fix_v1.json`; the binary layout is in `docs/ble-design.md`, with a golden-byte test)
- [x] Golden-file tests for wire formats (`Fix` v1 JSON; MessagePack round-trip)

### Quality
- [x] Unit tests for NMEA, Haversine, filters and RUC (known-distance fixtures; RUC mechanics tests use fixture rates, and `ruc_rates` tests check the NZTA values and weight-band invariants)
- [ ] Property tests (proptest) and fuzzing (cargo-fuzz) for the parser (proptest done. `fuzz/` has NMEA and Fix JSON targets, written and compiling, but not yet run: that needs nightly and cargo-fuzz)
- [ ] Real-world NMEA log fixtures (the README in `tests/fixtures/` says what to capture; only a labelled synthetic fixture exists. Needs real captures from you)
- [x] CI: `cargo check` for core on `wasm32-unknown-unknown` and `thumbv7em-none-eabihf`
- [ ] CI: `no_std` build check and coverage report (no_std check done in CI; `.github/workflows/coverage.yml` is written with cargo-llvm-cov but has not run yet)
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
- [x] IndexedDB schema: vehicles, trips, segments, settings, device keys (defined in `lib/db.ts` with versioned migrations, using `idb` 8.0.4, ISC. Device keys use a non-extractable private key. A unit test checks the stores with a recording fake, since the unit runner has no IndexedDB)
- [x] Storage layer with migrations (`idb`, ISC; migrations and the backup round trip are tested in Chromium)
- [x] Export/import of local data (backup) (download and restore from file, tested end to end; a non-backup file is refused)
- [x] Persistent storage request and quota handling (asks for persistent storage, shows usage against the quota, and warns at 80%; tested in Chromium)

### Features
- [x] Vehicle profiles (CRUD) (create, edit, list and delete, tested end to end)
- [x] Manual odometer entry (optional per vehicle, whole km, shown in the vehicle list; tested end to end)
- [x] Basic RUC calculator UI (distance in, liability out) (home page; runs the wasm liability function, with an end-to-end test for a known charge)
- [ ] Dashboard: current liability, distance until licence expiry (`pwa/components/Dashboard.tsx` shows this month's liability per vehicle, and the expiry date of a heavy licence bought before the newest rate change, with the days left. The date comes from the core rule, exported through the wasm `heavyLicenceExpiresOn`. Distance until expiry is NOT shown: the odometer-based model in the core needs the purchased licence distance, which the app does not store yet)
- [x] Trip history list and detail views (a manually entered trip list, with delete, and a trip detail view with the estimated charge on the trip's date, are done and tested. Device-recorded trips appear in the same list, with their origin and upload state)
- [x] CSV export (trips as `vehicle,date,distance_km`, with formula-injection protection and an end-to-end download test)
- [x] Persistent **Section 43 disclaimer** in the UI (first-run acknowledgement gates the app; the footer is on every page)
- [x] Settings: units, rate-table version, privacy/sync toggles (off by default) (kilometres only, which the RUC model requires. The rate-table version is read from a live liability call. Upload is off by default, and backend settings persist)
- [x] Accessibility pass (keyboard, contrast, screen reader labels) (contrast is checked for 12 text and background pairs, all at or above 4.5:1, with one close pair at about 4.55:1. An end-to-end audit found no unlabelled controls and tests Tab order. axe-core is only a transitive dependency, so it was not used)
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
- [x] Toolchain config for Xtensa (espup) and RISC-V. CI builds the C6 and P4 (RISC-V) binaries and checks the device licence policy. The S3 (Xtensa) build is local only, because it needs the espup toolchain
- [ ] Verify the esp-hal and BLE stack (`esp-hal` 1.2.2, `esp-println`, `esp-backtrace` and `esp-alloc` are MIT/Apache and pass the device policy. The BLE stack is not chosen yet. The device policy has two advisory exceptions and one licence exception, in `firmware/esp/deny.toml`)
- [x] GNSS UART reader task feeding `tpt-ruc-core` (`firmware/esp` reads UART1 at 9600 baud and runs the recorder. It links for ESP32-C6, ESP32-S3 and ESP32-P4, but none has been flashed or run yet)
- [x] Distance accumulation on-device using the core (`Recorder` runs the core tracker on the device; host-tested)
- [ ] Flash storage for trip segments (LittleFS or log-structured; wear levelling; power-fail safe) (`flash.rs` is a sector log with round-robin wear levelling, CRC-checked sector headers, sealing of torn tails, and retirement of the oldest sector after sync. A host test cuts power at every operation and checks recovery, and a mutation check confirmed the test catches a broken seal. The on-chip flash driver behind `SectorStorage` is not written, so this is not yet verified on hardware)
- [ ] Data-loss prevention when BLE is disconnected (a bounded in-RAM segment queue that counts drops is done and tested, and segments can now be stored in the sector log. Connecting the queue to flash and the BLE sender are still to do)
- [x] Custom BLE GATT service and UUID design (version 1 layout in `docs/ble-design.md`: live fix, segment transfer, control and device info characteristics. Not implemented yet)
- [x] MessagePack payload encoding of timestamp/lat/lon/speed/fix quality (version 1 array, pinned by golden bytes; `tpt-ruc-firmware::payload`)
- [ ] Chunking and MTU negotiation (the firmware chunk encoder and reassembler are implemented in `transfer.rs` and checked against the PWA reassembler's framing; MTU negotiation and on-device timing are not measured)
- [ ] Pairing/bonding approach and device identity (per-device ed25519 keypair stored on device) (`identity.rs` signs and verifies with Ed25519, checked against the RFC 8032 test vector. Generating and storing the seed on the chip, and the pairing and bonding approach, are still to do)
- [ ] OTA update plan (decide now or defer)
- [ ] Low-power modes and watchdog
- [ ] Firmware config (GNSS rate, filters) via BLE (command parser and validation are in `control.rs`, host-tested. Commands 0x11 and 0x12 are documented in `docs/ble-design.md`. Not yet wired to the GATT characteristic)
- [x] Host-side tests for the firmware logic (21 tests: GNSS framing, fix merging, recorder, segment queue, payload)

### PWA ↔ ESP32
- [ ] Web Bluetooth: scan, connect, reconnect, subscribe to notifications (`pwa/lib/ble-connection.ts` is written and unit-tested against a mocked `navigator.bluetooth`. It is NOT tested on a board)
- [ ] Pass BLE bytes into the WASM core and update the live UI (`pwa/lib/live-track.ts` is written and tested. It has not been run against a device)
- [ ] Download stored trip segments from the device and merge into IndexedDB (dedupe) (download, save, then acknowledge, so the device keeps segments if saving fails. Mocked tests only. The acknowledgement timing in `docs/ble-design.md` is ambiguous, and the simplest reading was used. Not tested on a board)
- [x] Connection state UX, error handling and permissions flow (unsupported, idle, connecting, connected and error states, each with a message)
- [x] Capability detection and **graceful message for iOS Safari/Firefox** (no Web Bluetooth) (specific messages for iOS and Firefox. Their behaviour is reasoned about, not tested on the browsers)
- [x] Document supported browsers/platforms (`pwa/README.md` has a SUPPORTED_PLATFORMS table)
- [ ] Real-world drive tests, with comparison against a known odometer/route
- [ ] Phase 3 exit: tag `v0.3.0`, end-to-end device → PWA demo

## Phase 4 – The Backend & Sync (Weeks 9-11)

### Backend core
- [x] Create the `tpt-ruc-backend` crate (hyper and tokio, with our own routing, CORS and request logging; `cargo deny` passes. Axum and sqlx were dropped because they pulled in `sync_wrapper` (Apache-2.0 only) and `ryu` (BSL-1.0, not allowed))
- [ ] DB layer supporting **SQLite and PostgreSQL** (sqlx or similar; licence check) (SQLite via rusqlite, bundled, is done. PostgreSQL is BLOCKED for now: the handlers are synchronous, about 34 call sites would need an async rewrite, and there is no PostgreSQL server here to test against. tokio-postgres passes the licence check)
- [ ] Migrations for both databases; CI tests against both (SQLite migration `0001_init.sql` runs at startup and is tested; the PostgreSQL migrations and CI job are still to do)
- [ ] Config (env + file), structured logging (`RUST_LOG`), health/readiness endpoints (env config, `RUST_LOG` logging, `/healthz` and `/readyz` are done; a config file is not)
- [x] OpenAPI spec (verify generator licence) and versioned API (`/v1`) (`docs/api/openapi.yaml` covers all 23 operations, hand-written with no generator. It is checked with Python's yaml parser, since no YAML crate is in the tree)
- [x] Rate limiting, request size limits, CORS for the PWA (body size limit and CORS allow-list are done. Rate limiting is per client IP: 120 requests a minute, and a client is blocked for 15 minutes after 10 failed API key checks in that window. It sees the TCP peer address, so behind a reverse proxy all clients share the proxy's limit. Trusting `X-Forwarded-For` is not built)

### Auth & multi-tenancy
- [x] Single-owner mode: one API key, vehicles under one owner (`TPT_RUC_API_KEY`, at least 32 characters, sent as a Bearer token and compared in constant time; the server refuses to start without it. Vehicles are not yet modelled server-side, so trips carry the vehicle fields directly)
- [ ] Org mode: users, orgs, roles (admin/manager/driver) (users, roles, and vehicles with driver assignment are done. Orgs are not built: there is one organisation. Admins create users with `POST /v1/users`, and each user gets a token shown once. Drivers see and delete only their own trips and cannot run reports. Managers see every trip and run reports. Admins also manage users, and deleting a user deletes their trips. Orgs are not built: there is one organisation)
- [ ] Password hashing (argon2) and session/token strategy (users sign in with random 256-bit tokens, and only their SHA-256 hashes are stored. Passwords and sessions are not built. Tokens cannot be rotated yet, so a lost token means deleting the user and creating another)
- [x] Per-device ed25519 key registration and revocation (`POST /v1/devices` registers a key owned by a user, using ed25519-dalek, BSD-3-Clause. Revoked devices are refused)
- [x] Signature verification of sync payloads; replay protection (nonce/sequence) (signed uploads carry a per-device sequence that must rise, so replays and older sequences are refused. Owner and user tokens still work unsigned)
- [ ] Mode switch and docs for upgrading single-owner → org (the owner key and users already coexist. A written upgrade guide is not yet done)

### Ingest & fleet
- [ ] Ingest endpoints (REST) for summaries and batched trips; optional WebSocket (`POST /v1/trips` and `GET /v1/trips` are done; WebSocket is not built, and that is deliberate for now)
- [ ] **Summaries-only by default**; explicit opt-in endpoint for raw GPS points (only summaries can be sent, so the default holds; the raw GPS opt-in endpoint is not built)
- [x] Idempotent uploads and conflict handling (a resent trip with the same values is `unchanged`; the same ID with different values is a `409` and the whole batch is refused, atomically)
- [x] Vehicles, drivers, device assignment (`/v1/vehicles`, with drivers restricted to vehicles assigned to them. Trips may name a `vehicle_id`)
- [x] Fleet aggregation: per vehicle/month/org, liability, licence expiry alerts (`GET /v1/fleet/summary` groups by vehicle, type and month. `GET /v1/alerts` lists heavy vehicles with licences expiring soon, using only the core's expiry rules. Grouping by org waits for orgs)
- [x] Re-validate distance and liability server-side using `tpt-ruc-core` (the server computes every charge; client-sent amounts are ignored)
- [x] Data retention and deletion endpoints (privacy: export/delete my data) (`GET /v1/export` returns the caller's trips as versioned JSON. `DELETE /v1/trips?from=&to=` deletes an inclusive date range. Optional `TPT_RUC_RETENTION_DAYS` runs an hourly task that deletes older trips)

### Reports & eRUC
- [x] CSV reports formatted for NZTA auditing (`GET /v1/reports/trips.csv`, one line per trip with distance, billed km, rate version and charge; `GET /v1/reports/monthly.csv`, totals per month and vehicle type. Both take optional `from` and `to` dates. Charges are NZD to the cent, rounded once from exact totals. Text fields are escaped, and formula-like values are neutralised. PDF reports are still to do)
- [x] PDF reports (licence-checked PDF crate, e.g. printpdf or genpdf) (`GET /v1/reports/monthly.pdf` uses lopdf 0.46 with default features off, MIT. pdf-writer was blocked by ryu)
- [ ] Define the generic eRUC export schema (JSON/CSV) and document it in `docs/`
- [ ] `ErucProvider` adapter trait and a generic exporter
- [ ] Push-to-provider job queue with retries and audit log
- [ ] Placeholder tasks per real provider (blocked on you supplying API docs)

### PWA sync
- [x] Sync settings UI (backend URL, API key/login, opt-in toggles) (the API key is stored in this browser, and the UI warns about that)
- [x] Device key generation (WebCrypto ed25519, or a WASM fallback) and secure storage (WebCrypto Ed25519 with a non-extractable key. If the browser lacks Ed25519, signing is turned off and the UI says so. The backend does not yet verify PWA signatures)
- [x] Sync engine: IndexedDB → backend with batching, retries and offline queue (batched uploads, retries honouring `Retry-After`, an offline queue, and conflicts and rejections shown with a retry action)
- [x] Sign payloads; show sync status and errors (signed when the browser supports Ed25519. Status and errors are shown)
- [x] Fleet views in the PWA (if online/org mode) (uses `/v1/fleet/summary`, so it needs a manager or admin key)
- [ ] In-app sync schedule setting: manual, on open, every 15 minutes, hourly or daily, plus sync on `visibilitychange` and `online`
- [ ] Periodic Background Sync via the service worker (progressive enhancement: Chromium and installed PWAs only; the browser picks the actual timing)

### Deployment
- [x] Multi-stage Dockerfile (small image, non-root, healthcheck) (`Dockerfile`: a Rust build stage and a Debian slim runtime that runs as user 10001. The health check is `tpt-ruc-backend healthcheck`, so no `curl` is needed. Not yet built in a Docker environment, only its build command has been checked outside Docker)
- [ ] Multi-arch images (amd64 + arm64 for Raspberry Pi/NAS), published to ghcr.io from CI
- [ ] `docker-compose.yml` (SQLite default; Postgres profile) with the `./data` volume (`docker-compose.yml` runs with SQLite on a named volume, `tpt-ruc-data`, because a bind-mounted host folder is not writable by the container's user. The Postgres profile is not built)
- [ ] Optional bundled PWA static files served by the backend
- [ ] Backups guidance, TLS/reverse-proxy guide (Caddy/Traefik example)
- [ ] Integration tests: PWA ↔ backend end-to-end
- [ ] Security review: input validation, auth, dependency audit (`cargo audit`) (an internal review fixed a cross-user trip-ID leak, migration 0005. Advisories are checked by `cargo deny`. `cargo audit` itself has not been run, and a human review is still needed)
- [ ] Phase 4 exit: tag `v0.4.0`, `docker compose up` works on a Raspberry Pi

## Phase 5 – Hardware Refinement & Mobile FFI (Weeks 12+)

### Custom PCB
- [ ] Requirements: ESP32-S3 + GNSS, 12V/24V input, OBD-II and hardwire options (not yet written as a requirements document; the schematic is also not started)
- [ ] Power design: automotive buck regulator, transient/reverse-polarity protection
- [ ] LiPo backup + charger and brown-out/clean-shutdown detection (ties into firmware flush-to-flash)
- [ ] GNSS antenna design and placement (active antenna connector)
- [ ] Schematic in KiCad (BLOCKED: needs KiCad, which is not available in this environment)
- [ ] PCB layout, DRC/ERC clean
- [ ] Gerbers, BOM and pick-and-place output generated in CI (BLOCKED on the schematic and PCB layout)
- [ ] Prototype order, assembly and bring-up (BLOCKED on the PCB design)
- [ ] Firmware board-support for the custom PCB (S3 and RISC-V variant decision)
- [ ] Thermal/vibration/automotive voltage testing
- [ ] Rev B fixes
- [ ] Enclosure design (3D STLs), mounting and cable strain relief (BLOCKED: no CAD tool here)
- [ ] Hardware licence decision for designs (e.g. CERN-OHL-P or CC-BY, compatible with the project's permissive stance) (options compared in `docs/hardware-licence-options.md`, with a recommendation for you to decide. The licence texts were not checked against their official sources)
- [x] Assembly/build instructions in `docs/` (bench build in `docs/guides/hardware-build.md`, untested; custom PCB build in `docs/pcb-build.md` is BLOCKED until the schematic exists)

### Mobile bindings (no MPL tooling)
- [x] Create `tpt-ruc-ffi-ios` using **swift-bridge** (verify licence) (`crates/tpt-ruc-ffi-ios`; swift-bridge 0.1.59 is usable under its MIT option. Returns NZD as `Double` because swift-bridge cannot return `u64`. Host-compiled; iOS target not built here)
- [x] Create `tpt-ruc-ffi-android` using the **jni** crate (`crates/tpt-ruc-ffi-android`, jni 0.22.4, MIT OR Apache-2.0. JNI symbols present in the host build; Android target not built here)
- [ ] Build scripts for xcframework and Android `.so` (cargo-ndk or similar; licence check) (scripts are written as text and have not been run. The Swift glue-generation step is missing, and the cargo-ndk licence still needs checking)
- [x] Minimal sample apps (Swift, Kotlin) calling the core (under `samples/`, marked untested)
- [ ] Native BLE on iOS/Android feeding the core (fills the iOS Web Bluetooth gap) (BLOCKED: needs devices. The plan is in `docs/mobile-ble-plan.md`)
- [ ] CI builds for the iOS and Android targets (BLOCKED: needs macOS and Android SDK runners, not added yet)

### eRUC providers
- [ ] Review the provider API docs you supply
- [ ] Implement an adapter per provider with sandbox tests
- [ ] Provider onboarding docs

### NZTA Section 43 approval prep
- [ ] Read Section 43 and the NZTA distance recorder approval requirements; record them in `docs/nzta/` (criteria the docs cite are marked "needs NZTA source". They must be read from the NZTA primary source)
- [x] Gap analysis: current design vs approval criteria (accuracy, tamper evidence, security, data integrity) (`docs/nzta/section43-gap-analysis.md`; criteria not verified against primary sources)
- [x] Define accuracy test protocol and test route evidence (`docs/nzta/accuracy-test-protocol.md`, a protocol only)
- [ ] Collect test results and calibration evidence
- [ ] Draft the application pack and technical documentation
- [ ] Decide whether to apply (outside the open-source stance), and who owns the application (options memo in `docs/nzta/`, no recommendation to file; decision is yours)
- [ ] Keep disclaimers in README/UI until approval is granted

## Phase 6 – Release & Ongoing

- [x] User guides: PWA, hardware build, self-hosting, FAQ (`docs/guides/`, linked from `docs/README.md`)
- [x] `docs/` site (mdBook or similar, licence-checked) (`docs/README.md` is the index. mdBook is MPL-2.0 tooling, so it is excluded by the licence policy)
- [x] NZTA compliance doc and rate-table update procedure (how to ship new rates) (`docs/nzta/rate-update-procedure.md`)
- [x] Privacy statement describing the local-first/zero-knowledge model (`docs/privacy.md`, checked against the code)
- [ ] Final licence audit (`cargo deny`, JS audit, third-party notices) and `v1.0.0` tag
- [ ] Release automation: binaries, Docker images, firmware `.bin`, PWA bundle (not started: this is publishing, which is excluded from this round)
- [ ] Maintenance plan: dependency updates, NZTA rate-change monitoring, issue triage
