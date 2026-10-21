// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Feeds arbitrary bytes to the JSON decoder for `Fix`. It must never panic.
//! Decoding must refuse bad coordinates rather than accept them.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tpt_ruc_core::Fix;

fuzz_target!(|data: &[u8]| {
    let _ = serde_json::from_slice::<Fix>(data);
});
