// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// The browser's signing key for this device. The private key is made
// non-extractable, so script can use it to sign but cannot read its bytes.
// Browsers without Ed25519 in WebCrypto get no key, and uploads go unsigned.

import { getDb, type DeviceKey } from "./db";

export const SIGNING_ALGORITHM = "Ed25519";

/** Base64 of a byte array, without relying on Node or Buffer. */
export function toBase64(bytes: Uint8Array): string {
    let binary = "";
    for (const byte of bytes) binary += String.fromCharCode(byte);
    return btoa(binary);
}

/**
 * True when the browser can make and use Ed25519 keys. Checked by trying, since
 * the algorithm list in the specification is not a reliable guide.
 */
export async function ed25519Supported(subtle: SubtleCrypto | undefined): Promise<boolean> {
    if (!subtle) return false;
    try {
        await subtle.generateKey({ name: SIGNING_ALGORITHM }, false, ["sign", "verify"]);
        return true;
    } catch {
        return false;
    }
}

/** Makes a new key pair. The private half is not extractable. */
export async function createDeviceKey(
    subtle: SubtleCrypto,
    nowMs = Date.now(),
): Promise<DeviceKey> {
    const pair = (await subtle.generateKey({ name: SIGNING_ALGORITHM }, false, [
        "sign",
        "verify",
    ])) as CryptoKeyPair;
    const raw = new Uint8Array(await subtle.exportKey("raw", pair.publicKey));
    const idBytes = new Uint8Array(8);
    crypto.getRandomValues(idBytes);
    const id = `dk_${toBase64(idBytes).replace(/[^a-zA-Z0-9]/g, "")}`;
    return {
        id,
        publicKeyBase64: toBase64(raw),
        privateKey: pair.privateKey,
        createdMs: nowMs,
    };
}

/** Signs a text with the device key. Returns the base64 signature. */
export async function signText(
    subtle: SubtleCrypto,
    key: DeviceKey,
    text: string,
): Promise<string> {
    if (!key.privateKey) throw new Error("this device key cannot sign");
    const signature = await subtle.sign(
        { name: SIGNING_ALGORITHM },
        key.privateKey,
        new TextEncoder().encode(text),
    );
    return toBase64(new Uint8Array(signature));
}

/** The stored device key, if one has been made. */
export async function loadDeviceKey(): Promise<DeviceKey | undefined> {
    const db = await getDb();
    const keys = await db.getAll("deviceKeys");
    return keys[0];
}

/**
 * Returns the device key, making one on first use. Returns `undefined` when the
 * browser cannot sign, so the caller can show that signing is off.
 */
export async function ensureDeviceKey(
    subtle: SubtleCrypto | undefined,
): Promise<DeviceKey | undefined> {
    const existing = await loadDeviceKey();
    if (existing) return existing;
    if (!(await ed25519Supported(subtle)) || !subtle) return undefined;
    const key = await createDeviceKey(subtle);
    const db = await getDb();
    await db.put("deviceKeys", key);
    return key;
}
