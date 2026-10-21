# tpt-ruc-nz web app

The browser app: trips, the RUC calculator, the device connection and uploads to the backend. It runs offline once loaded.

## Develop

```sh
npm install
npm run dev        # http://localhost:3000
npm test           # unit tests (Vitest)
npm run test:e2e   # builds the static site and runs the browser tests (Playwright)
```

The unit tests include a sync test against a real backend. Build the backend first with `cargo build -p tpt-ruc-backend` from the repository root, or that test is skipped.

## SUPPORTED_PLATFORMS

| Platform              | Browser                       | Trips by hand | Device over Bluetooth            | Upload to backend |
| --------------------- | ----------------------------- | ------------- | -------------------------------- | ----------------- |
| Windows, macOS, Linux | Chrome, Edge, Opera (current) | Yes           | Yes                              | Yes               |
| Android               | Chrome, Edge (current)        | Yes           | Yes                              | Yes               |
| iPhone and iPad       | Safari, Chrome, any browser   | Yes           | No, see below                    | Yes               |
| Any                   | Firefox                       | Yes           | No, Firefox has no Web Bluetooth | Yes               |

- **Bluetooth needs HTTPS or `localhost`.** The app says so when it is served any other way.
- **iPhone and iPad have no Web Bluetooth.** Every browser on iOS uses Apple's engine, so this is not a choice of browser. Use the native app when it exists. Until then, record trips on the device and import the segments file on the device-segments screen.
- **Firefox has no Web Bluetooth.** Use Chrome, Edge or Opera for the device, or import the segments file.
- **Signing keys need Ed25519 in WebCrypto.** Browsers without it still upload, but the uploads are not signed. The sync screen says so.
- The app is tested with Chromium in Playwright. Other browsers, and the device itself, have not been tested in this project.

## Layout

- `app/` the single page and the service worker registration
- `components/` the screens
- `lib/` storage, the RUC wasm calls, the BLE protocol and connection, the sync engine and its IndexedDB store
- `e2e/` browser tests
- `wasm-pkg/` the WebAssembly build of `crates/tpt-ruc-wasm`, made by `scripts/build-wasm.sh`
