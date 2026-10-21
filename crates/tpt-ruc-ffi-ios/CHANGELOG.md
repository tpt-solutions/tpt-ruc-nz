# Changelog

All notable changes to `tpt-ruc-ffi-ios` are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/). The workspace-wide history is in the [root changelog](../../CHANGELOG.md).

## [Unreleased]

### Added

- Swift bindings through swift-bridge: `liability_incl_gst_nzd` and `billable_km`, both throwing on bad input.
- `build-ios.sh`, which packages the libraries as an XCFramework. The script is **untested**; it has not been run on a Mac.
- `samples/ios/`, a Swift example. It is **untested** and has not been compiled.
