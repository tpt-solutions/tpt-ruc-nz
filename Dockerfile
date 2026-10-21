# SPDX-License-Identifier: MIT OR Apache-2.0
# Copyright (c) 2026 TPT Solutions

# tpt-ruc-nz backend image. Build and run it with `docker compose up -d`.
#
# Two stages: the first compiles the server, the second copies only the binary
# into a small Debian image. The server needs no other system libraries, since
# SQLite is compiled in.

FROM rust:1-bookworm AS build
WORKDIR /src

# The image uses the base image's stable toolchain. rust-toolchain.toml is not
# copied, because it pins cross-compilation targets the server does not need.
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY firmware ./firmware
RUN cargo build --release --locked -p tpt-ruc-backend

FROM debian:bookworm-slim

# Runs as a fixed, unprivileged user. Its home is the data volume.
RUN groupadd --system --gid 10001 tpt \
    && useradd --system --uid 10001 --gid tpt --home-dir /data --no-create-home tpt \
    && mkdir /data \
    && chown tpt:tpt /data

COPY --from=build /src/target/release/tpt-ruc-backend /usr/local/bin/tpt-ruc-backend

# Listen on every interface inside the container. Compose decides what the host exposes.
ENV TPT_RUC_BIND=0.0.0.0:8080 \
    TPT_RUC_DATABASE_URL=sqlite:///data/tpt-ruc.db \
    RUST_LOG=info

VOLUME ["/data"]
USER tpt
EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["tpt-ruc-backend", "healthcheck"]

ENTRYPOINT ["tpt-ruc-backend"]
