// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import { Backup } from "@/components/Backup";
import { Dashboard } from "@/components/Dashboard";
import { DeviceConnection } from "@/components/DeviceConnection";
import { DeviceSegments } from "@/components/DeviceSegments";
import { FleetView } from "@/components/FleetView";
import { RucCalculator } from "@/components/RucCalculator";
import { Section43Gate } from "@/components/Section43Gate";
import { ServiceWorkerRegister } from "@/components/ServiceWorkerRegister";
import { Settings } from "@/components/Settings";
import { StorageStatus } from "@/components/StorageStatus";
import { SyncPanel } from "@/components/SyncPanel";
import { Trips } from "@/components/Trips";
import { Vehicles } from "@/components/Vehicles";

/** The whole app: the page content, the footer disclaimer and the service worker. */
export function App() {
    return (
        <div className="min-h-dvh bg-neutral-50 text-neutral-900 antialiased">
            <main className="mx-auto max-w-2xl px-4 py-8">
                <Section43Gate>
                    <section className="space-y-8">
                        <h1 className="text-2xl font-semibold">tpt-ruc-nz</h1>
                        <p>
                            Track distance and estimate RUC liability offline. Enter trips by hand,
                            or connect the tracking device over Bluetooth and download the trips it
                            recorded. Export them as CSV.
                        </p>
                        <div className="space-y-3">
                            <h2 className="text-lg font-semibold">RUC calculator</h2>
                            <p className="text-sm text-neutral-600">
                                Uses the NZTA distance-licence rates in force today. Figures are
                                estimates. They are not a licence purchase.
                            </p>
                            <RucCalculator />
                        </div>
                        <Dashboard />
                        <Vehicles />
                        <DeviceConnection />
                        <Trips />
                        <StorageStatus />
                        <DeviceSegments />
                        <SyncPanel />
                        <FleetView />
                        <Backup />
                        <Settings />
                    </section>
                </Section43Gate>
            </main>
            <ServiceWorkerRegister />
            <footer className="mx-auto max-w-2xl px-4 pb-8 text-sm text-neutral-600">
                <p>
                    Not an NZTA-approved distance recorder. Claiming RUC deductions or using a
                    digital display in place of a licence label requires Section 43 approval from
                    NZTA first.
                </p>
            </footer>
        </div>
    );
}
