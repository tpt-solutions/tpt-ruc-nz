"use client";

import { useEffect, useRef, useState, type FormEvent } from "react";
import {
    readAppSettings,
    normaliseBackendUrl,
    writeAppSettings,
    type AppSettings,
} from "@/lib/app-settings";
import { ensureDeviceKey, ed25519Supported, loadDeviceKey, signText } from "@/lib/device-key";
import { indexedDbSyncStore, listRefusedTrips, requeueTrip, isPending } from "@/lib/sync-store";
import { syncPending, type SyncReport } from "@/lib/sync";
import type { Trip } from "@/lib/db";

function errorText(e: unknown): string {
    return e instanceof Error ? e.message : String(e);
}

const sleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

/** Settings for the backend and the sync screen: what is waiting, what was refused, and a Sync now button. */
export function SyncPanel() {
    const [settings, setSettings] = useState<AppSettings | null>(null);
    const [urlText, setUrlText] = useState("");
    const [keyText, setKeyText] = useState("");
    const [pendingCount, setPendingCount] = useState(0);
    const [refused, setRefused] = useState<Trip[]>([]);
    const [signing, setSigning] = useState<"unknown" | "on" | "off" | "unsupported">("unknown");
    const [report, setReport] = useState<SyncReport | null>(null);
    const [message, setMessage] = useState<string | null>(null);
    const [error, setError] = useState<string | null>(null);
    const [busy, setBusy] = useState(false);
    const busyRef = useRef(false);

    async function refresh() {
        const [current, pending, refusedTrips, key] = await Promise.all([
            readAppSettings(),
            indexedDbSyncStore.listPending(),
            listRefusedTrips(),
            loadDeviceKey(),
        ]);
        setSettings(current);
        setUrlText((text) => (text === "" ? current.backendUrl : text));
        setKeyText((text) => (text === "" ? current.apiKey : text));
        setPendingCount(pending.filter(isPending).length);
        setRefused(refusedTrips);
        if (key?.privateKey) setSigning("on");
        else if (key) setSigning("off");
        else {
            const supported = await ed25519Supported(globalThis.crypto?.subtle);
            setSigning(supported ? "off" : "unsupported");
        }
    }

    useEffect(() => {
        // eslint-disable-next-line react-hooks/set-state-in-effect -- browser-only state: navigator and window do not exist during the static prerender
        refresh().catch((e: unknown) => setError(errorText(e)));
        // A sync also runs when the browser comes back online, if sync is on.
        const onOnline = () => {
            void runSync();
        };
        window.addEventListener("online", onOnline);
        return () => window.removeEventListener("online", onOnline);
        // runSync reads the latest settings from storage, so the listener never goes stale.
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, []);

    async function save(event: FormEvent) {
        event.preventDefault();
        setError(null);
        setMessage(null);
        let url: string;
        try {
            url = normaliseBackendUrl(urlText);
        } catch (e) {
            setError(errorText(e));
            return;
        }
        try {
            const next = await writeAppSettings({ backendUrl: url, apiKey: keyText.trim() });
            setSettings(next);
            setMessage("Saved in this browser.");
            await refresh();
        } catch (e) {
            setError(errorText(e));
        }
    }

    async function setSyncEnabled(enabled: boolean) {
        setError(null);
        setMessage(null);
        // Show the new state at once, so the checkbox does not flicker while storage is written.
        setSettings((current) => (current ? { ...current, syncEnabled: enabled } : current));
        try {
            const next = await writeAppSettings({ syncEnabled: enabled });
            setSettings(next);
            if (enabled) {
                // Making the signing key now lets the first upload be signed.
                await ensureDeviceKey(globalThis.crypto?.subtle);
            }
            await refresh();
            setMessage(enabled ? "Upload is on." : "Upload is off. Trips stay on this device.");
        } catch (e) {
            setError(errorText(e));
        }
    }

    async function runSync() {
        if (busyRef.current) return;
        const current = await readAppSettings();
        if (!current.syncEnabled) return;
        if (!current.backendUrl || !current.apiKey) {
            setError("Enter the backend address and API key before uploading.");
            return;
        }
        busyRef.current = true;
        setBusy(true);
        setError(null);
        setMessage(null);
        try {
            const key = await loadDeviceKey();
            const subtle = globalThis.crypto?.subtle;
            const result = await syncPending(
                {
                    baseUrl: current.backendUrl,
                    apiKey: current.apiKey,
                    fetch: (input, init) => fetch(input, init),
                    sign:
                        key?.privateKey && subtle
                            ? (body) => signText(subtle, key, body)
                            : undefined,
                    sleep,
                },
                indexedDbSyncStore,
            );
            setReport(result);
            if (result.error) setError(result.error);
            await refresh();
        } catch (e) {
            setError(errorText(e));
        } finally {
            busyRef.current = false;
            setBusy(false);
        }
    }

    async function tryAgain(tripId: string) {
        try {
            await requeueTrip(tripId);
            await refresh();
            setMessage("The trip will be sent on the next sync.");
        } catch (e) {
            setError(errorText(e));
        }
    }

    const ready = settings !== null && settings.backendUrl !== "" && settings.apiKey !== "";

    return (
        <section className="space-y-4" aria-labelledby="sync-heading">
            <h2 id="sync-heading" className="text-lg font-semibold">
                Backend and upload
            </h2>
            <p className="text-sm text-neutral-600">
                Nothing leaves this device until you turn upload on. Trips are then sent to the
                backend you name below.
            </p>

            <form onSubmit={save} className="space-y-3">
                <div className="flex flex-col gap-1">
                    <label htmlFor="backend-url">Backend address</label>
                    <input
                        id="backend-url"
                        type="url"
                        inputMode="url"
                        autoComplete="off"
                        placeholder="http://localhost:8080"
                        className="rounded border border-neutral-400 bg-white px-2 py-2 text-neutral-900"
                        value={urlText}
                        onChange={(e) => setUrlText(e.target.value)}
                    />
                </div>
                <div className="flex flex-col gap-1">
                    <label htmlFor="backend-key">API key</label>
                    <input
                        id="backend-key"
                        type="password"
                        autoComplete="off"
                        className="rounded border border-neutral-400 bg-white px-2 py-2 text-neutral-900"
                        value={keyText}
                        onChange={(e) => setKeyText(e.target.value)}
                        aria-describedby="backend-key-warning"
                    />
                    <p id="backend-key-warning" className="text-sm text-amber-800">
                        The key is stored in this browser&apos;s database. Anyone who can use this
                        browser profile can read it, so enter a key only on a device you control.
                    </p>
                </div>
                <button
                    type="submit"
                    className="rounded bg-neutral-900 px-4 py-2 font-medium text-white"
                >
                    Save backend settings
                </button>
            </form>

            <div className="space-y-2">
                <label className="flex items-center gap-2">
                    <input
                        type="checkbox"
                        checked={settings?.syncEnabled ?? false}
                        onChange={(e) => void setSyncEnabled(e.target.checked)}
                    />
                    Upload trips to the backend
                </label>
                <p className="text-sm text-neutral-600">
                    Signing:{" "}
                    {signing === "on"
                        ? "uploads are signed with this device's key. The backend does not check signatures yet."
                        : signing === "off"
                          ? "a signing key is made when upload is turned on."
                          : signing === "unsupported"
                            ? "this browser cannot make signing keys, so uploads are not signed. Uploads still work."
                            : "checking…"}
                </p>
            </div>

            <div className="space-y-2" aria-live="polite">
                <p>
                    Waiting to upload: <strong>{pendingCount}</strong>
                </p>
                {report && !error && (
                    <p>
                        Last sync: {report.uploaded} uploaded, {report.conflicts.length} conflicts,{" "}
                        {report.rejected.length} rejected.
                    </p>
                )}
                {message && <p>{message}</p>}
            </div>

            <div className="flex flex-wrap gap-3">
                <button
                    type="button"
                    className="rounded border border-neutral-400 px-4 py-2 disabled:opacity-40"
                    disabled={busy || !ready || !settings?.syncEnabled}
                    onClick={() => void runSync()}
                >
                    {busy ? "Uploading…" : "Sync now"}
                </button>
            </div>
            {error && <p role="alert">{error}</p>}

            {refused.length > 0 && (
                <div className="space-y-2">
                    <h3 className="font-semibold">Trips the backend did not store</h3>
                    <p className="text-sm text-neutral-600">
                        These trips are kept on this device. Check the reason, then try again. A
                        conflict means the server already has a different trip with the same ID.
                    </p>
                    <ul className="space-y-2">
                        {refused.map((trip) => (
                            <li key={trip.id} className="rounded border border-neutral-300 p-3">
                                <p className="font-mono text-sm">{trip.id}</p>
                                <p>
                                    {trip.sync?.state === "conflict" ? "Conflict" : "Rejected"}:{" "}
                                    {trip.sync?.message}
                                </p>
                                <button
                                    type="button"
                                    className="mt-2 text-sm underline"
                                    onClick={() => void tryAgain(trip.id)}
                                >
                                    Try again for trip {trip.id}
                                </button>
                            </li>
                        ))}
                    </ul>
                </div>
            )}
        </section>
    );
}
