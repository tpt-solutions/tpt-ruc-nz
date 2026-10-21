// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions
//
// UNTESTED sample. It has not been compiled, and the native library has not
// been built for Android. See samples/android/README.md.
package com.tpt.ruc

object RucNative {
    init {
        // Loads libtpt_ruc_ffi_android.so from the app's jniLibs folder.
        System.loadLibrary("tpt_ruc_ffi_android")
    }

    /** GST-inclusive charge for one trip, in NZD. Throws RuntimeException on bad input. */
    external fun liabilityInclGstNzd(
        rateType: String,
        grossKg: Int,
        electric: Boolean,
        year: Int,
        month: Int,
        day: Int,
        metres: Double,
    ): Double

    /** Distance in whole kilometres, rounded the way RUC bills it. */
    external fun billableKm(metres: Double): Double
}

fun printExampleCharge() {
    try {
        // A 3,000 kg type 1 vehicle that drove 12.345 km on 1 October 2026.
        val charge = RucNative.liabilityInclGstNzd("1", 3_000, false, 2026, 10, 1, 12_345.0)
        println("Charge including GST: %.2f".format(charge))
        println("Billable kilometres: ${RucNative.billableKm(12_345.0)}")
    } catch (error: RuntimeException) {
        println("RUC error: ${error.message}")
    }
}
