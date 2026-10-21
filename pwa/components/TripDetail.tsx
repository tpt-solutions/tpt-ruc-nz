"use client";

import { useEffect, useState } from "react";
import { estimateTrip, type TripEstimate } from "@/lib/ruc";
import { formatMicros } from "@/lib/money";
import { localIsoDate } from "@/lib/csv";
import type { Trip, Vehicle } from "@/lib/db";

interface Props {
    trip: Trip;
    /** The trip's vehicle, or undefined when the vehicle has been deleted. */
    vehicle: Vehicle | undefined;
    onClose: () => void;
}

/** Shows one trip with its estimated RUC charge, priced on the trip's date. */
export function TripDetail({ trip, vehicle, onClose }: Props) {
    const [estimate, setEstimate] = useState<TripEstimate | null>(null);
    const [error, setError] = useState<string | null>(null);

    // The parent keys this component by trip id, so each trip starts with no estimate.
    useEffect(() => {
        if (!vehicle) return;
        let cancelled = false;
        estimateTrip(vehicle, trip)
            .then((value) => {
                if (!cancelled) setEstimate(value);
            })
            .catch((e: unknown) => {
                if (!cancelled) setError(e instanceof Error ? e.message : String(e));
            });
        return () => {
            cancelled = true;
        };
    }, [trip, vehicle]);

    return (
        <section className="space-y-2 rounded border border-neutral-300 p-4">
            <h3 className="font-semibold">Trip details</h3>
            <p>
                {vehicle ? vehicle.name : "(deleted vehicle)"}, {localIsoDate(trip.startMs)},{" "}
                {(trip.distanceM / 1000).toFixed(3)} km
            </p>
            {!vehicle && (
                <p className="text-sm text-neutral-600">
                    The vehicle for this trip has been deleted, so no charge can be estimated.
                </p>
            )}
            {vehicle && !estimate && !error && <p>Estimating charge…</p>}
            {vehicle && estimate && (
                <p>
                    Estimated charge including GST:{" "}
                    <strong>{formatMicros(estimate.inclGstMicros)}</strong>
                    <span className="block text-xs text-neutral-500">
                        Rate table: {estimate.rateVersion}. An estimate only.
                    </span>
                </p>
            )}
            {error && <p role="alert">Could not estimate this trip: {error}</p>}
            <button type="button" className="text-sm underline" onClick={onClose}>
                Close details
            </button>
        </section>
    );
}
