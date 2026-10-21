import { RucCalculator } from "@/components/RucCalculator";

export default function Home() {
    return (
        <section className="space-y-6">
            <h1 className="text-2xl font-semibold">tpt-ruc-nz</h1>
            <p>
                Track distance and estimate RUC liability offline. Trip history and vehicle profiles
                arrive in later releases.
            </p>
            <div className="space-y-3">
                <h2 className="text-lg font-semibold">RUC calculator</h2>
                <p className="text-sm text-neutral-600">
                    Uses the NZTA distance-licence rates in force today. Figures are estimates. They
                    are not a licence purchase.
                </p>
                <RucCalculator />
            </div>
        </section>
    );
}
