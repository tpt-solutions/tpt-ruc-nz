// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import { describe, expect, it } from "vitest";
import { createDeviceKey, ed25519Supported, signText, toBase64 } from "./device-key";

/** A stand-in for SubtleCrypto that records how its keys were made. */
function fakeSubtle(options: { supported?: boolean } = {}) {
    const calls: { op: string; extractable?: boolean }[] = [];
    const subtle = {
        async generateKey(_alg: unknown, extractable: boolean) {
            calls.push({ op: "generateKey", extractable });
            if (options.supported === false) throw new Error("Ed25519 not supported");
            return { publicKey: { kind: "public" }, privateKey: { kind: "private" } };
        },
        async exportKey(format: string) {
            calls.push({ op: `export:${format}` });
            return new Uint8Array([1, 2, 3, 4]).buffer;
        },
        async sign(_alg: unknown, key: { kind: string }, data: BufferSource) {
            calls.push({ op: `sign:${key.kind}` });
            return new Uint8Array([9, 8, 7, ...new Uint8Array(data as ArrayBuffer).slice(0, 1)])
                .buffer;
        },
    };
    return { subtle: subtle as unknown as SubtleCrypto, calls };
}

describe("toBase64", () => {
    it("encodes bytes as standard base64", () => {
        expect(toBase64(new Uint8Array([72, 105]))).toBe("SGk=");
        expect(toBase64(new Uint8Array([]))).toBe("");
    });
});

describe("ed25519Supported", () => {
    it("is false when there is no WebCrypto at all", async () => {
        expect(await ed25519Supported(undefined)).toBe(false);
    });

    it("is false when the browser refuses to make the key", async () => {
        expect(await ed25519Supported(fakeSubtle({ supported: false }).subtle)).toBe(false);
    });

    it("is true when a key can be made", async () => {
        expect(await ed25519Supported(fakeSubtle().subtle)).toBe(true);
    });
});

describe("createDeviceKey", () => {
    it("keeps the private key non-extractable and stores only the public bytes as text", async () => {
        const { subtle, calls } = fakeSubtle();
        const key = await createDeviceKey(subtle, 1234);
        expect(calls[0]).toEqual({ op: "generateKey", extractable: false });
        expect(key.publicKeyBase64).toBe(toBase64(new Uint8Array([1, 2, 3, 4])));
        expect(key.privateKey).toEqual({ kind: "private" });
        expect(key.createdMs).toBe(1234);
        expect(key.id).toMatch(/^dk_[A-Za-z0-9]+$/);
    });
});

describe("signText", () => {
    it("signs with the private half and returns base64", async () => {
        const { subtle, calls } = fakeSubtle();
        const key = await createDeviceKey(subtle, 1);
        const signature = await signText(subtle, key, "payload");
        expect(calls.at(-1)).toEqual({ op: "sign:private" });
        expect(signature).toBe(toBase64(new Uint8Array([9, 8, 7, 112])));
    });

    it("refuses when the key has no private half", async () => {
        const { subtle } = fakeSubtle();
        await expect(
            signText(subtle, { id: "dk_x", publicKeyBase64: "", createdMs: 0 }, "x"),
        ).rejects.toThrow("cannot sign");
    });
});
