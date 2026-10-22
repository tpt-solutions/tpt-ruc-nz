// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

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
