"use client";

import { useState } from "react";
import { setSetting, SECTION_43_ACK } from "@/lib/db";

/** Lets the user show the Section 43 notice again, for example on a shared device. */
export function Settings() {
    const [error, setError] = useState<string | null>(null);

    function showNoticeAgain() {
        setSetting(SECTION_43_ACK, false)
            .then(() => window.location.reload())
            .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)));
    }

    return (
        <section className="space-y-3">
            <h2 className="text-lg font-semibold">Settings</h2>
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
