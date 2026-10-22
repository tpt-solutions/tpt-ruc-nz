import { useEffect, useState } from "react";
import { setSetting, SECTION_43_ACK } from "@/lib/db";
import { rateTableVersion } from "@/lib/ruc";

/**
 * Settings that do not change how trips are recorded: the Section 43 notice,
 * the distance unit and the rate table in use. Sync settings are in SyncPanel.
 */
export function Settings() {
    const [error, setError] = useState<string | null>(null);
    const [rateVersion, setRateVersion] = useState<string | null>(null);

    useEffect(() => {
        rateTableVersion()
            .then(setRateVersion)
            .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)));
    }, []);

    function showNoticeAgain() {
        setSetting(SECTION_43_ACK, false)
            .then(() => window.location.reload())
            .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)));
    }

    return (
        <section className="space-y-3" aria-labelledby="settings-heading">
            <h2 id="settings-heading" className="text-lg font-semibold">
                Settings
            </h2>
            <dl className="grid grid-cols-[max-content_1fr] gap-x-4 gap-y-1 text-sm">
                <dt className="text-neutral-700">Distance unit</dt>
                <dd>Kilometres. Other units are not offered.</dd>
                <dt className="text-neutral-700">Rate table</dt>
                <dd>{rateVersion ?? "Loading…"}</dd>
            </dl>
            <button
                type="button"
                className="rounded border border-neutral-400 px-4 py-2"
                onClick={showNoticeAgain}
            >
                Show the Section 43 notice again
            </button>
            {error && <p role="alert">{error}</p>}
        </section>
    );
}
