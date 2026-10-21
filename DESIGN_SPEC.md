# Design Specification: tpt-ruc-nz

Owner: TPT Solutions · Licence: MIT OR Apache-2.0 · Status: living document (supersedes `spec.txt`)

## 1. Executive Summary

tpt-ruc-nz is a privacy-first, open-source, end-to-end ecosystem for tracking, calculating, and managing New Zealand Road User Charges (RUC). It ranges from fully self-contained, offline hardware and software to integrated cloud APIs for fleet management and eRUC provider offloading.

Built on a unified Rust core, the project aims for memory safety, performance, and cross-platform compatibility: ESP32 firmware, Web/WASM, native backend, and iOS/Android via native FFI bindings.

## 2. Core Principles

- **Privacy & local-first:** all raw GPS and distance data is processed and stored locally by default. Cloud sync is strictly opt-in.
- **Write once, run everywhere:** core logic (maths, parsing, RUC rates) is written once in Rust and compiled to multiple targets.
- **Solo-maintainer friendly:** strict architectural boundaries, heavy reliance on the Rust compiler for safety, and automated CI to minimise manual debugging.
- **Hardware agnostic:** a reference ESP32 design is provided, but the software accepts standard NMEA and JSON GPS data from any source.
- **Permissive licensing only:** every dependency must be MIT, BSD, ISC, Zlib, Unlicense/CC0, or dual MIT/Apache-2.0. Apache-2.0-only and MPL-2.0 dependencies are excluded (so no protobuf/prost, no uniffi, no cbindgen). Enforced by `cargo-deny` (see `deny.toml`).

## 3. Decisions Locked In

| Topic | Decision |
|---|---|
| Serialisation | MessagePack (`rmp-serde`) for BLE and sync wire format. JSON for config and GPS input |
| Firmware targets | ESP32-S3 (Xtensa) and an ESP RISC-V chip (C3/C6), one crate with chip feature flags |
| Core portability | `thumbv7em-none-eabihf` is a `no_std` portability check for the core, not a firmware target |
| Mobile FFI | `swift-bridge` (iOS) and the `jni` crate (Android). UniFFI is replaced |
| Web Bluetooth | iOS Safari and Firefox lack it. Accepted limitation: iOS BLE comes via the native app |
| RUC scope | Light and heavy vehicles, full NZTA types. Rates are a versioned data table, not hardcoded |
| Backend database | SQLite for v1. PostgreSQL is deferred until a deployment needs it |
| Auth | Single-owner API-key mode (default for self-host) plus org/user/role mode for fleets. Per-device ed25519 keys |
| Sync security | TLS plus ed25519-signed payloads. Summaries only by default; raw GPS sync is opt-in |
| eRUC export | Generic documented JSON/CSV format plus an adapter trait. Real provider adapters wait for provider API docs |
| PWA | Next.js static export plus service worker (no Node server). Backend can optionally serve it |
| Hosting | GitHub org `tpt-solutions`, GitHub Actions CI. No hosted service: operators run their own. Container images are built on demand (manual workflow), not published by default |
| Extras in scope | Hardware track (enclosure, BOM, power, LiPo shutdown) and NZTA Section 43 approval prep |

## 4. System Architecture

The project is a Cargo workspace monorepo, alongside hardware designs and a web frontend.

```
tpt-ruc-nz/
├── DESIGN_SPEC.md          # This document
├── Cargo.toml              # Rust workspace root
├── deny.toml               # cargo-deny licence and advisory policy
├── crates/
│   ├── tpt-ruc-core/       # Pure Rust logic (no_std + alloc, std feature)
│   ├── tpt-ruc-wasm/       # wasm-bindgen bindings for the PWA (Phase 2)
│   ├── tpt-ruc-firmware/   # ESP32 firmware, esp-hal (Phase 3)
│   ├── tpt-ruc-backend/    # Self-hostable API, Axum (Phase 4)
│   ├── tpt-ruc-ffi-ios/    # swift-bridge bindings (Phase 5)
│   └── tpt-ruc-ffi-android/# jni bindings (Phase 5)
├── pwa/                    # Progressive Web App (TypeScript, Next.js, WASM)
├── hardware/               # KiCad schematics, PCB layout, BOM, 3D STLs
└── docs/                   # User guides, NZTA compliance, build instructions
```

Crates are added to the workspace when their phase starts, so the workspace only lists crates that exist.

## 5. Component Breakdown

### 5.1 tpt-ruc-core (the brain)

The foundational library holding all business logic, maths, and data models. It is `no_std` with `alloc`, and enables `std` through a feature flag.

Responsibilities:

- **NMEA parsing:** raw GPS sentences into coordinate structs (GGA, RMC, VTG, GSA; GP, GN, GL, GA, BD talkers). Checksums are validated and malformed sentences are rejected.
- **Distance calculation:** Haversine formula using `libm`, with fix-quality, HDOP and speed filters.
- **RUC engine:** liability from vehicle class, powertrain, weight, axles, and trailers, using a versioned, effective-dated rate table. Rates are verified against nzta.govt.nz before release (see `todo.md`).
- **Data serialisation:** serde derives for JSON and MessagePack.

Compilation targets:

- Native (backend, tests)
- `thumbv7em-none-eabihf` (`no_std` portability check, and firmware)
- `wasm32-unknown-unknown` (PWA via wasm-bindgen)
- Native FFI bindings for iOS and Android (Phase 5)

### 5.2 tpt-ruc-firmware (the hardware node)

The reference hardware implementation for the physical distance recorder.

- **Hardware:** ESP32-S3 (low-power BLE/Wi-Fi) for the custom PCB and the bench build, plus a high-accuracy GNSS module (u-blox NEO-M8N or Quectel L80).
- **Power:** designed for 12 V/24 V vehicle systems (OBD-II or hardwire), with a small LiPo backup for clean shutdowns.
- **Responsibilities:**
  - Read UART data from the GNSS module and pass it to `tpt-ruc-core`.
  - Accumulate distance on-device.
  - Store trip segments in flash (LittleFS or log-structured, power-fail safe) so nothing is lost if BLE disconnects.
  - Broadcast live data and trip history over a custom BLE GATT service.

### 5.3 tpt-ruc-backend (the server)

A lightweight, self-hostable API for data aggregation, fleet management, and reporting.

- **Language:** Rust, Axum, tokio, tower.
- **Database:** SQLite (simple self-hosting). PostgreSQL is deferred until a deployment needs it.
- **Responsibilities:**
  - Receive signed trip summaries (and, opt-in only, raw points) from the PWA.
  - Re-validate distance and liability using `tpt-ruc-core`.
  - Aggregate data across vehicles and organisations.
  - Generate CSV and PDF reports formatted for NZTA auditing.
  - Format and push data to approved eRUC providers via adapters.
- **Deployment:** fully containerised with `docker-compose.yml` for one-command self-hosting on a Raspberry Pi or NAS.

### 5.4 The PWA (the gateway and UI)

The user-facing interface that bridges hardware and backend.

- **Stack:** TypeScript, Next.js (static export), Tailwind CSS.
- **Responsibilities:**
  - Web Bluetooth: scan, connect, and read data from the ESP32.
  - WASM: load `tpt-ruc-core` to do the heavy calculations in the browser.
  - Offline-first storage in IndexedDB.
  - Sync engine: push to the self-hosted backend or cloud API when the user chooses.
  - UI: dashboard for RUC liability, trip history, vehicle profiles, and export tools.

## 6. Data Flow and Protocols

**Hardware to phone (BLE):** the ESP32 broadcasts a custom GATT service. Data is sent as MessagePack payloads containing timestamp, latitude, longitude, speed, and fix quality.

**Phone processing (WASM):** the PWA receives the BLE payload and passes the bytes to the `tpt-ruc-core` WASM module. WASM parses the coordinates, calculates the distance delta, updates the running RUC liability, and returns structured data to the UI.

**Phone to backend (HTTPS):** when online, the PWA batches processed trip data and sends it to the backend over REST (WebSocket optional). Payloads are signed with the per-device ed25519 key. Ingest is idempotent.

## 7. Security, Privacy and Legal Compliance

### 7.1 Privacy architecture

- **Zero-knowledge by default:** the backend never sees raw GPS coordinates unless the user explicitly configures the PWA to sync them.
- **Local processing:** distance and RUC calculations happen on the ESP32 or in the browser (WASM). The backend only receives aggregated summaries (for example, "Vehicle X travelled 450 km this month") when sync is enabled.

### 7.2 NZTA legal disclaimer (critical)

Under the NZ Road User Charges Act, claiming RUC deductions (such as off-road travel), or using a digital display in place of a physical licence label, requires an NZTA Approved Distance Recorder (Section 43).

Project stance: tpt-ruc-nz is an open-source reference implementation for personal tracking, estimation, and fleet telemetry. It is not an approved distance recorder.

The README and the UI must prominently display a disclaimer that users must apply for Section 43 approval through NZTA before using the system for official RUC licensing or tax deductions.

## 8. Deployment and Self-Hosting

```yaml
services:
  tpt-ruc-api:
    image: ghcr.io/tpt-solutions/tpt-ruc-backend:latest
    ports:
      - "8080:8080"
    volumes:
      - ./data:/app/data   # SQLite persistence
    environment:
      - RUST_LOG=info
```

The image name is a placeholder until the GitHub org and repo are confirmed (see `todo.md`).

## 9. Phased Delivery Roadmap

Each phase ends with a tagged release and a working demo.

1. **Phase 0, Foundations and governance:** licences, repo skeleton, CI baseline, licence policy.
2. **Phase 1, Core and maths (weeks 1-2):** Cargo workspace, `tpt-ruc-core` (NMEA, Haversine, RUC rate logic), unit and property tests.
3. **Phase 2, PWA and WASM (weeks 3-5):** Next.js PWA scaffold, core compiled to WASM, manual odometer entry, basic RUC calculator UI.
4. **Phase 3, Hardware MVP (weeks 6-8):** off-the-shelf ESP32 and GPS breakout boards (no custom PCB yet). Firmware reads GPS and broadcasts over BLE. PWA connects via Web Bluetooth.
5. **Phase 4, Backend and sync (weeks 9-11):** `tpt-ruc-backend` (hyper and tokio, SQLite; PostgreSQL deferred), PWA sync, Docker setup for self-hosting.
6. **Phase 5, Hardware refinement and FFI (weeks 12+):** custom KiCad PCB for the ESP32 GPS node, iOS and Android bindings, eRUC provider export formats.
7. **Phase 6, Release and ongoing:** user guides, licence audit, `v1.0.0`, maintenance.
