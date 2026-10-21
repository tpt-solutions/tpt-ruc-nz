"use client";

import { useEffect, useState, type FormEvent } from "react";
import { deleteVehicle, listVehicles, newId, putVehicle, type Vehicle } from "@/lib/db";
import { notifyVehiclesChanged } from "@/lib/events";

const TYPE_CODES = ["1", "2", "6", "12", "14", "19"] as const;

function errorText(e: unknown): string {
    return e instanceof Error ? e.message : String(e);
}

export function Vehicles() {
    const [vehicles, setVehicles] = useState<Vehicle[] | null>(null);
    const [editingId, setEditingId] = useState<string | null>(null);
    const [name, setName] = useState("");
    const [rateType, setRateType] = useState<string>("1");
    const [grossKg, setGrossKg] = useState("2000");
    const [electric, setElectric] = useState(false);
    const [odometer, setOdometer] = useState("");
    const [licenceStart, setLicenceStart] = useState("");
    const [licenceKm, setLicenceKm] = useState("");
    const [error, setError] = useState<string | null>(null);

    function refresh() {
        listVehicles()
            .then(setVehicles)
            .catch((e: unknown) => setError(errorText(e)));
    }

    useEffect(refresh, []);

    function resetForm() {
        setEditingId(null);
        setName("");
        setRateType("1");
        setGrossKg("2000");
        setElectric(false);
        setOdometer("");
        setLicenceStart("");
        setLicenceKm("");
    }

    function edit(vehicle: Vehicle) {
        setEditingId(vehicle.id);
        setName(vehicle.name);
        setRateType(vehicle.rateType);
        setGrossKg(String(vehicle.grossKg));
        setElectric(vehicle.electric);
        setOdometer(vehicle.odometerKm === undefined ? "" : String(vehicle.odometerKm));
        setLicenceStart(
            vehicle.distanceLicence === undefined
                ? ""
                : String(vehicle.distanceLicence.startOdometerKm),
        );
        setLicenceKm(
            vehicle.distanceLicence === undefined ? "" : String(vehicle.distanceLicence.licensedKm),
        );
        setError(null);
    }

    function save(event: FormEvent) {
        event.preventDefault();
        const kg = Number(grossKg);
        if (!name.trim()) {
            setError("Give the vehicle a name.");
            return;
        }
        if (!Number.isInteger(kg) || kg <= 0) {
            setError("Enter a whole-number weight above zero.");
            return;
        }
        // The odometer is optional. When given, it must be a whole number of km, zero or more.
        const odometerKm = odometer.trim() === "" ? undefined : Number(odometer);
        if (odometerKm !== undefined && (!Number.isInteger(odometerKm) || odometerKm < 0)) {
            setError("Enter the odometer as a whole number of kilometres, or leave it blank.");
            return;
        }
        // The distance licence is optional. Its two figures must be given together, as whole km.
        const licenceGiven = licenceStart.trim() !== "" || licenceKm.trim() !== "";
        const startOdometerKm = Number(licenceStart);
        const licensedKm = Number(licenceKm);
        const licenceValid =
            Number.isInteger(startOdometerKm) &&
            startOdometerKm >= 0 &&
            Number.isInteger(licensedKm) &&
            licensedKm > 0;
        if (
            licenceGiven &&
            (!licenceValid || licenceStart.trim() === "" || licenceKm.trim() === "")
        ) {
            setError(
                "Enter the licence odometer as a whole number of km, and the licence distance as a whole number above zero. Or leave both blank.",
            );
            return;
        }
        const vehicle: Vehicle = {
            id: editingId ?? newId(),
            name: name.trim(),
            rateType,
            grossKg: kg,
            electric,
            odometerKm,
            distanceLicence: licenceGiven ? { startOdometerKm, licensedKm } : undefined,
        };
        putVehicle(vehicle)
            .then(() => {
                resetForm();
                setError(null);
                refresh();
                notifyVehiclesChanged();
            })
            .catch((e: unknown) => setError(errorText(e)));
    }

    return (
        <section className="space-y-4">
            <h2 className="text-lg font-semibold">Vehicles</h2>

            {vehicles === null && <p>Loading vehicles…</p>}
            {vehicles?.length === 0 && <p className="text-sm text-neutral-600">No vehicles yet.</p>}
            {vehicles && vehicles.length > 0 && (
                <ul className="divide-y divide-neutral-300 rounded border border-neutral-300">
                    {vehicles.map((vehicle) => (
                        <li
                            key={vehicle.id}
                            className="flex items-center justify-between gap-2 p-3"
                        >
                            <span>
                                <strong>{vehicle.name}</strong>{" "}
                                <span className="text-sm text-neutral-600">
                                    type {vehicle.rateType},{" "}
                                    {vehicle.grossKg.toLocaleString("en-NZ")} kg
                                    {vehicle.electric ? ", electric" : ""}
                                    {vehicle.odometerKm !== undefined &&
                                        `, odometer ${vehicle.odometerKm.toLocaleString("en-NZ")} km`}
                                </span>
                            </span>
                            <span className="flex gap-3">
                                <button
                                    type="button"
                                    className="text-sm underline"
                                    onClick={() => edit(vehicle)}
                                >
                                    Edit {vehicle.name}
                                </button>
                                <button
                                    type="button"
                                    className="text-sm text-red-700 underline"
                                    onClick={() => {
                                        deleteVehicle(vehicle.id)
                                            .then(() => {
                                                refresh();
                                                notifyVehiclesChanged();
                                            })
                                            .catch((e: unknown) => setError(errorText(e)));
                                    }}
                                >
                                    Delete {vehicle.name}
                                </button>
                            </span>
                        </li>
                    ))}
                </ul>
            )}

            <form className="grid gap-3 sm:grid-cols-2" onSubmit={save}>
                <label className="flex flex-col gap-1 text-sm">
                    Name
                    <input
                        className="rounded border border-neutral-400 bg-white px-2 py-2 text-neutral-900"
                        value={name}
                        onChange={(e) => setName(e.target.value)}
                    />
                </label>
                <label className="flex flex-col gap-1 text-sm">
                    Vehicle type
                    <select
                        className="rounded border border-neutral-400 bg-white px-2 py-2 text-neutral-900"
                        value={rateType}
                        onChange={(e) => setRateType(e.target.value)}
                    >
                        {TYPE_CODES.map((code) => (
                            <option key={code} value={code}>
                                Type {code}
                            </option>
                        ))}
                    </select>
                </label>
                <label className="flex flex-col gap-1 text-sm">
                    Vehicle RUC weight (kg)
                    <input
                        className="rounded border border-neutral-400 bg-white px-2 py-2 text-neutral-900"
                        inputMode="numeric"
                        value={grossKg}
                        onChange={(e) => setGrossKg(e.target.value)}
                    />
                </label>
                <label className="flex flex-col gap-1 text-sm">
                    Odometer (km), optional
                    <input
                        className="rounded border border-neutral-400 bg-white px-2 py-2 text-neutral-900"
                        inputMode="numeric"
                        value={odometer}
                        onChange={(e) => setOdometer(e.target.value)}
                    />
                </label>
                <label className="flex flex-col gap-1 text-sm">
                    Odometer when licence bought (km), optional
                    <input
                        className="rounded border border-neutral-400 bg-white px-2 py-2 text-neutral-900"
                        inputMode="numeric"
                        value={licenceStart}
                        onChange={(e) => setLicenceStart(e.target.value)}
                    />
                </label>
                <label className="flex flex-col gap-1 text-sm">
                    Distance licence covers (km), optional
                    <input
                        className="rounded border border-neutral-400 bg-white px-2 py-2 text-neutral-900"
                        inputMode="numeric"
                        value={licenceKm}
                        onChange={(e) => setLicenceKm(e.target.value)}
                    />
                </label>
                <label className="flex items-center gap-2 text-sm">
                    <input
                        type="checkbox"
                        checked={electric}
                        onChange={(e) => setElectric(e.target.checked)}
                    />
                    Battery electric
                </label>
                <div className="flex gap-3 sm:col-span-2">
                    <button
                        type="submit"
                        className="rounded bg-neutral-900 px-4 py-2 font-medium text-white"
                    >
                        {editingId ? "Save changes" : "Add vehicle"}
                    </button>
                    {editingId && (
                        <button
                            type="button"
                            className="rounded border border-neutral-400 px-4 py-2"
                            onClick={resetForm}
                        >
                            Cancel
                        </button>
                    )}
                </div>
            </form>

            {error && <p role="alert">{error}</p>}
        </section>
    );
}
