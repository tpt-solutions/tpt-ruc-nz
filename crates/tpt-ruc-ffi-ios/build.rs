// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Generates the Swift and C glue for the bridge in `src/lib.rs`.
//!
//! The output goes to `target/ios/generated`, where `build-ios.sh` and the
//! Xcode sample look for it. It is written on every build, so it never goes stale.

use std::path::PathBuf;

fn main() {
    let bridge = "src/lib.rs";
    println!("cargo:rerun-if-changed={bridge}");

    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("set by cargo"));
    let out = manifest.join("../../target/ios/generated");
    std::fs::create_dir_all(&out).expect("can create the glue output folder");

    swift_bridge_build::parse_bridges(vec![bridge]).write_all_concatenated(&out, "tpt-ruc-ffi-ios");
}
