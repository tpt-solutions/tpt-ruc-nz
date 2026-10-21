"use client";

import { useEffect, useState } from "react";
import { listVehicles, type Vehicle, type DeviceSegment } from "@/lib/db";
import {
    assignSegment,
    listDeviceSegments,
    listUnassignedSegments,
    mergeDeviceSegments,
    parseSegmentsJson,
} from "@/lib/device-segments";
import { localIsoDate } from "@/lib/csv";
import { notifyVehiclesChanged, onVehiclesChanged } from "@/lib/events";

function errorText(e: unknown): string {
    return e instanceof Error ? e.message : String(e);
}

/**
 * Imports trip segments recorded on the device, and assigns them to vehicles as
 * trips. Until the BLE link is built, segments come from a JSON file. Segments
 * already stored are skipped.
 */
export function DeviceSegments() {
    const [stored, setStored] = useState<number | null>(null);
    const [unassigned, setUnassigned] = useState<DeviceSegment[]>([]);
    const [vehicles, setVehicles] = useState<Vehicle[]>([]);
    const [vehicleId, setVehicleId] = useState("");
    const [status, setStatus] = useState<string | null>(null);
    const [error, setError] = useState<string | null>(null);

    function refresh() {
        Promise.all([listDeviceSegments(), listUnassignedSegments(), listVehicles()])
            .then(([all, pending, known]) => {
                setStored(all.length);
                setUnassigned(pending);
                setVehicles(known);
                setVehicleId((current) =>
                    known.some((v) => v.id === current) ? current : (known[0]?.id ?? ""),
                );
            })
            .catch((e: unknown) => setError(errorText(e)));
    }

    useEffect(() => {
        refresh();
        return onVehiclesChanged(refresh);
    }, []);

    function importFile(file: File) {
        setError(null);
        setStatus(null);
        file.text()
            .then((text) => mergeDeviceSegments(parseSegmentsJson(text)))
            .then(({ added, duplicates }) => {
                setStatus(`Added ${added} segments. ${duplicates} were already stored.`);
                refresh();
            })
            .catch((e: unknown) => setError(`Could not import that file: ${errorText(e)}`));
    }

    function assign(segment: DeviceSegment) {
        setError(null);
        setStatus(null);
        assignSegment(segment, vehicleId)
            .then(() => {
                setStatus(`Assigned ${(segment.distanceM / 1000).toFixed(3)} km to a vehicle.`);
                // Trips reload on this event, so the new trip appears at once.
                notifyVehiclesChanged();
                refresh();
            })
            .catch((e: unknown) => setError(`Could not assign that segment: ${errorText(e)}`));
    }

    return (
        <section className="space-y-3">
            <h2 className="text-lg font-semibold">Device segments</h2>
            <p className="text-sm text-neutral-600">
                Trips recorded on the device. Import a JSON file of segments, then assign each one
                to a vehicle. A segment that is already stored is skipped.
            </p>
            <p className="text-sm">
                Stored: {stored === null ? "checking…" : `${stored} segments`}
            </p>

            {unassigned.length > 0 && vehicles.length > 0 && (
                <div className="space-y-2">
                    <label className="flex flex-col gap-1 text-sm">
                        Assign to vehicle
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
                    <ul className="divide-y divide-neutral-300 rounded border border-neutral-300">
                        {unassigned.map((segment) => (
                            <li
                                key={segment.id}
                                className="flex items-center justify-between gap-2 p-3"
                            >
                                <span className="text-sm">
                                    {localIsoDate(segment.startMs)},{" "}
                                    {(segment.distanceM / 1000).toFixed(3)} km
                                </span>
                                <button
                                    type="button"
                                    className="text-sm underline"
                                    onClick={() => assign(segment)}
                                >
                                    Assign {segment.id}
                                </button>
                            </li>
                        ))}
                    </ul>
                </div>
            )}
            {unassigned.length > 0 && vehicles.length === 0 && (
                <p className="text-sm text-neutral-600">Add a vehicle to assign these segments.</p>
            )}

            <label className="flex items-center gap-2 text-sm">
                <span>Import segments file</span>
                <input
                    type="file"
                    accept="application/json,.json"
                    aria-label="Import segments file"
                    onChange={(e) => {
                        const file = e.target.files?.[0];
                        if (file) importFile(file);
                        e.target.value = "";
                    }}
                />
            </label>
            {status && <p role="status">{status}</p>}
            {error && <p role="alert">{error}</p>}
        </section>
    );
}
