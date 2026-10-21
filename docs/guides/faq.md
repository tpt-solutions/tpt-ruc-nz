# Frequently asked questions

Written for: anyone using or setting up tpt-ruc-nz.

## Is this approved by NZTA?

No. tpt-ruc-nz is an open-source reference tool for tracking and estimating RUC. It is not an NZTA Approved Distance Recorder. If you want to claim RUC deductions, for example for off-road travel, or use a digital display in place of a licence label, you must apply to NZTA for Section 43 approval first.

## Can I use the charges it shows to pay my RUC?

Use them as an estimate, and check them against the NZTA figures. Rates come from the NZTA rates page, which was retrieved on 21 October 2026. Rates change, and the project's rate table is updated by hand. Check the figure before you rely on it.

## Does it send my data anywhere?

No. The web app keeps everything in your browser, and makes no network requests. The self-hosted server stores its data on your own computer. TPT Solutions does not receive any of it. See [privacy](../privacy.md) for the details.

## Do I need an account?

The web app does not need one. The self-hosted server needs an API key, which it creates for you the first time it starts.

## Where is my API key?

It is in the server's log the first time it starts, and it is saved in the data folder as `api-key`. Anyone with that key can read and change your data, so keep it private. Run `docker compose logs tpt-ruc` to see it again.

## I lost my API key. What now?

Your key is stored in the data volume. If you have a backup, restore it. Otherwise, stop the server, delete the `api-key` file from the data folder, and start the server again. It will create a new key. Users you created have their own tokens, which are not affected.

## Can I have more than one driver?

Yes. Create a user for each driver with the owner key. Each driver uses their own token. Drivers see only their own trips. See the [self-hosting guide](self-hosting.md#users-and-roles).

## Does the web app work offline?

Yes, after the first load. It keeps working without a connection, and you can install it on your device.

## Why can't the web app talk to the server?

Sync is not built yet. The server works, and you can call it directly, but the web app does not send data to it. This is the next big piece of work.

## Can I use it with a GPS device?

Not over Bluetooth yet. The firmware and the Bluetooth link are still being built. You can import trip segments recorded by a device from a file.

## Which vehicles are supported?

Any vehicle with an NZTA RUC vehicle type code and a known weight. The rate table covers the vehicle types on NZTA's rates page. Trailers are licensed separately, so they have no charge of their own in the table. An unknown type code gives an error, not a guessed charge.

## Why does a trip before August 2026 give an error instead of a charge?

The rate table in the project covers rates from 6 August 2026. Earlier rates are not in the table yet, so the calculator returns an error for those dates rather than guessing. Check the date you entered.

## Electric vehicles: do I pay?

The project models electric vehicles from 1 April 2024, which is the date the maintainer gave. Check this against NZTA before you rely on it.

## Is my trip data safe if the computer breaks?

Only if you have a backup. Make a backup of the web app's data from the Backup section, and back up the Docker volume if you run the server. Store the backups somewhere else.

## How do I report a bug or a security problem?

Report ordinary bugs on the project's issue tracker. For a security problem, follow [SECURITY.md](../../SECURITY.md) and do not post it publicly.

## Where is the design?

See [DESIGN_SPEC.md](../../DESIGN_SPEC.md). The spec has some details that are out of date, including its example Docker setup. The self-hosting guide describes the setup that works today.
