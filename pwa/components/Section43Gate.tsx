"use client";

import { useEffect, useState, type ReactNode } from "react";
import { getSetting, setSetting, SECTION_43_ACK } from "@/lib/db";

/**
 * Shows the Section 43 notice until the user accepts it once. The acceptance
 * is kept in the local database, so it is asked for on first use only.
 */
export function Section43Gate({ children }: { children: ReactNode }) {
    const [state, setState] = useState<"checking" | "required" | "accepted">("checking");
    const [checked, setChecked] = useState(false);
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        getSetting<boolean>(SECTION_43_ACK)
            .then((acknowledged) => setState(acknowledged ? "accepted" : "required"))
            .catch((e: unknown) => {
                setError(e instanceof Error ? e.message : String(e));
                setState("required");
            });
    }, []);

    if (state === "accepted") return <>{children}</>;
    if (state === "checking") return <p>Loading…</p>;

    return (
        <section className="space-y-4 rounded border border-amber-500 bg-amber-50 p-4 text-neutral-900">
            <h2 className="text-lg font-semibold">Before you start</h2>
            <p>
                tpt-ruc-nz is <strong>not an NZTA-approved distance recorder</strong>. Using it for
                an official RUC licence, a claimed deduction or a digital licence display requires
                Section 43 approval from NZTA first.
            </p>
            <label className="flex items-start gap-2 text-sm">
                <input
                    type="checkbox"
                    className="mt-1"
                    checked={checked}
                    onChange={(e) => setChecked(e.target.checked)}
                />
                I understand this is an estimate tool and not an approved distance recorder.
            </label>
            <button
                type="button"
                disabled={!checked}
                className="rounded bg-neutral-900 px-4 py-2 font-medium text-white disabled:opacity-40"
                onClick={() => {
                    setSetting(SECTION_43_ACK, true)
                        .then(() => setState("accepted"))
                        .catch((e: unknown) =>
                            setError(e instanceof Error ? e.message : String(e)),
                        );
                }}
            >
                Continue
            </button>
            {error && <p role="alert">Could not save your choice: {error}</p>}
        </section>
    );
}
