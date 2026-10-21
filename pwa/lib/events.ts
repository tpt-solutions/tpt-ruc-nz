// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

/** Fired on `window` when vehicle data changes, so other components can reload. */
export const VEHICLES_CHANGED = "tpt-ruc:vehicles-changed";

export function notifyVehiclesChanged(): void {
    window.dispatchEvent(new Event(VEHICLES_CHANGED));
}

/** Subscribes to vehicle changes. Returns a function that unsubscribes. */
export function onVehiclesChanged(callback: () => void): () => void {
    window.addEventListener(VEHICLES_CHANGED, callback);
    return () => window.removeEventListener(VEHICLES_CHANGED, callback);
}
