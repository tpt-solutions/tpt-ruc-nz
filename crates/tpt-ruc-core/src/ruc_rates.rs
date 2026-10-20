// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Official NZTA distance-licence rates, transcribed into a [`RateSchedule`].
//!
//! Source: NZTA, "RUC rates and transaction fees", as retrieved on 2026-10-21
//! (<https://www.nzta.govt.nz/vehicles/road-user-charges/ruc-rates-and-transaction-fees>).
//! Rates are dollars per 1,000 km, GST inclusive. Each value is stored in
//! micro-dollars, so `76` dollars is `76_000_000`.
//!
//! Covered: powered, unpowered, all-terrain crane, vintage, motor caravan,
//! overweight specialist and H-type distance licences.
//!
//! Not covered yet: additional licences for overweight permits, which are
//! priced per km by additional weight and need a different rule shape.
//!
//! Effective date: the page marks types 408, 430 and 433 as "From 6 August
//! 2026" and does not state an effective date for the rest. This table is
//! treated as effective from 2026-08-06. Trips dated earlier get
//! `RucError::NoRateTable` rather than a guessed rate, until the earlier
//! table is transcribed.

use crate::ruc::{split_gst, Liability, RateDate, RateSchedule, RateTable, RucError, Rule};

/// Dollars per 1,000 km, expressed in micro-dollars.
const fn dollars(value: u64) -> u64 {
    value * 1_000_000
}

const NZ_GST_BASIS_POINTS: u32 = 1500;

const RULES_2026_08_06: &[Rule] = &[
    // Powered vehicles, except H types.
    Rule {
        type_code: "1",
        min_gross_kg: 0,
        max_gross_kg: 3_500,
        rate_micros_per_1000km: dollars(76),
    },
    Rule {
        type_code: "1",
        min_gross_kg: 3_500,
        max_gross_kg: 6_000,
        rate_micros_per_1000km: dollars(82),
    },
    Rule {
        type_code: "1",
        min_gross_kg: 6_000,
        max_gross_kg: 9_000,
        rate_micros_per_1000km: dollars(167),
    },
    Rule {
        type_code: "1",
        min_gross_kg: 9_000,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(352),
    },
    Rule {
        type_code: "12",
        min_gross_kg: 1_000,
        max_gross_kg: 3_500,
        rate_micros_per_1000km: dollars(38),
    },
    Rule {
        type_code: "2",
        min_gross_kg: 0,
        max_gross_kg: 6_000,
        rate_micros_per_1000km: dollars(80),
    },
    Rule {
        type_code: "2",
        min_gross_kg: 6_000,
        max_gross_kg: 9_000,
        rate_micros_per_1000km: dollars(126),
    },
    Rule {
        type_code: "2",
        min_gross_kg: 9_000,
        max_gross_kg: 12_000,
        rate_micros_per_1000km: dollars(172),
    },
    Rule {
        type_code: "2",
        min_gross_kg: 12_000,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(315),
    },
    Rule {
        type_code: "311",
        min_gross_kg: 0,
        max_gross_kg: 18_000,
        rate_micros_per_1000km: dollars(336),
    },
    Rule {
        type_code: "311",
        min_gross_kg: 18_000,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(413),
    },
    Rule {
        type_code: "6",
        min_gross_kg: 0,
        max_gross_kg: 12_000,
        rate_micros_per_1000km: dollars(110),
    },
    Rule {
        type_code: "6",
        min_gross_kg: 12_000,
        max_gross_kg: 18_000,
        rate_micros_per_1000km: dollars(356),
    },
    Rule {
        type_code: "6",
        min_gross_kg: 18_000,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(434),
    },
    Rule {
        type_code: "308",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(431),
    },
    Rule {
        type_code: "408",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(401),
    },
    Rule {
        type_code: "309",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(346),
    },
    Rule {
        type_code: "430",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(287),
    },
    Rule {
        type_code: "433",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(359),
    },
    Rule {
        type_code: "14",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(435),
    },
    Rule {
        type_code: "19",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(389),
    },
    // Unpowered vehicles and trailers.
    Rule {
        type_code: "24",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(126),
    },
    Rule {
        type_code: "28",
        min_gross_kg: 0,
        max_gross_kg: 10_000,
        rate_micros_per_1000km: dollars(48),
    },
    Rule {
        type_code: "28",
        min_gross_kg: 10_000,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(311),
    },
    Rule {
        type_code: "29",
        min_gross_kg: 0,
        max_gross_kg: 10_000,
        rate_micros_per_1000km: dollars(41),
    },
    Rule {
        type_code: "29",
        min_gross_kg: 10_000,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(139),
    },
    Rule {
        type_code: "30",
        min_gross_kg: 0,
        max_gross_kg: 10_000,
        rate_micros_per_1000km: dollars(41),
    },
    Rule {
        type_code: "30",
        min_gross_kg: 10_000,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(234),
    },
    Rule {
        type_code: "33",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(186),
    },
    Rule {
        type_code: "37",
        min_gross_kg: 0,
        max_gross_kg: 10_000,
        rate_micros_per_1000km: dollars(44),
    },
    Rule {
        type_code: "37",
        min_gross_kg: 10_000,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(318),
    },
    Rule {
        type_code: "43",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(238),
    },
    Rule {
        type_code: "951",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(179),
    },
    Rule {
        type_code: "929",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(101),
    },
    Rule {
        type_code: "939",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(67),
    },
    // All-terrain cranes.
    Rule {
        type_code: "299",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(279),
    },
    Rule {
        type_code: "399",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(392),
    },
    Rule {
        type_code: "499",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(364),
    },
    Rule {
        type_code: "599",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(320),
    },
    // Vintage powered vehicles (over 40 years old, not commercial).
    Rule {
        type_code: "402",
        min_gross_kg: 12_000,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(222),
    },
    Rule {
        type_code: "403",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(195),
    },
    Rule {
        type_code: "404",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(200),
    },
    // Motor caravans.
    Rule {
        type_code: "413",
        min_gross_kg: 18_000,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(314),
    },
    Rule {
        type_code: "414",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(273),
    },
    // Overweight specialist vehicles, by permit weight.
    Rule {
        type_code: "660",
        min_gross_kg: 0,
        max_gross_kg: 16_000,
        rate_micros_per_1000km: dollars(552),
    },
    Rule {
        type_code: "661",
        min_gross_kg: 16_000,
        max_gross_kg: 18_000,
        rate_micros_per_1000km: dollars(837),
    },
    Rule {
        type_code: "662",
        min_gross_kg: 0,
        max_gross_kg: 23_000,
        rate_micros_per_1000km: dollars(541),
    },
    Rule {
        type_code: "663",
        min_gross_kg: 23_000,
        max_gross_kg: 25_000,
        rate_micros_per_1000km: dollars(775),
    },
    Rule {
        type_code: "664",
        min_gross_kg: 0,
        max_gross_kg: 27_000,
        rate_micros_per_1000km: dollars(616),
    },
    Rule {
        type_code: "665",
        min_gross_kg: 27_000,
        max_gross_kg: 29_000,
        rate_micros_per_1000km: dollars(782),
    },
    Rule {
        type_code: "666",
        min_gross_kg: 18_000,
        max_gross_kg: 20_000,
        rate_micros_per_1000km: dollars(1_087),
    },
    // H-type overweight combinations, which replace standard powered licences.
    Rule {
        type_code: "H01",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(541),
    },
    Rule {
        type_code: "H11",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(356),
    },
    Rule {
        type_code: "H12",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(525),
    },
    Rule {
        type_code: "H13",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(650),
    },
    Rule {
        type_code: "H14",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(198),
    },
    Rule {
        type_code: "H15",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(369),
    },
    Rule {
        type_code: "H17",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(117),
    },
    Rule {
        type_code: "H18",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(234),
    },
    Rule {
        type_code: "H19",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(317),
    },
    Rule {
        type_code: "H31",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(424),
    },
    Rule {
        type_code: "H32",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(587),
    },
    Rule {
        type_code: "H34",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(497),
    },
    Rule {
        type_code: "H35",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(658),
    },
    Rule {
        type_code: "H36",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(430),
    },
    Rule {
        type_code: "H37",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(599),
    },
    Rule {
        type_code: "H38",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(724),
    },
    Rule {
        type_code: "H61",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(663),
    },
    Rule {
        type_code: "H62",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(801),
    },
    Rule {
        type_code: "H63",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(842),
    },
    Rule {
        type_code: "H71",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(658),
    },
    Rule {
        type_code: "H72",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(659),
    },
    Rule {
        type_code: "H73",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(711),
    },
    Rule {
        type_code: "H74",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(937),
    },
    Rule {
        type_code: "H75",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(500),
    },
    Rule {
        type_code: "H76",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(691),
    },
    Rule {
        type_code: "H77",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(711),
    },
    Rule {
        type_code: "H78",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(712),
    },
    Rule {
        type_code: "H79",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(938),
    },
    Rule {
        type_code: "H81",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(455),
    },
    Rule {
        type_code: "H82",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(657),
    },
    Rule {
        type_code: "H83",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(942),
    },
    Rule {
        type_code: "H84",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(472),
    },
    Rule {
        type_code: "H85",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(662),
    },
    Rule {
        type_code: "H92",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(470),
    },
    Rule {
        type_code: "H93",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(632),
    },
    Rule {
        type_code: "H95",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(525),
    },
    Rule {
        type_code: "H96",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(688),
    },
    Rule {
        type_code: "H98",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(453),
    },
    Rule {
        type_code: "H99",
        min_gross_kg: 0,
        max_gross_kg: u32::MAX,
        rate_micros_per_1000km: dollars(614),
    },
];

/// NZTA distance-licence rates, from 2026-08-06, retrieved 2026-10-21.
pub const NZTA_2026_08_06: RateTable = RateTable {
    version: "nzta-ruc-rates-retrieved-2026-10-21",
    effective: RateDate {
        year: 2026,
        month: 8,
        day: 6,
    },
    rules: RULES_2026_08_06,
    gst_basis_points: NZ_GST_BASIS_POINTS,
};

/// Additional licence for an overweight permit. Priced per km, not per 1,000 km.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdditionalLicence {
    /// Vehicle type code. Combinations are keyed `comb5` to `comb11` by axle count.
    pub type_code: &'static str,
    /// Cents per km for extra weight of 1,000, 2,000 ... 20,000 kg. Entry `i` covers `(i*1000, (i+1)*1000]`.
    pub cents_per_km: [u32; 20],
    /// Cents per km for each further started 1,000 kg above 20,000 kg.
    pub cents_per_1000kg_over_20000: u32,
}

/// Additional licences, from the NZTA page. Columns are read straight across each row.
pub const ADDITIONAL_LICENCES: &[AdditionalLicence] = &[
    AdditionalLicence {
        type_code: "1",
        cents_per_km: [
            56, 83, 117, 157, 205, 263, 332, 413, 506, 613, 738, 877, 1035, 1215, 1417, 1641, 1892,
            2169, 2477, 2815,
        ],
        cents_per_1000kg_over_20000: 369,
    },
    AdditionalLicence {
        type_code: "2",
        cents_per_km: [
            83, 112, 144, 184, 231, 284, 346, 418, 499, 591, 695, 811, 941, 1087, 1246, 1423, 1617,
            1833, 2067, 2324,
        ],
        cents_per_1000kg_over_20000: 279,
    },
    AdditionalLicence {
        type_code: "6",
        cents_per_km: [
            52, 67, 86, 106, 131, 157, 185, 218, 256, 296, 340, 388, 441, 498, 561, 630, 704, 786,
            871, 965,
        ],
        cents_per_1000kg_over_20000: 101,
    },
    AdditionalLicence {
        type_code: "14",
        cents_per_km: [
            74, 90, 107, 126, 148, 173, 200, 227, 259, 295, 330, 371, 415, 461, 512, 565, 625, 687,
            754, 826,
        ],
        cents_per_1000kg_over_20000: 77,
    },
    AdditionalLicence {
        type_code: "19",
        cents_per_km: [
            47, 57, 68, 80, 94, 107, 123, 140, 159, 179, 200, 223, 247, 275, 302, 332, 365, 399,
            437, 476,
        ],
        cents_per_1000kg_over_20000: 41,
    },
    AdditionalLicence {
        type_code: "299",
        cents_per_km: [
            19, 29, 43, 60, 78, 99, 124, 155, 187, 225, 269, 318, 371, 433, 500, 575, 658, 749,
            849, 958,
        ],
        cents_per_1000kg_over_20000: 120,
    },
    AdditionalLicence {
        type_code: "399",
        cents_per_km: [
            22, 32, 43, 56, 72, 87, 105, 125, 147, 173, 200, 230, 262, 298, 337, 379, 424, 473,
            528, 584,
        ],
        cents_per_1000kg_over_20000: 63,
    },
    AdditionalLicence {
        type_code: "499",
        cents_per_km: [
            40, 49, 61, 72, 84, 98, 114, 130, 147, 166, 187, 211, 236, 261, 289, 320, 352, 386,
            423, 463,
        ],
        cents_per_1000kg_over_20000: 41,
    },
    AdditionalLicence {
        type_code: "599",
        cents_per_km: [
            60, 68, 79, 90, 101, 115, 128, 142, 158, 176, 193, 211, 231, 254, 276, 300, 325, 352,
            381, 412,
        ],
        cents_per_1000kg_over_20000: 32,
    },
    AdditionalLicence {
        type_code: "699",
        cents_per_km: [
            84, 94, 103, 116, 126, 140, 154, 166, 182, 197, 215, 232, 251, 271, 290, 312, 336, 359,
            385, 412,
        ],
        cents_per_1000kg_over_20000: 27,
    },
    AdditionalLicence {
        type_code: "799",
        cents_per_km: [
            99, 108, 119, 130, 140, 154, 164, 178, 191, 206, 222, 238, 255, 272, 290, 309, 330,
            351, 373, 396,
        ],
        cents_per_1000kg_over_20000: 24,
    },
    AdditionalLicence {
        type_code: "comb5",
        cents_per_km: [
            118, 118, 118, 176, 176, 176, 219, 219, 272, 300, 331, 363, 398, 435, 474, 516, 560,
            605, 654, 707,
        ],
        cents_per_1000kg_over_20000: 55,
    },
    AdditionalLicence {
        type_code: "comb6",
        cents_per_km: [
            65, 65, 65, 88, 88, 130, 130, 130, 130, 180, 199, 219, 240, 262, 286, 312, 338, 366,
            396, 426,
        ],
        cents_per_1000kg_over_20000: 33,
    },
    AdditionalLicence {
        type_code: "comb7",
        cents_per_km: [
            43, 43, 43, 72, 72, 72, 72, 106, 106, 147, 147, 147, 180, 198, 216, 235, 256, 277, 299,
            323,
        ],
        cents_per_1000kg_over_20000: 24,
    },
    AdditionalLicence {
        type_code: "comb8",
        cents_per_km: [
            37, 37, 37, 37, 79, 79, 79, 79, 79, 136, 136, 136, 136, 136, 178, 193, 208, 225, 243,
            262,
        ],
        cents_per_1000kg_over_20000: 20,
    },
    AdditionalLicence {
        type_code: "comb9",
        cents_per_km: [
            2, 2, 2, 2, 2, 2, 22, 22, 22, 22, 47, 47, 47, 47, 69, 79, 87, 97, 106, 117,
        ],
        cents_per_1000kg_over_20000: 12,
    },
    AdditionalLicence {
        type_code: "comb10",
        cents_per_km: [
            3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 33, 33, 33, 33, 33, 33, 55, 55, 55,
        ],
        cents_per_1000kg_over_20000: 7,
    },
    AdditionalLicence {
        type_code: "comb11",
        cents_per_km: [
            4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 19, 19, 19, 23,
        ],
        cents_per_1000kg_over_20000: 5,
    },
];

/// Charges an additional overweight licence for `additional_kg` of extra weight
/// over `metres` of travel, on `date`.
///
/// Assumptions, not stated on the NZTA page: any part of 1,000 kg counts as a
/// full 1,000 kg step, and the weight is matched to the smallest row at or above
/// it. Additional licences use the same effective date as the standard table.
pub fn additional_liability(
    type_code: &'static str,
    additional_kg: u32,
    date: RateDate,
    metres: f64,
) -> Result<Liability, RucError> {
    if metres.is_nan() || metres < 0.0 {
        return Err(RucError::InvalidDistance);
    }
    if date < NZTA_2026_08_06.effective {
        return Err(RucError::NoRateTable);
    }
    let licence = ADDITIONAL_LICENCES
        .iter()
        .find(|l| l.type_code == type_code)
        .ok_or(RucError::NoMatchingRule)?;

    let billed_metres = libm::round(metres) as u64;
    let cents_per_km = if additional_kg == 0 {
        0
    } else if additional_kg <= 20_000 {
        licence.cents_per_km[((additional_kg - 1) / 1_000) as usize]
    } else {
        let steps = (additional_kg - 20_000).div_ceil(1_000);
        licence.cents_per_km[19] + steps * licence.cents_per_1000kg_over_20000
    };

    // Micro-dollars = metres * cents per km * 10_000 / 1_000 = metres * cents * 10.
    let incl = billed_metres
        .checked_mul(u64::from(cents_per_km) * 10)
        .ok_or(RucError::Overflow)?;
    let (excl, gst) = split_gst(incl, NZTA_2026_08_06.gst_basis_points)?;
    Ok(Liability {
        rate_version: NZTA_2026_08_06.version,
        billed_metres,
        incl_gst_micros: incl,
        excl_gst_micros: excl,
        gst_micros: gst,
    })
}

/// How a distance licence was bought. The fee depends on the channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PurchaseMethod {
    /// Online, or through the NZTA app.
    Online,
    /// Counter sales at an agent.
    Agent,
}

/// Administration fee per licence, including GST, in micro-dollars.
pub const fn admin_fee_incl_gst_micros(method: PurchaseMethod) -> u64 {
    match method {
        PurchaseMethod::Online => 12_440_000,
        PurchaseMethod::Agent => 13_710_000,
    }
}

/// The schedule of all NZTA rate tables this crate ships.
pub const NZTA_SCHEDULE: RateSchedule = &[NZTA_2026_08_06];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ruc::{liability, rule_for, split_gst, RucError, Vehicle};

    #[test]
    fn admin_fees_match_the_page() {
        assert_eq!(
            admin_fee_incl_gst_micros(PurchaseMethod::Online),
            12_440_000
        );
        assert_eq!(admin_fee_incl_gst_micros(PurchaseMethod::Agent), 13_710_000);
    }

    #[test]
    fn online_fee_splits_into_gst_exclusive_and_gst() {
        // $12.44 incl GST is $10.82 excl GST plus $1.62 GST.
        let (excl, gst) =
            split_gst(admin_fee_incl_gst_micros(PurchaseMethod::Online), 1500).unwrap();
        assert_eq!(excl, 10_817_391);
        assert_eq!(gst, 1_622_609);
        assert_eq!(excl + gst, 12_440_000);
    }

    fn on(year: u16, month: u8, day: u8) -> RateDate {
        RateDate { year, month, day }
    }

    fn charge(rate_type: &'static str, gross_kg: u32) -> Result<u64, RucError> {
        // 1,000 km, so the charge in micro-dollars equals the published rate.
        let v = Vehicle {
            rate_type,
            gross_kg,
            electric: false,
        };
        liability(NZTA_SCHEDULE, &v, on(2026, 10, 21), 1_000_000.0).map(|l| l.incl_gst_micros)
    }

    #[test]
    fn light_diesel_matches_published_rate() {
        assert_eq!(charge("1", 3_500), Ok(dollars(76)));
    }

    #[test]
    fn weight_bands_for_type_1_match_the_page() {
        assert_eq!(charge("1", 3_501), Ok(dollars(82)));
        assert_eq!(charge("1", 6_000), Ok(dollars(82)));
        assert_eq!(charge("1", 6_001), Ok(dollars(167)));
        assert_eq!(charge("1", 9_000), Ok(dollars(167)));
        assert_eq!(charge("1", 9_001), Ok(dollars(352)));
    }

    #[test]
    fn plug_in_hybrid_has_its_own_rate() {
        assert_eq!(charge("12", 1_001), Ok(dollars(38)));
        assert_eq!(charge("12", 3_500), Ok(dollars(38)));
        // Type 12 starts at 1,001 kg, so a lighter weight has no rule.
        assert_eq!(charge("12", 1_000), Err(RucError::NoMatchingRule));
    }

    #[test]
    fn vintage_needs_weight_over_12000() {
        assert_eq!(charge("402", 12_001), Ok(dollars(222)));
        assert_eq!(charge("402", 12_000), Err(RucError::NoMatchingRule));
    }

    #[test]
    fn overweight_specialist_bands_are_split_by_permit_weight() {
        assert_eq!(charge("661", 17_000), Ok(dollars(837)));
        assert_eq!(charge("661", 16_000), Err(RucError::NoMatchingRule));
        assert_eq!(charge("666", 19_000), Ok(dollars(1_087)));
    }

    #[test]
    fn type_408_is_only_in_force_from_6_august_2026() {
        let v = Vehicle {
            rate_type: "408",
            gross_kg: 40_000,
            electric: false,
        };
        assert_eq!(
            liability(NZTA_SCHEDULE, &v, on(2026, 8, 5), 1.0),
            Err(RucError::NoRateTable)
        );
        assert!(liability(NZTA_SCHEDULE, &v, on(2026, 8, 6), 1.0).is_ok());
    }

    #[test]
    fn unpowered_and_crane_types_are_present() {
        assert_eq!(charge("939", 5_000), Ok(dollars(67)));
        assert_eq!(charge("599", 30_000), Ok(dollars(320)));
        // Cranes 699 and 799 have no published rate on the page.
        assert_eq!(charge("699", 30_000), Err(RucError::NoMatchingRule));
    }

    #[test]
    fn electric_light_vehicle_is_charged_at_type_1_rate_after_start_date() {
        let v = Vehicle {
            rate_type: "1",
            gross_kg: 2_000,
            electric: true,
        };
        let l = liability(NZTA_SCHEDULE, &v, on(2026, 10, 21), 1_000_000.0).unwrap();
        assert_eq!(l.incl_gst_micros, dollars(76));
    }

    #[test]
    fn schedule_is_sorted_by_effective_date() {
        for pair in NZTA_SCHEDULE.windows(2) {
            assert!(pair[0].effective < pair[1].effective);
        }
    }

    #[test]
    fn weight_bands_are_contiguous_for_every_type() {
        // Each band must start where the previous one for the same type ends.
        for (i, rule) in RULES_2026_08_06.iter().enumerate() {
            assert!(
                rule.min_gross_kg < rule.max_gross_kg,
                "empty band: {rule:?}"
            );
            if let Some(next) = RULES_2026_08_06[i + 1..]
                .iter()
                .find(|r| r.type_code == rule.type_code)
            {
                if rule.max_gross_kg != u32::MAX {
                    assert_eq!(next.min_gross_kg, rule.max_gross_kg, "gap after {rule:?}");
                }
            }
        }
    }

    #[test]
    fn every_rule_is_reachable_through_rule_for() {
        let table = &NZTA_2026_08_06;
        for rule in table.rules {
            let probe = Vehicle {
                electric: false,
                rate_type: rule.type_code,
                gross_kg: rule.max_gross_kg.min(rule.min_gross_kg.saturating_add(1)),
            };
            assert_eq!(rule_for(table, &probe), Ok(rule));
        }
    }
}

#[cfg(test)]
mod additional_tests {
    use super::*;
    use crate::ruc::RucError;

    fn on(year: u16, month: u8, day: u8) -> RateDate {
        RateDate { year, month, day }
    }

    /// Cents per km charged for `kg` of extra weight, using 1 km of travel.
    fn cents_per_km(type_code: &'static str, kg: u32) -> u64 {
        let l = additional_liability(type_code, kg, on(2026, 10, 21), 1_000.0).unwrap();
        // 1 km at c cents per km is c * 10_000 micro-dollars.
        l.incl_gst_micros / 10_000
    }

    #[test]
    fn every_type_has_twenty_bands_and_a_positive_first_band() {
        for licence in ADDITIONAL_LICENCES {
            assert!(licence.cents_per_km[0] > 0, "{}", licence.type_code);
        }
    }

    #[test]
    fn first_and_last_rows_match_the_page() {
        assert_eq!(cents_per_km("1", 1_000), 56);
        assert_eq!(cents_per_km("1", 20_000), 2_815);
        assert_eq!(cents_per_km("comb11", 20_000), 23);
        assert_eq!(cents_per_km("799", 1_000), 99);
    }

    #[test]
    fn part_of_a_thousand_kg_uses_the_next_row() {
        // 1,500 kg falls in the 2,000 kg row.
        assert_eq!(cents_per_km("1", 1_500), 83);
        assert_eq!(cents_per_km("1", 2_000), 83);
        assert_eq!(cents_per_km("1", 2_001), 117);
    }

    #[test]
    fn weight_over_20000_adds_an_increment_per_started_1000kg() {
        // Type 1: 2,815 at 20,000 kg, plus 369 per started 1,000 kg.
        assert_eq!(cents_per_km("1", 21_000), 2_815 + 369);
        assert_eq!(cents_per_km("1", 21_001), 2_815 + 2 * 369);
    }

    #[test]
    fn zero_additional_weight_charges_nothing() {
        assert_eq!(cents_per_km("1", 0), 0);
    }

    #[test]
    fn charge_is_exact_for_whole_metres() {
        // 2,000 kg on type 6 for 10 km: 10,000 m * 67 cents * 10 = 6,700,000 micros.
        let l = additional_liability("6", 2_000, on(2026, 10, 21), 10_000.0).unwrap();
        assert_eq!(l.incl_gst_micros, 6_700_000);
        assert_eq!(l.excl_gst_micros + l.gst_micros, l.incl_gst_micros);
    }

    #[test]
    fn unknown_type_and_dates_before_table_are_errors() {
        assert_eq!(
            additional_liability("999", 1_000, on(2026, 10, 21), 1.0),
            Err(RucError::NoMatchingRule)
        );
        assert_eq!(
            additional_liability("1", 1_000, on(2026, 8, 5), 1.0),
            Err(RucError::NoRateTable)
        );
    }
}
