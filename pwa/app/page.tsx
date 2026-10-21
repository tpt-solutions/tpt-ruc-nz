import { RucCalculator } from "@/components/RucCalculator";
import { Section43Gate } from "@/components/Section43Gate";
import { Trips } from "@/components/Trips";
import { Vehicles } from "@/components/Vehicles";

export default function Home() {
    return (
        <Section43Gate>
            <section className="space-y-8">
                <h1 className="text-2xl font-semibold">tpt-ruc-nz</h1>
                <p>
                    Track distance and estimate RUC liability offline. Trips can be entered by hand
                    and exported as CSV. Automatic GPS tracking arrives in a later release.
                </p>
                <div className="space-y-3">
                    <h2 className="text-lg font-semibold">RUC calculator</h2>
                    <p className="text-sm text-neutral-600">
                        Uses the NZTA distance-licence rates in force today. Figures are estimates.
                        They are not a licence purchase.
                    </p>
                    <RucCalculator />
                </div>
                <Vehicles />
                <Trips />
            </section>
        </Section43Gate>
    );
}
