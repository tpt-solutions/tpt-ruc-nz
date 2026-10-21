# Changelog

All notable changes to `tpt-ruc-ffi-android` are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/). The workspace-wide history is in the [root changelog](../../CHANGELOG.md).

## [Unreleased]

### Added

- Kotlin bindings through the `jni` crate: `com.tpt.ruc.RucNative.liabilityInclGstNzd` and `billableKm`. A bad input throws a `RuntimeException` with the reason.
- `build-android.sh`, which builds the library for the common ABIs. The script is **untested**; it has not been run with the Android NDK.
- `samples/android/`, a Kotlin example. It is **untested** and has not been compiled.
