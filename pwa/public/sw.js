// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions
//
// Offline shell for the PWA. Same-origin GET requests are cached as they are
// fetched, and served from the cache first while the network refreshes it
// (stale-while-revalidate). Navigations fall back to the cached home page, so
// the app opens offline after one online visit.

const CACHE = "tpt-ruc-shell-v2";
const SHELL = ["/", "/manifest.webmanifest", "/icon.svg"];

// Periodic Background Sync. The sync engine is bundled from lib/ into
// sync-worker.js by `npm run build:sw`. If that file is missing the shell
// still works, and background sync is simply unavailable.
const PERIODIC_SYNC_TAG = "tpt-ruc-sync"; // keep in step with lib/background-sync.ts
try {
    importScripts("/sync-worker.js");
} catch (error) {
    console.warn("sync-worker.js not loaded; background sync is off", error);
}

self.addEventListener("periodicsync", (event) => {
    if (event.tag !== PERIODIC_SYNC_TAG || typeof self.tptRucSync !== "function") return;
    event.waitUntil(
        self.tptRucSync().catch((error) => {
            console.warn("background sync failed", error);
        }),
    );
});

self.addEventListener("install", (event) => {
    event.waitUntil(
        caches
            .open(CACHE)
            .then((cache) => cache.addAll(SHELL))
            .then(() => self.skipWaiting()),
    );
});

self.addEventListener("activate", (event) => {
    event.waitUntil(
        caches
            .keys()
            .then((keys) =>
                Promise.all(keys.filter((key) => key !== CACHE).map((key) => caches.delete(key))),
            )
            .then(() => self.clients.claim()),
    );
});

self.addEventListener("fetch", (event) => {
    const { request } = event;
    if (request.method !== "GET") return;
    if (new URL(request.url).origin !== self.location.origin) return;
    event.respondWith(staleWhileRevalidate(request));
});

async function staleWhileRevalidate(request) {
    const cache = await caches.open(CACHE);
    const cached = await cache.match(request);

    const refresh = fetch(request)
        .then((response) => {
            if (response.ok) cache.put(request, response.clone());
            return response;
        })
        .catch(() => undefined);

    if (cached) return cached;

    const network = await refresh;
    if (network) return network;

    if (request.mode === "navigate") {
        const home = await cache.match("/");
        if (home) return home;
    }
    return Response.error();
}
