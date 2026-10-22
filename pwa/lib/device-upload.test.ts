// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import { describe, expect, it } from "vitest";
import type { DeviceKey } from "./db";
import {
    canonicalJson,
    makeDeviceSigner,
    publicKeyHex,
    signingMessage,
    type DeviceState,
    type DeviceStateStore,
} from "./device-upload";
import type { UploadTrip } from "./sync";

const TRIP: UploadTrip = {
    id: "t1",
    date: "2026-10-01",
    metres: 1000,
    vehicle: { rate_type: "1", gross_kg: 3000, electric: false },
};

function base64Of(bytes: number[]): string {
    return Buffer.from(bytes).toString("base64");
}

/** A key whose public half is 32 bytes of `fill`, with a stand-in private key. */
function fakeKey(options: { fill?: number; canSign?: boolean } = {}): DeviceKey {
    return {
        id: "dk_test",
        publicKeyBase64: base64Of(new Array(32).fill(options.fill ?? 9)),
        privateKey: options.canSign === false ? undefined : ({} as CryptoKey),
        createdMs: 0,
    };
}

function memoryState(
    initial: Partial<DeviceState> = {},
): DeviceStateStore & { value: DeviceState } {
    const store = {
        value: {
            deviceId: "",
            deviceKeyId: "",
            deviceBackendUrl: "",
            deviceSequence: 0,
            ...initial,
        },
        async read() {
            return { ...store.value };
        },
        async write(patch: Partial<DeviceState>) {
            store.value = { ...store.value, ...patch };
        },
    };
    return store;
}

/** A WebCrypto stand-in that returns a 64-byte signature filled with 7. */
const fakeSubtle = {
    async sign() {
        return new Uint8Array(64).fill(7).buffer;
    },
} as unknown as SubtleCrypto;

function fetchReturning(status: number, body: unknown) {
    const calls: { url: string; init?: RequestInit }[] = [];
    const fetchFn = (async (input: RequestInfo | URL, init?: RequestInit) => {
        calls.push({ url: String(input), init });
        return new Response(JSON.stringify(body), {
            status,
            headers: { "Content-Type": "application/json" },
        });
    }) as unknown as typeof fetch;
    return { fetchFn, calls };
}

describe("canonicalJson", () => {
    it("sorts object keys at every level and keeps array order", () => {
        expect(canonicalJson([{ b: 1, a: { d: 2, c: [3, { z: 1, y: 2 }] } }])).toBe(
            '[{"a":{"c":[3,{"y":2,"z":1}],"d":2},"b":1}]',
        );
    });

    it("is compact", () => {
        expect(canonicalJson({ a: [1, 2] })).toBe('{"a":[1,2]}');
    });
});

describe("signingMessage", () => {
    it("uses the backend's domain prefix and the device, sequence and trips", () => {
        const text = new TextDecoder().decode(signingMessage("d_1", 42, "[]"));
        expect(text).toBe("tpt-ruc-trips-v1\nd_1\n42\n[]");
    });
});

describe("publicKeyHex", () => {
    it("gives the raw public key as lower-case hex", () => {
        const key = { ...fakeKey(), publicKeyBase64: base64Of([0, 1, 255]) };
        expect(publicKeyHex(key)).toBe("0001ff");
    });
});

describe("makeDeviceSigner", () => {
    it("registers the device once, then signs with rising sequence numbers", async () => {
        const state = memoryState();
        const { fetchFn, calls } = fetchReturning(201, { id: "d_1" });
        const sign = makeDeviceSigner({
            subtle: fakeSubtle,
            key: fakeKey(),
            baseUrl: "http://backend.test",
            apiKey: "user-token",
            fetch: fetchFn,
            state,
        });

        const first = await sign([TRIP]);
        const second = await sign([TRIP]);

        expect(calls).toHaveLength(1);
        expect(calls[0].url).toBe("http://backend.test/v1/devices");
        expect(JSON.parse(String(calls[0].init?.body))).toEqual({
            name: "PWA browser",
            public_key: "09".repeat(32),
        });
        expect(first).toEqual({
            "X-Device-Id": "d_1",
            "X-Device-Sequence": "1",
            "X-Device-Signature": "07".repeat(64),
        });
        expect(second["X-Device-Sequence"]).toBe("2");
        expect(state.value.deviceSequence).toBe(2);
    });

    it("reuses a registration made for the same key and backend", async () => {
        const state = memoryState({
            deviceId: "d_old",
            deviceKeyId: "dk_test",
            deviceBackendUrl: "http://backend.test",
        });
        const { fetchFn, calls } = fetchReturning(201, { id: "d_new" });
        const sign = makeDeviceSigner({
            subtle: fakeSubtle,
            key: fakeKey(),
            baseUrl: "http://backend.test",
            apiKey: "user-token",
            fetch: fetchFn,
            state,
        });
        expect((await sign([TRIP]))["X-Device-Id"]).toBe("d_old");
        expect(calls).toHaveLength(0);
    });

    it("registers again when the backend changes", async () => {
        const state = memoryState({
            deviceId: "d_old",
            deviceKeyId: "dk_test",
            deviceBackendUrl: "http://old.test",
        });
        const { fetchFn } = fetchReturning(201, { id: "d_new" });
        const sign = makeDeviceSigner({
            subtle: fakeSubtle,
            key: fakeKey(),
            baseUrl: "http://backend.test",
            apiKey: "user-token",
            fetch: fetchFn,
            state,
        });
        expect((await sign([TRIP]))["X-Device-Id"]).toBe("d_new");
        expect(state.value.deviceBackendUrl).toBe("http://backend.test");
    });

    it("sends the upload unsigned when registration is refused, as with the owner key", async () => {
        const state = memoryState();
        const { fetchFn } = fetchReturning(400, { error: "user_id is required" });
        const sign = makeDeviceSigner({
            subtle: fakeSubtle,
            key: fakeKey(),
            baseUrl: "http://backend.test",
            apiKey: "owner-key",
            fetch: fetchFn,
            state,
        });
        expect(await sign([TRIP])).toEqual({});
        expect(state.value.deviceSequence).toBe(0);
    });

    it("sends the upload unsigned when the key cannot sign", async () => {
        const { fetchFn, calls } = fetchReturning(201, { id: "d_1" });
        const sign = makeDeviceSigner({
            subtle: fakeSubtle,
            key: fakeKey({ canSign: false }),
            baseUrl: "http://backend.test",
            apiKey: "user-token",
            fetch: fetchFn,
            state: memoryState(),
        });
        expect(await sign([TRIP])).toEqual({});
        expect(calls).toHaveLength(0);
    });
});
