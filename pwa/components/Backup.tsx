"use client";

import { useState } from "react";
import { exportBackup, getDb, parseBackup, restoreBackup } from "@/lib/db";
import { notifyVehiclesChanged } from "@/lib/events";

function errorText(e: unknown): string {
    return e instanceof Error ? e.message : String(e);
}

export function Backup() {
    const [status, setStatus] = useState<string | null>(null);
    const [error, setError] = useState<string | null>(null);

    function download() {
        setError(null);
        getDb()
            .then((db) => exportBackup(db))
            .then((backup) => {
                const url = URL.createObjectURL(
                    new Blob([JSON.stringify(backup, null, 2)], { type: "application/json" }),
                );
                const link = document.createElement("a");
                link.href = url;
                link.download = "tpt-ruc-backup.json";
                link.click();
                URL.revokeObjectURL(url);
                setStatus(
                    `Exported ${backup.vehicles.length} vehicles and ${backup.trips.length} trips.`,
                );
            })
            .catch((e: unknown) => setError(errorText(e)));
    }

    function restore(file: File) {
        setError(null);
        setStatus(null);
        file.text()
            .then((text) => parseBackup(JSON.parse(text)))
            .then((backup) =>
                getDb()
                    .then((db) => restoreBackup(db, backup))
                    .then(() => {
                        setStatus(
                            `Restored ${backup.vehicles.length} vehicles and ${backup.trips.length} trips.`,
                        );
                        notifyVehiclesChanged();
                    }),
            )
            .catch((e: unknown) => setError(`Could not restore that file: ${errorText(e)}`));
    }

    return (
        <section className="space-y-4">
            <h2 className="text-lg font-semibold">Backup</h2>
            <p className="text-sm text-neutral-600">
                Save your vehicles and trips to a file, or restore them from one. Restoring replaces
                any record with the same id.
            </p>
            <div className="flex flex-wrap items-center gap-4">
                <button
                    type="button"
                    className="rounded border border-neutral-400 px-4 py-2"
                    onClick={download}
                >
                    Export backup
                </button>
                <label className="flex items-center gap-2 text-sm">
                    <span>Restore from file</span>
                    <input
                        type="file"
                        accept="application/json,.json"
                        aria-label="Restore from file"
                        onChange={(e) => {
                            const file = e.target.files?.[0];
                            if (file) restore(file);
                            e.target.value = "";
                        }}
                    />
                </label>
            </div>
            {status && <p role="status">{status}</p>}
            {error && <p role="alert">{error}</p>}
        </section>
    );
}
