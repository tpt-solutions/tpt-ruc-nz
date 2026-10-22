import { useEffect, useState } from "react";
import { listVehicles, type Trip, type Vehicle } from "@/lib/db";
import { listTrips } from "@/lib/trips";
import { onVehiclesChanged } from "@/lib/events";
import { formatMicros } from "@/lib/money";
import { daysUntil, totalCharges, tripsThisMonth, type ChargeTotals } from "@/lib/dashboard";
import { estimateTrip, heavyLicenceExpiry, licenceRemainingKm } from "@/lib/ruc";

function errorText(e: unknown): string {
    return e instanceof Error ? e.message : String(e);
}

/**
 * This month's distance and estimated RUC charge for one vehicle, and the
 * expiry date of a heavy licence bought before the newest rate change.
 */
export function Dashboard() {
    const [vehicles, setVehicles] = useState<Vehicle[]>([]);
    const [trips, setTrips] = useState<Trip[]>([]);
    const [vehicleId, setVehicleId] = useState("");
    // Totals are kept with the vehicle they belong to, so a stale figure is never shown.
    const [totals, setTotals] = useState<{ vehicleId: string; value: ChargeTotals } | null>(null);
    // Expiry is kept with its vehicle too, for the same reason as the totals.
    const [expiry, setExpiry] = useState<{
        vehicleId: string;
        date?: string;
        checkedMs: number;
    } | null>(null);
    // Distance left on a purchased licence, kept with its vehicle for the same reason.
    const [licenceLeft, setLicenceLeft] = useState<{
        vehicleId: string;
        key: string;
        km: number;
        odometerKm: number;
    } | null>(null);
    const [error, setError] = useState<string | null>(null);

    function refresh() {
        Promise.all([listVehicles(), listTrips()])
            .then(([v, t]) => {
                setVehicles(v);
                setTrips(t);
                setVehicleId((current) =>
                    v.some((vehicle) => vehicle.id === current) ? current : (v[0]?.id ?? ""),
                );
            })
            .catch((e: unknown) => setError(errorText(e)));
    }

    useEffect(() => {
        refresh();
        return onVehiclesChanged(refresh);
    }, []);

    useEffect(() => {
        const vehicle = vehicles.find((v) => v.id === vehicleId);
        if (!vehicle) return;
        let cancelled = false;
        heavyLicenceExpiry(vehicle.grossKg)
            .then((date) => {
                if (!cancelled) setExpiry({ vehicleId: vehicle.id, date, checkedMs: Date.now() });
            })
            .catch((e: unknown) => {
                if (!cancelled) setError(`Could not read the licence rule: ${errorText(e)}`);
            });
        // Needs both a licence and an odometer reading: the reading is the only place the current odometer comes from.
        const key = licenceKey(vehicle);
        if (vehicle.distanceLicence && vehicle.odometerKm !== undefined) {
            const { odometerKm } = vehicle;
            licenceRemainingKm(vehicle.distanceLicence, odometerKm)
                .then((km) => {
                    if (!cancelled) setLicenceLeft({ vehicleId: vehicle.id, key, km, odometerKm });
                })
                .catch((e: unknown) => {
                    if (!cancelled)
                        setError(`Could not read the licence distance: ${errorText(e)}`);
                });
        }
        const monthTrips = tripsThisMonth(trips, vehicle.id, Date.now());
        Promise.all(monthTrips.map((trip) => estimateTrip(vehicle, trip)))
            .then((estimates) => {
                if (!cancelled) {
                    setTotals({
                        vehicleId: vehicle.id,
                        value: totalCharges(monthTrips, estimates),
                    });
                }
            })
            .catch((e: unknown) => {
                if (!cancelled) setError(`Could not price this month's trips: ${errorText(e)}`);
            });
        return () => {
            cancelled = true;
        };
    }, [vehicles, trips, vehicleId]);

    const selectedVehicle = vehicles.find((v) => v.id === vehicleId);

    if (vehicles.length === 0) {
        return (
            <section className="space-y-2" aria-labelledby="dashboard-heading">
                <h2 id="dashboard-heading" className="text-lg font-semibold">
                    This month
                </h2>
                <p>Add a vehicle to see this month&apos;s distance and estimated charge.</p>
            </section>
        );
    }

    return (
        <section className="space-y-3" aria-labelledby="dashboard-heading">
            <h2 id="dashboard-heading" className="text-lg font-semibold">
                This month
            </h2>
            <div className="flex flex-col gap-1">
                <label htmlFor="dashboard-vehicle">Vehicle</label>
                <select
                    id="dashboard-vehicle"
                    className="rounded border border-neutral-400 bg-white px-2 py-2 text-neutral-900"
                    value={vehicleId}
                    onChange={(e) => setVehicleId(e.target.value)}
                >
                    {vehicles.map((vehicle) => (
                        <option key={vehicle.id} value={vehicle.id}>
                            {vehicle.name}
                        </option>
                    ))}
                </select>
            </div>
            {error && <p role="alert">{error}</p>}
            {totals && totals.vehicleId === vehicleId && (
                <dl className="grid grid-cols-2 gap-x-4 gap-y-2">
                    <dt className="text-neutral-700">Trips</dt>
                    <dd>{totals.value.trips}</dd>
                    <dt className="text-neutral-700">Distance</dt>
                    <dd>{(totals.value.distanceM / 1000).toFixed(1)} km</dd>
                    <dt className="text-neutral-700">Estimated charge, including GST</dt>
                    <dd>
                        <strong>{formatMicros(totals.value.inclGstMicros)}</strong>
                    </dd>
                    <dt className="text-neutral-700">Excluding GST</dt>
                    <dd>{formatMicros(totals.value.exclGstMicros)}</dd>
                    <dt className="text-neutral-700">GST</dt>
                    <dd>{formatMicros(totals.value.gstMicros)}</dd>
                </dl>
            )}
            {licenceLeft &&
                licenceLeft.vehicleId === vehicleId &&
                licenceLeft.key === licenceKey(selectedVehicle) && (
                    <p>
                        {licenceLeft.km === 0 ? (
                            <strong>The distance licence is used up.</strong>
                        ) : (
                            <>
                                Distance left on the licence:{" "}
                                <strong>{licenceLeft.km.toLocaleString("en-NZ")} km</strong>.
                            </>
                        )}{" "}
                        This estimate uses the odometer you entered,{" "}
                        {licenceLeft.odometerKm.toLocaleString("en-NZ")} km, so update the odometer
                        to refresh it. The licence model is not checked against NZTA guidance.
                    </p>
                )}
            {expiry && expiry.vehicleId === vehicleId && expiry.date && (
                <ExpiryNote isoDate={expiry.date} checkedMs={expiry.checkedMs} />
            )}
            <p className="text-sm text-neutral-600">Estimates are not a licence purchase.</p>
        </section>
    );
}

/** Identifies the licence inputs a remaining-distance figure was worked out from. */
function licenceKey(vehicle: Vehicle | undefined): string {
    if (!vehicle?.distanceLicence || vehicle.odometerKm === undefined) return "";
    const { startOdometerKm, licensedKm } = vehicle.distanceLicence;
    return `${vehicle.odometerKm}/${startOdometerKm}/${licensedKm}`;
}

/**
 * Says when a heavy licence bought before the rate change expires, while that date is still ahead.
 * Once it has passed the note says so, rather than showing a date that is over.
 */
function ExpiryNote({ isoDate, checkedMs }: { isoDate: string; checkedMs: number }) {
    const days = daysUntil(isoDate, checkedMs);
    if (days < 0) {
        return (
            <p>
                Heavy licences bought before the rate change on {isoDate} expired on that date.
                Check your licence with NZTA.
            </p>
        );
    }
    return (
        <p>
            Heavy licences bought before the rate change expire on <strong>{isoDate}</strong>, in{" "}
            {days} {days === 1 ? "day" : "days"}.
        </p>
    );
}
