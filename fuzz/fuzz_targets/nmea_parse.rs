// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Feeds arbitrary bytes to the NMEA parser. It must never panic. Errors are expected.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tpt_ruc_core::parse_sentence;

fuzz_target!(|data: &[u8]| {
    // Receivers send text, so non-UTF-8 input is skipped rather than decoded lossily.
    if let Ok(text) = std::str::from_utf8(data) {
        for line in text.lines() {
            let _ = parse_sentence(line);
        }
    }
});
