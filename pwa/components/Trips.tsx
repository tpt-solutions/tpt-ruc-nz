"use client";

import { useEffect, useState, type FormEvent } from "react";
import { listVehicles, newId, type Vehicle } from "@/lib/db";
import { listTrips, deleteTrip, putTrip } from "@/lib/trips";
import { localIsoDate, tripsToCsv } from "@/lib/csv";
import { onVehiclesChanged } from "@/lib/events";
import type { Trip } from "@/lib/db";

function errorText(e: unknown): string {
    return e instanceof Error ? e.message : String(e);
}

/** Today's local date as `YYYY-MM-DD`, for the date input. */
function today(): string {
    return localIsoDate(Date.now());
}

export function Trips() {
    const [vehicles, setVehicles] = useState<Vehicle[]>([]);
    const [trips, setTrips] = useState<Trip[] | null>(null);
    const [vehicleId, setVehicleId] = useState("");
    const [date, setDate] = useState(today);
    const [km, setKm] = useState("");
    const [error, setError] = useState<string | null>(null);

    function refresh() {
        Promise.all([listVehicles(), listTrips()])
            .then(([v, t]) => {
                setVehicles(v);
                setTrips(t);
                // Keep the selection if its vehicle still exists, otherwise pick the first.
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

    function add(event: FormEvent) {
        event.preventDefault();
        const distanceKm = Number(km);
        if (!vehicleId) {
            setError("Add a vehicle before recording a trip.");
            return;
        }
        if (!Number.isFinite(distanceKm) || distanceKm < 0) {
            setError("Enter a distance of zero or more.");
            return;
        }
        const [year, month, day] = date.split("-").map(Number);
        const startMs = new Date(year, month - 1, day).getTime();
        if (Number.isNaN(startMs)) {
            setError("Enter a valid date.");
            return;
        }
        putTrip({
            id: newId(),
            vehicleId,
            startMs,
            endMs: startMs,
            distanceM: Math.round(distanceKm * 1000),
        })
            .then(() => {
                setKm("");
                setError(null);
                refresh();
            })
            .catch((e: unknown) => setError(errorText(e)));
    }

    function downloadCsv() {
        if (!trips) return;
        const names = new Map(vehicles.map((v) => [v.id, v.name]));
        const csv = tripsToCsv(
            trips.map((t) => ({
                vehicle: names.get(t.vehicleId) ?? "(deleted vehicle)",
                date: localIsoDate(t.startMs),
                distanceM: t.distanceM,
            })),
        );
        const url = URL.createObjectURL(new Blob([csv], { type: "text/csv" }));
        const link = document.createElement("a");
        link.href = url;
        link.download = "tpt-ruc-trips.csv";
        link.click();
        URL.revokeObjectURL(url);
    }

    const names = new Map(vehicles.map((v) => [v.id, v.name]));

    return (
        <section className="space-y-4">
            <h2 className="text-lg font-semibold">Trips</h2>

            {trips === null && <p>Loading trips…</p>}
            {trips?.length === 0 && <p className="text-sm text-neutral-600">No trips yet.</p>}
            {trips && trips.length > 0 && (
                <>
                    <ul className="divide-y divide-neutral-300 rounded border border-neutral-300">
                        {trips.map((trip) => (
                            <li
                                key={trip.id}
                                className="flex items-center justify-between gap-2 p-3"
                            >
                                <span>
                                    <strong>
                                        {names.get(trip.vehicleId) ?? "(deleted vehicle)"}
                                    </strong>{" "}
                                    <span className="text-sm text-neutral-600">
                                        {localIsoDate(trip.startMs)},{" "}
                                        {(trip.distanceM / 1000).toFixed(3)} km
                                    </span>
                                </span>
                                <button
                                    type="button"
                                    className="text-sm text-red-700 underline"
                                    onClick={() => {
                                        deleteTrip(trip.id)
                                            .then(refresh)
                                            .catch((e: unknown) => setError(errorText(e)));
                                    }}
                                >
                                    Delete trip
                                </button>
                            </li>
                        ))}
                    </ul>
                    <button
                        type="button"
                        className="rounded border border-neutral-400 px-4 py-2"
                        onClick={downloadCsv}
                    >
                        Download CSV
                    </button>
                </>
            )}

            <form className="grid gap-3 sm:grid-cols-2" onSubmit={add}>
                <label className="flex flex-col gap-1 text-sm">
                    Vehicle
                    <select
                        className="rounded border border-neutral-400 bg-white px-2 py-2 text-neutral-900"
                        value={vehicleId}
                        onChange={(e) => setVehicleId(e.target.value)}
                    >
                        {vehicles.map((v) => (
                            <option key={v.id} value={v.id}>
                                {v.name}
                            </option>
                        ))}
                    </select>
                </label>
                <label className="flex flex-col gap-1 text-sm">
                    Date
                    <input
                        type="date"
                        className="rounded border border-neutral-400 bg-white px-2 py-2 text-neutral-900"
                        value={date}
                        onChange={(e) => setDate(e.target.value)}
                    />
                </label>
                <label className="flex flex-col gap-1 text-sm">
                    Distance (km)
                    <input
                        className="rounded border border-neutral-400 bg-white px-2 py-2 text-neutral-900"
                        inputMode="decimal"
                        value={km}
                        onChange={(e) => setKm(e.target.value)}
                    />
                </label>
                <button
                    type="submit"
                    className="self-end rounded bg-neutral-900 px-4 py-2 font-medium text-white"
                >
                    Add trip
                </button>
            </form>

            {error && <p role="alert">{error}</p>}
        </section>
    );
}
