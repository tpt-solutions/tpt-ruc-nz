import { useEffect, useState } from "react";
import init, { liability } from "tpt-ruc-wasm";
import { formatMicros } from "@/lib/money";

/** A few common NZTA powered-vehicle types. The full list is in the core crate. */
const TYPES: ReadonlyArray<{ code: string; label: string }> = [
    { code: "1", label: "1 – Powered, 2 axles" },
    { code: "2", label: "2 – Powered, single and twin-tyred spaced axles" },
    { code: "6", label: "6 – Powered, 3 axles" },
    { code: "12", label: "12 – Plug-in petrol hybrid, 2 axles" },
    { code: "14", label: "14 – Powered, 4 axles" },
    { code: "19", label: "19 – Powered, 5 or more axles" },
];

interface Result {
    inclGstMicros: number;
    exclGstMicros: number;
    gstMicros: number;
    rateVersion: string;
}

interface LiabilityJs {
    rateVersion: string;
    billedMetres: number;
    inclGstMicros: number;
    exclGstMicros: number;
    gstMicros: number;
}

export function RucCalculator() {
    const [ready, setReady] = useState(false);
    const [loadError, setLoadError] = useState<string | null>(null);
    const [rateType, setRateType] = useState("1");
    const [grossKg, setGrossKg] = useState("2000");
    const [electric, setElectric] = useState(false);
    const [km, setKm] = useState("100");
    const [result, setResult] = useState<Result | null>(null);
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        init()
            .then(() => setReady(true))
            .catch((e: unknown) => setLoadError(String(e)));
    }, []);

    function calculate() {
        setError(null);
        const kg = Number(grossKg);
        const distance = Number(km);
        if (!Number.isInteger(kg) || kg <= 0) {
            setError("Enter a whole-number weight above zero.");
            setResult(null);
            return;
        }
        if (!Number.isFinite(distance) || distance < 0) {
            setError("Enter a distance of zero or more.");
            setResult(null);
            return;
        }
        const now = new Date();
        try {
            const value = liability(
                rateType,
                kg,
                electric,
                now.getFullYear(),
                now.getMonth() + 1,
                now.getDate(),
                distance * 1000,
            ) as LiabilityJs;
            setResult({
                inclGstMicros: value.inclGstMicros,
                exclGstMicros: value.exclGstMicros,
                gstMicros: value.gstMicros,
                rateVersion: value.rateVersion,
            });
        } catch (e) {
            setResult(null);
            setError(e instanceof Error ? e.message : String(e));
        }
    }

    if (loadError) {
        return <p role="alert">The calculator could not load: {loadError}</p>;
    }
    if (!ready) {
        return <p>Loading calculator…</p>;
    }

    return (
        <form
            className="space-y-4"
            onSubmit={(event) => {
                event.preventDefault();
                calculate();
            }}
        >
            <div className="grid gap-4 sm:grid-cols-2">
                <label className="flex flex-col gap-1 text-sm">
                    Vehicle type
                    <select
                        className="rounded border border-neutral-400 bg-white px-2 py-2 text-neutral-900"
                        value={rateType}
                        onChange={(e) => setRateType(e.target.value)}
                    >
                        {TYPES.map((t) => (
                            <option key={t.code} value={t.code}>
                                {t.label}
                            </option>
                        ))}
                    </select>
                </label>
                <label className="flex flex-col gap-1 text-sm">
                    RUC weight (kg)
                    <input
                        className="rounded border border-neutral-400 bg-white px-2 py-2 text-neutral-900"
                        inputMode="numeric"
                        value={grossKg}
                        onChange={(e) => setGrossKg(e.target.value)}
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
                <label className="flex items-center gap-2 text-sm">
                    <input
                        type="checkbox"
                        checked={electric}
                        onChange={(e) => setElectric(e.target.checked)}
                    />
                    Battery electric
                </label>
            </div>
            <button
                type="submit"
                className="rounded bg-neutral-900 px-4 py-2 font-medium text-white hover:bg-neutral-700"
            >
                Calculate
            </button>

            {error && <p role="alert">{error}</p>}

            {result && (
                <output className="block space-y-1 rounded border border-neutral-300 p-4">
                    <p>
                        Charge including GST: <strong>{formatMicros(result.inclGstMicros)}</strong>
                    </p>
                    <p className="text-sm text-neutral-600">
                        Excluding GST {formatMicros(result.exclGstMicros)}, GST{" "}
                        {formatMicros(result.gstMicros)}
                    </p>
                    <p className="text-xs text-neutral-500">Rate table: {result.rateVersion}</p>
                </output>
            )}
        </form>
    );
}
