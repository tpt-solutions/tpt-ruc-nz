// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Core logic for tpt-ruc-nz: NMEA parsing, coordinates and distance maths.
//!
//! The crate is `no_std` when the `std` feature is disabled. The `std` feature
//! is on by default for host use; build with `--no-default-features` for
//! microcontroller and WASM targets.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

pub mod coord;
pub mod distance;
pub mod nmea;

pub use coord::{Coordinate, CoordinateError, Latitude, Longitude};
pub use distance::{haversine_m, EARTH_RADIUS_M};
pub use nmea::{parse_sentence, FixQuality, ParseError, Sentence};
