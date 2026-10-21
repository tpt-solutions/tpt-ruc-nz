"use client";

import { useEffect, useState, type FormEvent } from "react";
import { deleteVehicle, listVehicles, newId, putVehicle, type Vehicle } from "@/lib/db";

const TYPE_CODES = ["1", "2", "6", "12", "14", "19"] as const;

function errorText(e: unknown): string {
    return e instanceof Error ? e.message : String(e);
}

export function Vehicles() {
    const [vehicles, setVehicles] = useState<Vehicle[] | null>(null);
    const [name, setName] = useState("");
    const [rateType, setRateType] = useState<string>("1");
    const [grossKg, setGrossKg] = useState("2000");
    const [electric, setElectric] = useState(false);
    const [error, setError] = useState<string | null>(null);

    function refresh() {
        listVehicles()
            .then(setVehicles)
            .catch((e: unknown) => setError(errorText(e)));
    }

    useEffect(refresh, []);

    function add(event: FormEvent) {
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
        putVehicle({ id: newId(), name: name.trim(), rateType, grossKg: kg, electric })
            .then(() => {
                setName("");
                setError(null);
                refresh();
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
                                </span>
                            </span>
                            <button
                                type="button"
                                className="text-sm text-red-700 underline"
                                onClick={() => {
                                    deleteVehicle(vehicle.id)
                                        .then(refresh)
                                        .catch((e: unknown) => setError(errorText(e)));
                                }}
                            >
                                Delete {vehicle.name}
                            </button>
                        </li>
                    ))}
                </ul>
            )}

            <form className="grid gap-3 sm:grid-cols-2" onSubmit={add}>
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
                <label className="flex items-center gap-2 text-sm">
                    <input
                        type="checkbox"
                        checked={electric}
                        onChange={(e) => setElectric(e.target.checked)}
                    />
                    Battery electric
                </label>
                <button
                    type="submit"
                    className="rounded bg-neutral-900 px-4 py-2 font-medium text-white sm:col-span-2"
                >
                    Add vehicle
                </button>
            </form>

            {error && <p role="alert">{error}</p>}
        </section>
    );
}
