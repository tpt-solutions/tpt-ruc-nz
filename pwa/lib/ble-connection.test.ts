// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import { describe, expect, it } from "vitest";
import {
    CHAR_CONTROL,
    CHAR_LIVE_FIX,
    CHAR_SEGMENTS,
    CMD_ACK_SEGMENTS,
    CMD_REQUEST_SEGMENTS,
    FLAG_END_OF_SEGMENT,
    FLAG_END_OF_TRANSFER,
    SERVICE_UUID,
} from "./ble";
import {
    BleLink,
    MAX_RECONNECT_ATTEMPTS,
    downloadToStore,
    detectBleSupport,
    reconnectDelayMs,
    type BleCharacteristic,
    type BleDevice,
    type BluetoothLike,
    type LinkState,
} from "./ble-connection";

const hex = (s: string) => Uint8Array.from(s.match(/../g)!.map((b) => parseInt(b, 16)));
/** One stored segment: start 1 ms, end 60001 ms, 1 m. */
const SEGMENT_RECORD = hex("a5100046c8bd369301ce0000ea61cb3ff0000000000000");
const LIVE_FIX = hex("9601cf0000018bcfe5687bd2e7642e18ce682cbb50c001");

/** A characteristic that records writes and lets the test send notifications. */
class FakeCharacteristic implements BleCharacteristic {
    writes: Uint8Array[] = [];
    notifying = false;
    private listeners: ((event: { target: { value?: DataView } }) => void)[] = [];
    failWrites = false;

    async startNotifications() {
        this.notifying = true;
        return this;
    }
    addEventListener(
        _type: "characteristicvaluechanged",
        listener: (event: { target: { value?: DataView } }) => void,
    ) {
        this.listeners.push(listener);
    }
    async writeValue(value: BufferSource) {
        if (this.failWrites) throw new Error("write refused");
        const view = value as ArrayBufferView;
        this.writes.push(new Uint8Array(view.buffer, view.byteOffset, view.byteLength));
    }
    notify(bytes: Uint8Array) {
        const value = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
        for (const listener of this.listeners) listener({ target: { value } });
    }
}

/** A device with a GATT server that can be connected, dropped and reconnected. */
function fakeBluetooth(options: { refuseConnect?: boolean } = {}) {
    const live = new FakeCharacteristic();
    const control = new FakeCharacteristic();
    const segments = new FakeCharacteristic();
    const characteristics: Record<string, FakeCharacteristic> = {
        [CHAR_LIVE_FIX]: live,
        [CHAR_CONTROL]: control,
        [CHAR_SEGMENTS]: segments,
    };
    let disconnectListener: (() => void) | undefined;
    let connectCalls = 0;
    const server = {
        connected: true,
        async connect() {
            connectCalls++;
            if (options.refuseConnect) throw new Error("device out of range");
            server.connected = true;
            return server;
        },
        disconnect() {
            server.connected = false;
        },
        async getPrimaryService(uuid: string) {
            expect(uuid).toBe(SERVICE_UUID);
            return {
                async getCharacteristic(id: string) {
                    return characteristics[id];
                },
            };
        },
    };
    const device: BleDevice = {
        name: "tpt-ruc-node",
        gatt: server,
        addEventListener(_type, listener) {
            disconnectListener = listener;
        },
    };
    let requests = 0;
    const bluetooth: BluetoothLike = {
        async requestDevice(options) {
            requests++;
            expect(options.filters).toEqual([{ services: [SERVICE_UUID] }]);
            return device;
        },
    };
    return {
        bluetooth,
        live,
        control,
        segments,
        drop: () => {
            server.connected = false;
            disconnectListener?.();
        },
        connectCalls: () => connectCalls,
        requests: () => requests,
    };
}

function linkWith(bluetooth: BluetoothLike, overrides: { transferTimeoutMs?: number } = {}) {
    const states: LinkState[] = [];
    const fixes: unknown[] = [];
    const sleeps: number[] = [];
    const link = new BleLink(bluetooth, {
        onState: (state) => states.push(state),
        onLiveFix: (fix) => fixes.push(fix),
        sleep: async (ms) => {
            sleeps.push(ms);
        },
        ...overrides,
    });
    return { link, states, fixes, sleeps };
}

describe("detectBleSupport", () => {
    const chrome = "Mozilla/5.0 Chrome/140.0 Safari/537.36";

    it("accepts a secure page with Web Bluetooth", () => {
        expect(
            detectBleSupport({ bluetooth: {}, isSecureContext: true, userAgent: chrome }),
        ).toEqual({ supported: true });
    });

    it("explains iOS Safari and iOS Chrome, which have no Web Bluetooth", () => {
        const ios = detectBleSupport({
            bluetooth: undefined,
            isSecureContext: true,
            userAgent: "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0) Version/18.0 Mobile/15E148 Safari",
        });
        expect(ios.supported).toBe(false);
        expect(ios.supported === false && ios.message).toContain("iPhone and iPad");
    });

    it("explains Firefox", () => {
        const firefox = detectBleSupport({
            bluetooth: undefined,
            isSecureContext: true,
            userAgent: "Mozilla/5.0 Gecko/20100101 Firefox/132.0",
        });
        expect(firefox.supported === false && firefox.message).toContain("Firefox");
    });

    it("explains an insecure page before anything else", () => {
        const insecure = detectBleSupport({
            bluetooth: {},
            isSecureContext: false,
            userAgent: chrome,
        });
        expect(insecure.supported === false && insecure.message).toContain("HTTPS");
    });
});

describe("reconnectDelayMs", () => {
    it("backs off from one second to a 30 second cap", () => {
        expect(reconnectDelayMs(1)).toBe(1_000);
        expect(reconnectDelayMs(3)).toBe(4_000);
        expect(reconnectDelayMs(10)).toBe(30_000);
    });
});

describe("BleLink connect", () => {
    it("requests the device by service UUID and reports connected with its name", async () => {
        const fake = fakeBluetooth();
        const { link, states } = linkWith(fake.bluetooth);
        await link.connect();
        expect(fake.requests()).toBe(1);
        expect(states.at(-1)).toEqual({ kind: "connected", name: "tpt-ruc-node" });
        expect(fake.live.notifying).toBe(true);
        expect(fake.segments.notifying).toBe(true);
    });

    it("reports an error when the chooser is cancelled", async () => {
        const cancelled: BluetoothLike = {
            async requestDevice() {
                throw Object.assign(new Error("cancelled"), { name: "NotFoundError" });
            },
        };
        const { link, states } = linkWith(cancelled);
        await link.connect();
        expect(states.at(-1)).toEqual({ kind: "error", message: "No device was chosen." });
    });

    it("reports an error when the device cannot be reached", async () => {
        const fake = fakeBluetooth({ refuseConnect: true });
        const { link, states } = linkWith(fake.bluetooth);
        await link.connect();
        expect(states.at(-1)).toEqual({ kind: "error", message: "device out of range" });
    });

    it("passes each live fix to the callback, and skips malformed ones", async () => {
        const fake = fakeBluetooth();
        const { link, fixes } = linkWith(fake.bluetooth);
        await link.connect();
        fake.live.notify(LIVE_FIX);
        fake.live.notify(hex("00"));
        expect(fixes).toHaveLength(1);
        expect(fixes[0]).toMatchObject({ version: 1 });
    });
});

describe("BleLink segment download", () => {
    it("requests segments, returns what arrives, and acknowledges only when asked", async () => {
        const fake = fakeBluetooth();
        const { link } = linkWith(fake.bluetooth);
        await link.connect();

        const download = link.downloadSegments();
        expect(fake.control.writes[0]).toEqual(Uint8Array.of(CMD_REQUEST_SEGMENTS));
        fake.segments.notify(
            Uint8Array.from([0, 0, FLAG_END_OF_SEGMENT | FLAG_END_OF_TRANSFER, ...SEGMENT_RECORD]),
        );
        const segments = await download;
        expect(segments).toEqual([{ startMs: 1, endMs: 60_001, distanceM: 1 }]);
        expect(fake.control.writes).toHaveLength(1);

        await link.acknowledge(segments.length);
        expect(fake.control.writes[1]).toEqual(hex("0201000000"));
        expect(fake.control.writes[1][0]).toBe(CMD_ACK_SEGMENTS);
    });

    it("fails a transfer that stops early", async () => {
        const fake = fakeBluetooth();
        const { link } = linkWith(fake.bluetooth, { transferTimeoutMs: 5 });
        await link.connect();
        await expect(link.downloadSegments()).rejects.toThrow("stopped sending");
    });

    it("fails a transfer with a corrupt record, rather than saving it", async () => {
        const fake = fakeBluetooth();
        const { link } = linkWith(fake.bluetooth);
        await link.connect();
        const download = link.downloadSegments();
        const corrupt = Uint8Array.from(SEGMENT_RECORD);
        corrupt[corrupt.length - 1] ^= 0xff;
        fake.segments.notify(
            Uint8Array.from([0, 0, FLAG_END_OF_SEGMENT | FLAG_END_OF_TRANSFER, ...corrupt]),
        );
        await expect(download).rejects.toThrow("CRC");
    });

    it("refuses to download before connecting", async () => {
        const fake = fakeBluetooth();
        const { link } = linkWith(fake.bluetooth);
        await expect(link.downloadSegments()).rejects.toThrow("Connect to the device");
    });

    it("sends a write failure to the caller instead of leaving the download hanging", async () => {
        const fake = fakeBluetooth();
        const { link } = linkWith(fake.bluetooth, { transferTimeoutMs: 1_000 });
        await link.connect();
        fake.control.failWrites = true;
        await expect(link.downloadSegments()).rejects.toThrow("write refused");
    });
});

describe("BleLink reconnect", () => {
    it("reconnects after an unexpected drop, with backoff", async () => {
        const fake = fakeBluetooth();
        const { link, states, sleeps } = linkWith(fake.bluetooth);
        await link.connect();
        fake.drop();
        await new Promise((resolve) => setTimeout(resolve, 0));
        expect(sleeps).toEqual([1_000]);
        expect(states.at(-1)).toEqual({ kind: "connected", name: "tpt-ruc-node" });
        expect(fake.connectCalls()).toBe(2);
    });

    it("gives up with a clear message after the maximum attempts", async () => {
        const fake = fakeBluetooth();
        const { link, states, sleeps } = linkWith(fake.bluetooth);
        await link.connect();
        // Make every later connect fail.
        const failing = fakeBluetooth({ refuseConnect: true });
        const device = await failing.bluetooth.requestDevice({
            filters: [{ services: [SERVICE_UUID] }],
        });
        (link as unknown as { device: BleDevice }).device = device;
        fake.drop();
        await new Promise((resolve) => setTimeout(resolve, 0));
        expect(sleeps).toHaveLength(MAX_RECONNECT_ATTEMPTS);
        expect(states.at(-1)).toEqual({
            kind: "error",
            message: `Could not reconnect after ${MAX_RECONNECT_ATTEMPTS} attempts. Press Connect to try again.`,
        });
    });

    it("does not reconnect after the user disconnects on purpose", async () => {
        const fake = fakeBluetooth();
        const { link, sleeps, states } = linkWith(fake.bluetooth);
        await link.connect();
        link.disconnect();
        fake.drop();
        await new Promise((resolve) => setTimeout(resolve, 0));
        expect(sleeps).toHaveLength(0);
        expect(states.at(-1)).toEqual({ kind: "idle" });
    });
});

describe("downloadToStore", () => {
    it("saves the segments before acknowledging them", async () => {
        const order: string[] = [];
        const link = {
            async downloadSegments() {
                order.push("download");
                return [{ startMs: 1, endMs: 2, distanceM: 3 }];
            },
            async acknowledge(count: number) {
                order.push(`ack:${count}`);
            },
        };
        const result = await downloadToStore(link, async (segments) => {
            order.push(`save:${segments.length}`);
            return { added: 1, duplicates: 0 };
        });
        expect(order).toEqual(["download", "save:1", "ack:1"]);
        expect(result).toEqual({ received: 1, added: 1, duplicates: 0 });
    });

    it("does not acknowledge when saving fails, so the device keeps its segments", async () => {
        let acknowledged = false;
        const link = {
            async downloadSegments() {
                return [{ startMs: 1, endMs: 2, distanceM: 3 }];
            },
            async acknowledge() {
                acknowledged = true;
            },
        };
        await expect(
            downloadToStore(link, async () => {
                throw new Error("database full");
            }),
        ).rejects.toThrow("database full");
        expect(acknowledged).toBe(false);
    });
});
