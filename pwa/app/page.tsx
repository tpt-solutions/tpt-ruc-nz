import { formatMicros } from "@/lib/money";

export default function Home() {
    return (
        <section className="space-y-4">
            <h1 className="text-2xl font-semibold">tpt-ruc-nz</h1>
            <p>
                Track distance and estimate RUC liability offline. The calculator and trip history
                arrive in later releases.
            </p>
            <p className="text-sm text-neutral-600">
                Example standard licence for 1,000 km at the light-vehicle rate:{" "}
                {formatMicros(76_000_000)} per 1,000 km, including GST.
            </p>
        </section>
    );
}
