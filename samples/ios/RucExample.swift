// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions
//
// UNTESTED sample. It has not been compiled. It assumes the swift-bridge glue
// has been generated and linked as described in samples/ios/README.md.
import Foundation

func printExampleCharge() {
    do {
        // A 3,000 kg type 1 vehicle that drove 12.345 km on 1 October 2026.
        let charge = try liability_incl_gst_nzd("1", 3_000, false, 2026, 10, 1, 12_345)
        print(String(format: "Charge including GST: $%.2f", charge))

        let km = try billable_km(12_345)
        print("Billable kilometres: \(km)")
    } catch {
        print("RUC error: \(error)")
    }
}
