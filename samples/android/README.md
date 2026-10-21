# Android sample (untested)

Status: **untested.** The Kotlin file has not been compiled, and the native library has not been built for Android. Treat it as a starting point.

What is here: `RucExample.kt`, which declares the `external` functions that the `tpt-ruc-ffi-android` crate exports. The class must be `com.tpt.ruc.RucNative`, because the Rust function names encode that package and class.

To try it:

1. Install the Android NDK and `cargo-ndk`, then run `crates/tpt-ruc-ffi-android/build-android.sh`. That writes the `.so` files to `samples/android/app/src/main/jniLibs`.
2. Put `RucExample.kt` in a Kotlin app module and call `printExampleCharge()`.

Bad input throws a `RuntimeException` whose message gives the reason.
