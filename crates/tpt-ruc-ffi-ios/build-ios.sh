#!/usr/bin/env sh
# SPDX-License-Identifier: MIT OR Apache-2.0
# Copyright (c) 2026 TPT Solutions
#
# Builds the iOS device and simulator libraries and packages them as an
# XCFramework that an Xcode project can link. The Swift and C glue comes from
# build.rs (swift-bridge), which writes it to target/ios/generated on each build.
#
# Needs a Mac with Xcode. CI runs it on a macOS runner (.github/workflows/mobile.yml).
set -eu

cd "$(dirname "$0")/../.."

rustup target add aarch64-apple-ios aarch64-apple-ios-sim

cargo build --release -p tpt-ruc-ffi-ios --target aarch64-apple-ios
cargo build --release -p tpt-ruc-ffi-ios --target aarch64-apple-ios-sim

generated=target/ios/generated
out=target/ios
headers="$out/headers"
rm -rf "$out/headers" "$out/swift" "$out/TptRucFfi.xcframework"
mkdir -p "$headers" "$out/swift"

# The C headers are in two folders, so flatten them into the one the XCFramework reads.
cp "$generated/SwiftBridgeCore.h" "$headers/"
cp "$generated/tpt-ruc-ffi-ios/tpt-ruc-ffi-ios.h" "$headers/"

# The Swift files are linked into the Xcode project, not into the XCFramework.
cp "$generated/SwiftBridgeCore.swift" "$out/swift/"
cp "$generated/tpt-ruc-ffi-ios/tpt-ruc-ffi-ios.swift" "$out/swift/"

xcodebuild -create-xcframework \
  -library target/aarch64-apple-ios/release/libtpt_ruc_ffi_ios.a -headers "$headers" \
  -library target/aarch64-apple-ios-sim/release/libtpt_ruc_ffi_ios.a -headers "$headers" \
  -output "$out/TptRucFfi.xcframework"
