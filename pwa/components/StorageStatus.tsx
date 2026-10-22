import { useEffect, useState } from "react";
import { requestPersistentStorage } from "@/lib/db";
import { formatBytes, isNearQuota } from "@/lib/storage";

interface Estimate {
    usage: number;
    quota: number;
}

/** Shows how much browser storage the app uses, and asks to keep it. */
export function StorageStatus() {
    const [estimate, setEstimate] = useState<Estimate | null>(null);
    const [persistent, setPersistent] = useState<boolean | null>(null);

    useEffect(() => {
        let cancelled = false;
        async function load() {
            const quota = await navigator.storage?.estimate?.();
            if (!cancelled && quota?.usage !== undefined && quota.quota !== undefined) {
                setEstimate({ usage: quota.usage, quota: quota.quota });
            }
            const granted = await requestPersistentStorage();
            if (!cancelled) setPersistent(granted);
        }
        load().catch(() => {
            if (!cancelled) setPersistent(false);
        });
        return () => {
            cancelled = true;
        };
    }, []);

    if (!estimate) {
        return <p className="text-sm text-neutral-600">Storage: checking…</p>;
    }

    return (
        <div className="space-y-1 text-sm text-neutral-700">
            <p>
                Storage in use: {formatBytes(estimate.usage)} of {formatBytes(estimate.quota)}.{" "}
                {persistent === true
                    ? "The browser will keep this data."
                    : "The browser may clear this data under storage pressure."}
            </p>
            {isNearQuota(estimate.usage, estimate.quota) && (
                <p role="alert" className="text-amber-800">
                    Storage is nearly full. Export a backup and free up space.
                </p>
            )}
        </div>
    );
}
