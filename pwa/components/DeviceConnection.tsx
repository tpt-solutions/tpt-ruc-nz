"use client";

import { useEffect, useRef, useState } from "react";
import { TripTracker } from "tpt-ruc-wasm";
import {
    BleLink,
    detectBleSupport,
    downloadToStore,
    type BleSupport,
    type BluetoothLike,
    type LinkState,
} from "@/lib/ble-connection";
import type { LiveFix } from "@/lib/ble";
import { feedTracker } from "@/lib/live-track";
import { loadRuc } from "@/lib/ruc";
import { mergeDeviceSegments } from "@/lib/device-segments";

function describeState(state: LinkState): string {
    switch (state.kind) {
        case "idle":
            return "Not connected.";
        case "connecting":
            return state.attempt > 1
                ? `Reconnecting, attempt ${state.attempt}…`
                : "Connecting. Choose your device in the list.";
        case "connected":
            return `Connected to ${state.name}.`;
        case "error":
            return state.message;
    }
}

/**
 * Connects to the tpt-ruc device over Bluetooth, shows its live position, and
 * downloads the segments it has stored. Browsers without Web Bluetooth get a
 * message that says why, and the file import on the device-segments screen still works.
 */
export function DeviceConnection() {
    const [support, setSupport] = useState<BleSupport | null>(null);
    const [state, setState] = useState<LinkState>({ kind: "idle" });
    const [fix, setFix] = useState<LiveFix | null>(null);
    const [liveMetres, setLiveMetres] = useState<number | null>(null);
    const [downloadMessage, setDownloadMessage] = useState<string | null>(null);
    const [busy, setBusy] = useState(false);
    const linkRef = useRef<BleLink | null>(null);
    const trackerRef = useRef<TripTracker | null>(null);

    useEffect(() => {
        const detected = detectBleSupport({
            bluetooth: (navigator as Navigator & { bluetooth?: unknown }).bluetooth,
            isSecureContext: window.isSecureContext,
            userAgent: navigator.userAgent,
        });
        // eslint-disable-next-line react-hooks/set-state-in-effect -- browser-only state: navigator and window do not exist during the static prerender
        setSupport(detected);
        if (!detected.supported) return;

        let cancelled = false;
        loadRuc().then(() => {
            if (cancelled) return;
            trackerRef.current = new TripTracker();
        });
        const bluetooth = (navigator as Navigator & { bluetooth?: BluetoothLike }).bluetooth;
        if (!bluetooth) return;
        linkRef.current = new BleLink(bluetooth, {
            onState: setState,
            onLiveFix: (next) => {
                setFix(next);
                const tracker = trackerRef.current;
                if (tracker) setLiveMetres(feedTracker(tracker, next).totalM);
            },
            sleep: (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
        });
        return () => {
            cancelled = true;
            linkRef.current?.disconnect();
        };
    }, []);

    async function connect() {
        setDownloadMessage(null);
        await linkRef.current?.connect();
    }

    async function download() {
        const link = linkRef.current;
        if (!link) return;
        setBusy(true);
        setDownloadMessage(null);
        try {
            const result = await downloadToStore(link, mergeDeviceSegments);
            setDownloadMessage(
                result.received === 0
                    ? "The device has no stored segments."
                    : `Received ${result.received} segments: ${result.added} new, ${result.duplicates} already stored.`,
            );
        } catch (e) {
            setDownloadMessage(
                `Download failed: ${e instanceof Error ? e.message : String(e)}. Nothing was removed from the device.`,
            );
        } finally {
            setBusy(false);
        }
    }

    if (support === null) return null;

    const connected = state.kind === "connected";
    const working = state.kind === "connecting";

    return (
        <section className="space-y-3" aria-labelledby="device-heading">
            <h2 id="device-heading" className="text-lg font-semibold">
                Tracking device
            </h2>

            {!support.supported ? (
                <p>{support.message}</p>
            ) : (
                <>
                    <p aria-live="polite">{describeState(state)}</p>
                    <div className="flex flex-wrap gap-3">
                        <button
                            type="button"
                            className="rounded bg-neutral-900 px-4 py-2 font-medium text-white disabled:opacity-40"
                            onClick={() => void connect()}
                            disabled={connected || working}
                        >
                            Connect to device
                        </button>
                        <button
                            type="button"
                            className="rounded border border-neutral-400 px-4 py-2 disabled:opacity-40"
                            onClick={() => linkRef.current?.disconnect()}
                            disabled={!connected && !working}
                        >
                            Disconnect
                        </button>
                        <button
                            type="button"
                            className="rounded border border-neutral-400 px-4 py-2 disabled:opacity-40"
                            onClick={() => void download()}
                            disabled={!connected || busy}
                        >
                            {busy ? "Downloading…" : "Download stored trips"}
                        </button>
                    </div>
                    {downloadMessage && <p aria-live="polite">{downloadMessage}</p>}
                    {fix && (
                        <dl className="grid grid-cols-[max-content_1fr] gap-x-4 gap-y-1 text-sm">
                            <dt className="text-neutral-700">Position</dt>
                            <dd>
                                {fix.latDegrees.toFixed(5)}, {fix.lonDegrees.toFixed(5)}
                            </dd>
                            <dt className="text-neutral-700">Speed</dt>
                            <dd>
                                {fix.speedMps === null
                                    ? "not reported"
                                    : `${(fix.speedMps * 3.6).toFixed(0)} km/h`}
                            </dd>
                            <dt className="text-neutral-700">Distance since connecting</dt>
                            <dd>
                                {liveMetres === null ? "—" : `${(liveMetres / 1000).toFixed(3)} km`}
                            </dd>
                        </dl>
                    )}
                </>
            )}
        </section>
    );
}
