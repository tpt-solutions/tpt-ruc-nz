# iOS sample (untested)

Status: **untested.** The Swift file has not been compiled, and the glue code has not been generated on a Mac. Treat it as a starting point.

What is here: `RucExample.swift`, which calls `liability_incl_gst_nzd` and `billable_km` from the `tpt-ruc-ffi-ios` crate.

To try it:

1. On a Mac with Xcode, run `crates/tpt-ruc-ffi-ios/build-ios.sh`. The generation step in that script is still to be written, so this step does not work yet.
2. Add `TptRucFfi.xcframework` and the generated Swift file to an Xcode project.
3. Call `printExampleCharge()` from the app.

The functions throw a Swift error when the input is wrong, for example an unknown vehicle type or a date with no rate table.
