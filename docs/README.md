# Documentation

Written for: everyone who uses, runs or works on tpt-ruc-nz.

Start with the [README](../README.md) for a quick start. Then pick the guide that matches what you want to do.

## Using the project

- [Web app (PWA) guide](guides/pwa.md): calculate RUC and record trips in a browser.
- [Self-hosting guide](guides/self-hosting.md): run the server with Docker, manage users, back up and upgrade.
- [HTTPS and backups](guides/reverse-proxy-and-backups.md): put the server behind a reverse proxy, and test a restore.
- [Frequently asked questions](guides/faq.md).
- [Privacy statement](privacy.md): what is stored, where, and what is logged.

## Building hardware

- [Bench GPS rig guide](guides/hardware-build.md): the breadboard test setup. Untested.
- [Hardware licence options](hardware-licence-options.md): choosing a licence for the design files. For the owner to decide.
- [Custom PCB build](pcb-build.md): blocked until the schematic exists.
- [Hardware overview](../hardware/README.md): chip choices, pin map and bill of materials.

## Mobile apps

- [eRUC export format](eruc-export.md): the generic export for eRUC providers, and the adapter trait. No provider adapter exists yet.
- [Mobile bindings](mobile-ffi.md): the small API for iOS and Android.
- [Native Bluetooth plan](mobile-ble-plan.md): blocked until devices and protocol bindings are ready.
- Samples: [iOS](../samples/ios/README.md) and [Android](../samples/android/README.md). Both are untested.

## NZTA and compliance

- [RUC rates: source and status](nzta/rates-research.md).
- [Rate table update procedure](nzta/rate-update-procedure.md): how to check and ship new rates.
- [Section 43 gap analysis](nzta/section43-gap-analysis.md): the current design against what approval needs. Criteria marked "needs NZTA source" have not been checked against NZTA's own text.
- [Accuracy test protocol](nzta/accuracy-test-protocol.md): how to test distance accuracy. A protocol, not results.
- [Section 43 decision memo](nzta/section43-decision-memo.md): options for whether to apply. For the owner to decide.

## Project

- [Bluetooth design](ble-design.md): the GATT layout for the device link.
- [Licensing and SPDX policy](LICENSING.md).
- [Releasing](RELEASING.md).
- [Design specification](../DESIGN_SPEC.md).
- [Contributing](../CONTRIBUTING.md) and [security policy](../SECURITY.md).

## Why there is no docs website

The project does not use mdBook to publish these pages. mdBook is licensed under MPL-2.0, and the project's licence policy excludes MPL-2.0 tooling. Read the Markdown on the repository page, or add a static site generator that passes the policy, which the owner should decide first.
