#!/usr/bin/env sh
# SPDX-License-Identifier: MIT OR Apache-2.0
# Copyright (c) 2026 TPT Solutions
#
# Builds the tpt-ruc-wasm package into pwa/wasm-pkg, where the PWA imports it.
# Requires wasm-pack (cargo install wasm-pack --locked).
set -eu

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

wasm-pack build crates/tpt-ruc-wasm \
  --release \
  --target web \
  --out-dir ../../pwa/wasm-pkg \
  --out-name tpt_ruc_wasm
