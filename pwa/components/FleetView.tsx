import { useEffect, useState, type FormEvent } from "react";
import { readAppSettings } from "@/lib/app-settings";
import { fetchFleetSummary, type FleetRow } from "@/lib/fleet";
import { localIsoDate } from "@/lib/csv";

function errorText(e: unknown): string {
    return e instanceof Error ? e.message : String(e);
}

/** Totals per vehicle and month from the backend. Needs a manager or admin key. */
export function FleetView() {
    const [configured, setConfigured] = useState<boolean | null>(null);
    const [from, setFrom] = useState("");
    const [to, setTo] = useState("");
    const [rows, setRows] = useState<FleetRow[] | null>(null);
    const [error, setError] = useState<string | null>(null);
    const [busy, setBusy] = useState(false);

    useEffect(() => {
        const now = Date.now();
        const year = new Date(now).getFullYear();
        // Runs once on the client, so the browser's own settings and clock are read here.
        // eslint-disable-next-line react-hooks/set-state-in-effect -- browser-only state: storage and the clock
        setFrom(`${year}-01-01`);
        setTo(localIsoDate(now));
        readAppSettings()
            .then((settings) => setConfigured(settings.backendUrl !== "" && settings.apiKey !== ""))
            .catch((e: unknown) => setError(errorText(e)));
    }, []);

    async function load(event: FormEvent) {
        event.preventDefault();
        setError(null);
        setBusy(true);
        try {
            const settings = await readAppSettings();
            setRows(
                await fetchFleetSummary(
                    {
                        baseUrl: settings.backendUrl,
                        apiKey: settings.apiKey,
                        fetch: (i, init) => fetch(i, init),
                    },
                    from,
                    to,
                ),
            );
        } catch (e) {
            setRows(null);
            setError(errorText(e));
        } finally {
            setBusy(false);
        }
    }

    return (
        <section className="space-y-3" aria-labelledby="fleet-heading">
            <h2 id="fleet-heading" className="text-lg font-semibold">
                Fleet summary
            </h2>
            {configured === false && (
                <p>Enter the backend address and API key above to see the fleet summary.</p>
            )}
            {configured && (
                <form onSubmit={load} className="flex flex-wrap items-end gap-3">
                    <div className="flex flex-col gap-1">
                        <label htmlFor="fleet-from">From</label>
                        <input
                            id="fleet-from"
                            type="date"
                            className="rounded border border-neutral-400 bg-white px-2 py-2 text-neutral-900"
                            value={from}
                            onChange={(e) => setFrom(e.target.value)}
                            required
                        />
                    </div>
                    <div className="flex flex-col gap-1">
                        <label htmlFor="fleet-to">To</label>
                        <input
                            id="fleet-to"
                            type="date"
                            className="rounded border border-neutral-400 bg-white px-2 py-2 text-neutral-900"
                            value={to}
                            onChange={(e) => setTo(e.target.value)}
                            required
                        />
                    </div>
                    <button
                        type="submit"
                        className="rounded bg-neutral-900 px-4 py-2 font-medium text-white disabled:opacity-40"
                        disabled={busy}
                    >
                        {busy ? "Loading…" : "Load summary"}
                    </button>
                </form>
            )}
            {error && <p role="alert">{error}</p>}
            {rows && rows.length === 0 && <p>No trips in this range.</p>}
            {rows && rows.length > 0 && (
                <div className="overflow-x-auto">
                    <table className="w-full text-left text-sm">
                        <caption className="sr-only">Charges per month and vehicle</caption>
                        <thead>
                            <tr className="border-b border-neutral-400">
                                <th scope="col" className="py-2 pr-3">
                                    Month
                                </th>
                                <th scope="col" className="py-2 pr-3">
                                    Vehicle
                                </th>
                                <th scope="col" className="py-2 pr-3">
                                    Trips
                                </th>
                                <th scope="col" className="py-2 pr-3">
                                    Km
                                </th>
                                <th scope="col" className="py-2 pr-3">
                                    Incl. GST (NZD)
                                </th>
                            </tr>
                        </thead>
                        <tbody>
                            {rows.map((row) => (
                                <tr
                                    key={`${row.month}-${row.vehicleId ?? "inline"}-${row.rateType}`}
                                    className="border-b border-neutral-300"
                                >
                                    <td className="py-2 pr-3">{row.month}</td>
                                    <td className="py-2 pr-3">
                                        {row.vehicleName ?? `Type ${row.rateType} (not registered)`}
                                    </td>
                                    <td className="py-2 pr-3">{row.trips}</td>
                                    <td className="py-2 pr-3">{row.billedKm}</td>
                                    <td className="py-2 pr-3">{row.chargeInclGstNzd}</td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                </div>
            )}
        </section>
    );
}
