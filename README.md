# tpt-ruc-nz

A privacy-first, open-source toolkit for tracking and estimating New Zealand Road User Charges (RUC): a shared Rust core, an ESP32 reference GPS node, a static PWA, and a self-hostable backend.

> **Important: NZTA Section 43 disclaimer**
>
> tpt-ruc-nz is an open-source reference implementation for personal tracking, estimation and fleet telemetry. **It is not an NZTA-approved distance recorder.**
>
> Under the NZ Road User Charges Act, claiming RUC deductions (such as off-road travel), or using a digital display in place of a physical licence label, requires an NZTA Approved Distance Recorder under Section 43. If you intend to use this system for official RUC licensing or tax deductions, you must apply for Section 43 approval through NZTA first.
>
> Rates in this project are not authoritative until they have been verified against [nzta.govt.nz](https://www.nzta.govt.nz). See [DESIGN_SPEC.md](DESIGN_SPEC.md).

## Status

Early development. Phase 0 (foundations) and Phase 1 (core maths) are in progress. See [todo.md](todo.md) for the full checklist.

| Component | Path | Status |
|---|---|---|
| Core: NMEA parsing, Haversine distance, coordinates | `crates/tpt-ruc-core` | Working, 27 tests |
| RUC rate engine | `crates/tpt-ruc-core` | Not started (rates need verification) |
| WASM bindings and PWA | `crates/tpt-ruc-wasm`, `pwa/` | Planned (Phase 2) |
| ESP32 firmware | `crates/tpt-ruc-firmware` | Planned (Phase 3) |
| Backend | `crates/tpt-ruc-backend` | Working: trips, users, reports, export, rate limits, Docker (Phase 4 in progress) |

## Run the server

You need Docker only. On Windows or Mac, install [Docker Desktop](https://www.docker.com/products/docker-desktop/). On Linux, install Docker Engine with the Compose plugin.

```sh
git clone <repository-url> tpt-ruc-nz
cd tpt-ruc-nz
docker compose up -d
docker compose logs tpt-ruc
```

The first start creates an API key and prints it in the log, under the line "No API key was set". Copy it. Every request needs it. Then check that the server answers:

```sh
curl -H "Authorization: Bearer YOUR-KEY" http://localhost:8080/v1/trips
```

You should see `{"limit":100,"offset":0,"trips":[]}`. On Windows PowerShell, use `curl.exe` instead of `curl`.

| To do this | Run |
|---|---|
| Stop the server, keeping your data | `docker compose down` |
| Start it again | `docker compose up -d` |
| Update after pulling new code | `git pull` then `docker compose up -d --build` |
| Change settings | Copy `.env.example` to `.env`, edit it, then `docker compose up -d` |

Your trips and the saved API key live in the `tpt-ruc-data` Docker volume. Back it up before you upgrade or delete the project.

**Keep the server private.** The compose file only listens on `127.0.0.1`, so only this computer can reach it. Anyone who can read the Docker log can read the API key, so do not share that log. To reach the server from a phone, change the port in `docker-compose.yml` to `8080:8080`, and put it behind an HTTPS reverse proxy such as Caddy first. The key is sent with every request, so never expose the plain HTTP port to the internet.

**Without Docker:** install [Rust](https://rustup.rs), then run `cargo run -p tpt-ruc-backend` from the project folder. The data and the key are kept in `./data`.

## Repository layout

```
crates/tpt-ruc-core/   Pure Rust logic, no_std compatible
pwa/                   Progressive Web App (Phase 2)
hardware/              KiCad, BOM, enclosure (Phase 3 onward)
docs/                  User guides and NZTA compliance notes
DESIGN_SPEC.md         Design specification and locked decisions
```

## Building

Requires the stable Rust toolchain (pinned in `rust-toolchain.toml`).

```sh
cargo test --workspace

# no_std check for microcontroller and WASM targets
cargo check -p tpt-ruc-core --no-default-features --target thumbv7em-none-eabihf
```

## Licence

Dual licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
Copyright (c) 2026 TPT Solutions.

Every dependency must be MIT, BSD, ISC, Zlib, Unlicense/CC0 or dual MIT/Apache-2.0. This is enforced in CI by `cargo-deny` (see `deny.toml`).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). To report a security issue, see [SECURITY.md](SECURITY.md).
