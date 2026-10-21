import { Backup } from "@/components/Backup";
import { Dashboard } from "@/components/Dashboard";
import { DeviceConnection } from "@/components/DeviceConnection";
import { DeviceSegments } from "@/components/DeviceSegments";
import { FleetView } from "@/components/FleetView";
import { RucCalculator } from "@/components/RucCalculator";
import { Settings } from "@/components/Settings";
import { SyncPanel } from "@/components/SyncPanel";
import { StorageStatus } from "@/components/StorageStatus";
import { Section43Gate } from "@/components/Section43Gate";
import { Trips } from "@/components/Trips";
import { Vehicles } from "@/components/Vehicles";

export default function Home() {
    return (
        <Section43Gate>
            <section className="space-y-8">
                <h1 className="text-2xl font-semibold">tpt-ruc-nz</h1>
                <p>
                    Track distance and estimate RUC liability offline. Enter trips by hand, or
                    connect the tracking device over Bluetooth and download the trips it recorded.
                    Export them as CSV.
                </p>
                <div className="space-y-3">
                    <h2 className="text-lg font-semibold">RUC calculator</h2>
                    <p className="text-sm text-neutral-600">
                        Uses the NZTA distance-licence rates in force today. Figures are estimates.
                        They are not a licence purchase.
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
    );
}
