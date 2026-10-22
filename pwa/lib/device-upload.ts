// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// Signed trip uploads. The backend (crates/tpt-ruc-backend/src/devices.rs) checks
// three headers: the device ID, a sequence number that must rise, and a hex Ed25519
// signature over `tpt-ruc-trips-v1\n<device>\n<sequence>\n<trips as sorted-key JSON>`.
// This module builds those headers. Registering the device needs a user's token,
// because the owner key has no user to register under.

import { readAppSettings, writeAppSettings } from "./app-settings";
import type { DeviceKey } from "./db";
import type { UploadTrip } from "./sync";

/** Domain separator the backend signs under. Must match `MESSAGE_PREFIX` in devices.rs. */
export const SIGNED_MESSAGE_PREFIX = "tpt-ruc-trips-v1";

/** Headers added to a signed upload. */
export type SignatureHeaders = Record<string, string>;

/** Produces the headers for one upload, given its trips. */
export type UploadSigner = (trips: UploadTrip[]) => Promise<SignatureHeaders>;

/** The device registration this browser keeps, and the sequence number of its last upload. */
export interface DeviceState {
    deviceId: string;
    deviceKeyId: string;
    deviceBackendUrl: string;
    deviceSequence: number;
}

/** Where the registration and sequence number are kept. The app uses the settings store. */
export interface DeviceStateStore {
    read(): Promise<DeviceState>;
    write(patch: Partial<DeviceState>): Promise<void>;
}

export const appSettingsDeviceState: DeviceStateStore = {
    async read() {
        const settings = await readAppSettings();
        return {
            deviceId: settings.deviceId,
            deviceKeyId: settings.deviceKeyId,
            deviceBackendUrl: settings.deviceBackendUrl,
            deviceSequence: settings.deviceSequence,
        };
    },
    async write(patch) {
        await writeAppSettings(patch);
    },
};

/** Sorts object keys at every level, so the bytes match the backend's `serde_json` output. */
function sortKeys(value: unknown): unknown {
    if (Array.isArray(value)) return value.map(sortKeys);
    if (value !== null && typeof value === "object") {
        const sorted: Record<string, unknown> = {};
        for (const key of Object.keys(value).sort()) {
            sorted[key] = sortKeys((value as Record<string, unknown>)[key]);
        }
        return sorted;
    }
    return value;
}

/** Compact JSON with keys in sorted order: the form the backend signs. */
export function canonicalJson(value: unknown): string {
    return JSON.stringify(sortKeys(value));
}

/** The bytes the device signs for one upload. */
export function signingMessage(
    deviceId: string,
    sequence: number,
    tripsJson: string,
): Uint8Array<ArrayBuffer> {
    const bytes = new TextEncoder().encode(
        `${SIGNED_MESSAGE_PREFIX}\n${deviceId}\n${sequence}\n${tripsJson}`,
    );
    // Copy into a plain ArrayBuffer, which WebCrypto's sign() accepts.
    return new Uint8Array(bytes);
}

function toHex(bytes: Uint8Array): string {
    return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

function fromBase64(text: string): Uint8Array {
    return Uint8Array.from(atob(text), (c) => c.charCodeAt(0));
}

/** The public key as the 64 hex characters the registration endpoint takes. */
export function publicKeyHex(key: DeviceKey): string {
    return toHex(fromBase64(key.publicKeyBase64));
}

export interface DeviceSignerOptions {
    subtle: SubtleCrypto;
    key: DeviceKey;
    /** Backend origin, without a trailing slash. */
    baseUrl: string;
    /** A user's token. The owner key cannot register a device, so the signer returns no headers with it. */
    apiKey: string;
    fetch: typeof fetch;
    state?: DeviceStateStore;
}

/**
 * Registers this browser's key with the backend once, then signs each upload with
 * the next sequence number. When registration is not possible, for example with the
 * owner key, it returns no headers and the upload goes unsigned, which the backend allows.
 */
export function makeDeviceSigner(options: DeviceSignerOptions): UploadSigner {
    const state = options.state ?? appSettingsDeviceState;
    const { subtle, key, baseUrl, apiKey } = options;

    async function registeredDeviceId(): Promise<string | undefined> {
        const current = await state.read();
        if (
            current.deviceId &&
            current.deviceKeyId === key.id &&
            current.deviceBackendUrl === baseUrl
        ) {
            return current.deviceId;
        }
        try {
            const response = await options.fetch(`${baseUrl}/v1/devices`, {
                method: "POST",
                headers: {
                    "Content-Type": "application/json",
                    Authorization: `Bearer ${apiKey}`,
                },
                body: JSON.stringify({ name: "PWA browser", public_key: publicKeyHex(key) }),
            });
            if (!response.ok) return undefined;
            const body = (await response.json()) as { id?: unknown };
            if (typeof body.id !== "string") return undefined;
            await state.write({
                deviceId: body.id,
                deviceKeyId: key.id,
                deviceBackendUrl: baseUrl,
                deviceSequence: 0,
            });
            return body.id;
        } catch {
            return undefined;
        }
    }

    return async (trips: UploadTrip[]): Promise<SignatureHeaders> => {
        if (!key.privateKey) return {};
        const deviceId = await registeredDeviceId();
        if (!deviceId) return {};

        // Store the new sequence number before sending, so a retry never reuses one.
        const sequence = (await state.read()).deviceSequence + 1;
        await state.write({ deviceSequence: sequence });

        const signature = await subtle.sign(
            { name: "Ed25519" },
            key.privateKey,
            signingMessage(deviceId, sequence, canonicalJson(trips)),
        );
        return {
            "X-Device-Id": deviceId,
            "X-Device-Sequence": String(sequence),
            "X-Device-Signature": toHex(new Uint8Array(signature)),
        };
    };
}
