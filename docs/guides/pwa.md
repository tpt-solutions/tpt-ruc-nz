# Using the web app (PWA)

Written for: anyone who wants to calculate RUC, or record trips in a browser.

The web app is a single page you can open in a browser. It works offline after the first visit, and you can install it on a phone or computer.

> **Important.** This app is not an NZTA-approved distance recorder. Claiming RUC deductions, or using a display in place of a licence label, needs NZTA approval under Section 43. Before you use the app for official licensing or tax, apply to NZTA. The app shows this notice the first time you open it.

## Before you start

The web app is built from the project's `pwa/` folder. Someone needs to build it once. You need [Node.js](https://nodejs.org) 22 or later.

```sh
cd pwa
npm ci
npm run build
```

The build writes the finished app to `pwa/out/`. You can serve that folder with any static file server. For example, from `pwa/out`:

```sh
python -m http.server 3000
```

Then open `http://localhost:3000` in your browser.

## What the app does

- **Section 43 notice.** The first time you open the app, you must read the notice and tick that you understand it. This is kept in the browser.
- **RUC calculator.** Enter a distance and a vehicle, and the app works out the RUC charge for that distance. It uses the rates in the project's rate table.
- **Vehicles.** Add your vehicles with their type code and weight. You can also record the odometer reading.
- **Trips.** Add trips by hand, with a date, distance and vehicle. Each trip shows its estimated charge for the date it happened. You can export trips as a CSV file.
- **Device segments.** Import trip segments recorded by a GPS device, from a file. Segments are matched to vehicles.
- **Backup.** Download all your data as one file, and restore from that file later. Keep the file private, because it holds your trips.
- **Storage.** The app shows how much browser storage it uses, and warns you when it is nearly full.

## Where your data lives

Everything stays in your browser, in the browser's own storage (IndexedDB). The app makes no network requests. It does not send trips or vehicles anywhere. Clearing your browser's site data deletes the app's data, so make a backup first.

## Install it on your device

Installing adds the app to your home screen or applications list, so it opens like a normal app.

- **Chrome or Edge (Windows, Mac, Android):** open the app, then choose the install icon in the address bar, or the menu entry **Install app**.
- **Safari (iPhone and iPad):** open the app, tap **Share**, then **Add to Home Screen**.

Installing needs the app to be served over HTTPS, or from `localhost`. A browser will not install an app from a plain `http://` address on another computer.

## Limits to know about

- **No connection to the server yet.** The app does not send data to the self-hosted backend. Sync is planned.
- **No GPS from the browser.** The app does not read your location. Trips are entered by hand or imported from a device file.
- **Bluetooth is not connected yet.** The app cannot connect to a GPS device over Bluetooth yet. Chrome and Edge on desktop support Bluetooth in principle, but this app's connection code is not written. Safari and Firefox do not support Web Bluetooth at all.
- **Rates are from the NZTA page retrieved on 21 October 2026.** Check them against [nzta.govt.nz](https://www.nzta.govt.nz) before you rely on a figure.

## If the app does not work offline

The app keeps itself available offline after the first successful load. If you updated the app, close every tab of it and open it again, so the browser loads the new version.
