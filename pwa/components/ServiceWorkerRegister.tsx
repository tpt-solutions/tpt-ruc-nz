"use client";

import { useEffect } from "react";

/** Registers the offline shell service worker once the page has loaded. */
export function ServiceWorkerRegister() {
    useEffect(() => {
        if (!("serviceWorker" in navigator)) return;
        navigator.serviceWorker.register("/sw.js").catch((error: unknown) => {
            console.error("Service worker registration failed", error);
        });
    }, []);
    return null;
}
