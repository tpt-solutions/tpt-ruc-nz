"use client";

import { useEffect, useState } from "react";
import { listDeviceSegments, mergeDeviceSegments, parseSegmentsJson } from "@/lib/device-segments";

function errorText(e: unknown): string {
    return e instanceof Error ? e.message : String(e);
}

/**
 * Imports trip segments recorded on the device. Until the BLE link is built,
 * segments come from a JSON file. Segments already stored are skipped.
 */
export function DeviceSegments() {
    const [stored, setStored] = useState<number | null>(null);
    const [status, setStatus] = useState<string | null>(null);
    const [error, setError] = useState<string | null>(null);

    function refresh() {
        listDeviceSegments()
            .then((segments) => setStored(segments.length))
            .catch((e: unknown) => setError(errorText(e)));
    }

    useEffect(refresh, []);

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

    return (
        <section className="space-y-3">
            <h2 className="text-lg font-semibold">Device segments</h2>
            <p className="text-sm text-neutral-600">
                Trips recorded on the device. Import a JSON file of segments. A segment that is
                already stored is skipped.
            </p>
            <p className="text-sm">
                Stored: {stored === null ? "checking…" : `${stored} segments`}
            </p>
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
