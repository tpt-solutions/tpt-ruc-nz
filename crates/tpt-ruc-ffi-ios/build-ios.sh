#!/usr/bin/env sh
# SPDX-License-Identifier: MIT OR Apache-2.0
# Copyright (c) 2026 TPT Solutions
#
# UNTESTED. This script has not been run. It needs a Mac with Xcode, and the
# steps below are written from the swift-bridge and Apple documentation.
#
# Builds the iOS device and simulator libraries and packages them as an
# XCFramework that an Xcode project can link.
set -eu

cd "$(dirname "$0")/../.."

rustup target add aarch64-apple-ios aarch64-apple-ios-sim

cargo build --release -p tpt-ruc-ffi-ios --target aarch64-apple-ios
cargo build --release -p tpt-ruc-ffi-ios --target aarch64-apple-ios-sim

# Step still to write: generate the Swift and C glue with swift-bridge-build,
# and place the output in target/ios/generated. See samples/ios/README.md.

out=target/ios
rm -rf "$out"
mkdir -p "$out/headers"
cp -r "$out/generated/headers/." "$out/headers/" 2>/dev/null || true

xcodebuild -create-xcframework \
  -library target/aarch64-apple-ios/release/libtpt_ruc_ffi_ios.a -headers "$out/headers" \
  -library target/aarch64-apple-ios-sim/release/libtpt_ruc_ffi_ios.a -headers "$out/headers" \
  -output "$out/TptRucFfi.xcframework"
