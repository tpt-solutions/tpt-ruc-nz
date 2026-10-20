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

- [ ] Confirm current NZTA RUC rates and vehicle types. The spec's $76 diesel / $38 EV-PHEV figures need checking against nzta.govt.nz
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
- [ ] Add a JS licence check for `pwa/` (e.g. `license-checker` allow-list) (Phase 2, once `pwa/` has a `package.json`)
- [ ] Add a `THIRD_PARTY_LICENSES` generation step (cargo-about or cargo-bundle-licenses) (`about.toml` added; generation not yet wired into CI)
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
- [ ] Accept JSON GPS input as well as NMEA (hardware agnostic)

### Distance
- [x] Haversine implementation using `libm` (no_std floats)
- [ ] Fix-quality, HDOP and speed filters (reject jumps and drift when stationary)
- [ ] Trip segment model: start, stop, accumulated distance, gaps
- [ ] Decide metres vs km precision and rounding rules (RUC bills in km)
- [ ] Optional smoothing/dedupe of points

### RUC engine
- [ ] Vehicle model: class, fuel/powertrain (diesel, petrol-exempt, EV, PHEV), weight, axle config, trailers
- [ ] Rates as a **versioned, data-driven table** with effective dates (embedded + loadable)
- [ ] Light vehicle liability (<=3.5t)
- [ ] Heavy vehicle liability (weight bands, axles, trailers)
- [ ] GST handling
- [ ] Licence purchase, odometer-based licence expiry and refund modelling
- [ ] Off-road/exempt distance deductions (flagged "requires Section 43 approval")
- [ ] Historical-rate lookups for backdated trips
- [ ] Verify all rates and rules against NZTA and cite them in docs

### Serialisation
- [ ] `serde` derives for all models (`no_std`-friendly)
- [ ] JSON via `serde-json-core` or `serde_json` (std feature)
- [ ] MessagePack via `rmp-serde` (verify `no_std` support, else alternative)
- [ ] Define and version the BLE payload schema: timestamp, lat, lon, speed, fix quality
- [ ] Golden-file tests for wire formats

### Quality
- [ ] Unit tests for NMEA, Haversine, filters and RUC (known-distance fixtures)
- [ ] Property tests (proptest) and fuzzing (cargo-fuzz) for the parser
- [ ] Real-world NMEA log fixtures
- [x] CI: `cargo check` for core on `wasm32-unknown-unknown` and `thumbv7em-none-eabihf`
- [ ] CI: `no_std` build check and coverage report (no_std check done in CI; coverage still to add)
- [x] Rustdoc on the public API and a CHANGELOG entry
- [ ] Phase 1 exit: tag `v0.1.0`, all CI green

## Phase 2 – The PWA & WASM (Weeks 3-5)

### WASM
- [ ] Create the `tpt-ruc-wasm` bindings crate (wasm-bindgen + wasm-pack; keeps core free of bindgen)
- [ ] Expose parse, distance, RUC calc and (de)serialise functions
- [ ] Build script producing an npm package consumed by `pwa/`
- [ ] Size/perf optimisation (`wasm-opt`, release profile)
- [ ] Rust-side wasm tests (`wasm-bindgen-test`)

### PWA scaffold
- [ ] Next.js (TypeScript) with `output: export`, Tailwind CSS
- [ ] Lint/format/test setup (ESLint, Prettier, Vitest, Playwright)
- [ ] PWA manifest, icons, installability
- [ ] Service worker for offline-first shell + WASM caching (Workbox or similar, licence-checked)
- [ ] Dependency licence audit passes in CI

### Storage
- [ ] IndexedDB schema: vehicles, trips, segments, settings, device keys
- [ ] Storage layer with migrations (Dexie or idb; licence check)
- [ ] Export/import of local data (backup)
- [ ] Persistent storage request and quota handling

### Features
- [ ] Vehicle profiles (CRUD)
- [ ] Manual odometer entry
- [ ] Basic RUC calculator UI (distance in, liability out)
- [ ] Dashboard: current liability, distance until licence expiry
- [ ] Trip history list and detail views
- [ ] CSV export
- [ ] Persistent **Section 43 disclaimer** in the UI (first-run acknowledgement + footer)
- [ ] Settings: units, rate-table version, privacy/sync toggles (off by default)
- [ ] Accessibility pass (keyboard, contrast, screen reader labels)
- [ ] Deploy the static site via GitHub Pages or Cloudflare Pages from CI
- [ ] Phase 2 exit: tag `v0.2.0`, usable offline as a manual RUC tracker

## Phase 3 – The Hardware MVP (Weeks 6-8)

### Bench hardware
- [ ] Select dev boards: ESP32-S3 DevKit + ESP32-C3/C6 DevKit
- [ ] Select GNSS breakout (u-blox NEO-M8N or Quectel L80) and confirm UART settings/baud/update rate
- [ ] Breadboard wiring diagram and pin map for both chips
- [ ] Start the BOM draft in `hardware/`

### Firmware
- [ ] Create the `tpt-ruc-firmware` crate with chip feature flags (`esp32s3`, `esp32c3`/`esp32c6`)
- [ ] Toolchain config for Xtensa (espup) and RISC-V; CI builds both
- [ ] Verify the esp-hal and BLE stack (`esp-wifi`/`trouble` or `bleps`) licences (MIT/Apache dual)
- [ ] GNSS UART reader task feeding `tpt-ruc-core`
- [ ] Distance accumulation on-device using the core
- [ ] Flash storage for trip segments (LittleFS or log-structured; wear levelling; power-fail safe)
- [ ] Data-loss prevention when BLE is disconnected (store-and-forward)
- [ ] Custom BLE GATT service and UUID design (live data, trip sync, device info, time sync)
- [ ] MessagePack payload encoding of timestamp/lat/lon/speed/fix quality
- [ ] Chunking and MTU negotiation for trip history transfer
- [ ] Pairing/bonding approach and device identity (per-device ed25519 keypair stored on device)
- [ ] OTA update plan (decide now or defer)
- [ ] Low-power modes and watchdog
- [ ] Firmware config (GNSS rate, filters) via BLE
- [ ] Host-side tests for the firmware logic (hardware-in-the-loop optional)

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
