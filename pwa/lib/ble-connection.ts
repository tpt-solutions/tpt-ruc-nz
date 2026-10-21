// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// Web Bluetooth connection to the device. The protocol is in ./ble.ts, and this
// module only moves bytes and tracks state. The Web Bluetooth types are narrowed
// to the calls used here, so tests can stand in a mock for navigator.bluetooth.

import {
    CHAR_CONTROL,
    CHAR_LIVE_FIX,
    CHAR_SEGMENTS,
    CMD_ACK_SEGMENTS,
    CMD_REQUEST_SEGMENTS,
    SERVICE_UUID,
    SegmentAssembler,
    decodeLiveFix,
    encodeCommand,
    parseChunk,
    type LiveFix,
    type Segment,
} from "./ble";

// ---- Capability -------------------------------------------------------------

export type BleSupport = { supported: true } | { supported: false; message: string };

/**
 * Says whether this browser can talk to the device, and why not if it cannot.
 * iOS and Firefox do not offer Web Bluetooth. That is an accepted limit, so the
 * message points users to a browser that works.
 */
export function detectBleSupport(env: {
    bluetooth: unknown;
    isSecureContext: boolean;
    userAgent: string;
}): BleSupport {
    if (!env.isSecureContext) {
        return {
            supported: false,
            message: "Bluetooth needs this page to be served over HTTPS, or from localhost.",
        };
    }
    if (env.bluetooth) return { supported: true };
    if (/iPhone|iPad|iPod/.test(env.userAgent)) {
        return {
            supported: false,
            message:
                "iPhone and iPad browsers, including Safari, cannot use Bluetooth devices. Use a desktop or Android browser such as Chrome or Edge.",
        };
    }
    if (/Firefox\//.test(env.userAgent)) {
        return {
            supported: false,
            message:
                "Firefox does not support Bluetooth devices. Use Chrome, Edge or Opera instead.",
        };
    }
    return {
        supported: false,
        message:
            "This browser does not support Bluetooth devices. Use Chrome, Edge or Opera on a desktop or Android device.",
    };
}

// ---- Adapter types (the subset of Web Bluetooth used here) -------------------

export interface BleCharacteristic {
    startNotifications(): Promise<unknown>;
    addEventListener(
        type: "characteristicvaluechanged",
        listener: (event: { target: { value?: DataView } }) => void,
    ): void;
    writeValue(value: BufferSource): Promise<void>;
}

export interface BleServer {
    readonly connected: boolean;
    connect(): Promise<BleServer>;
    disconnect(): void;
    getPrimaryService(uuid: string): Promise<{
        getCharacteristic(uuid: string): Promise<BleCharacteristic>;
    }>;
}

export interface BleDevice {
    readonly name?: string;
    readonly gatt?: BleServer;
    addEventListener(type: "gattserverdisconnected", listener: () => void): void;
}

export interface BluetoothLike {
    requestDevice(options: { filters: { services: string[] }[] }): Promise<BleDevice>;
}

// ---- Connection state ----------------------------------------------------------

export type LinkState =
    | { kind: "idle" }
    | { kind: "connecting"; attempt: number }
    | { kind: "connected"; name: string }
    | { kind: "error"; message: string };

/** Reconnect attempts after an unexpected disconnect, before the link gives up. */
export const MAX_RECONNECT_ATTEMPTS = 5;
/** Longest wait for a segment transfer to finish. */
export const TRANSFER_TIMEOUT_MS = 60_000;

/** Wait before reconnect attempt `attempt` (1 for the first): doubles each time, capped at 30 seconds. */
export function reconnectDelayMs(attempt: number): number {
    return Math.min(30_000, 1_000 * 2 ** (attempt - 1));
}

export interface LinkOptions {
    /** Called on every state change. */
    onState: (state: LinkState) => void;
    /** Called for each live fix the device sends. */
    onLiveFix: (fix: LiveFix) => void;
    /** Waits between reconnect attempts. Tests replace it. */
    sleep: (ms: number) => Promise<void>;
    /** Transfer timeout. Tests shorten it. */
    transferTimeoutMs?: number;
}

/**
 * One connection to the device. Connect with a user gesture, since browsers only
 * show the device chooser on a click. After an unexpected disconnect, the link
 * retries with backoff and then reports an error.
 */
export class BleLink {
    private device?: BleDevice;
    private control?: BleCharacteristic;
    private segments?: BleCharacteristic;
    private intentionalDisconnect = false;
    private transfer?: {
        assembler: SegmentAssembler;
        resolve: (segments: Segment[]) => void;
        reject: (error: Error) => void;
    };

    constructor(
        private readonly bluetooth: BluetoothLike,
        private readonly options: LinkOptions,
    ) {}

    /** Shows the device chooser, then connects to the chosen device. */
    async connect(): Promise<void> {
        this.intentionalDisconnect = false;
        this.options.onState({ kind: "connecting", attempt: 1 });
        try {
            const device = await this.bluetooth.requestDevice({
                filters: [{ services: [SERVICE_UUID] }],
            });
            this.device = device;
            device.addEventListener("gattserverdisconnected", () => {
                void this.handleDisconnect();
            });
            await this.connectGatt(device);
        } catch (error) {
            this.options.onState({ kind: "error", message: errorText(error) });
        }
    }

    /** Ends the connection on purpose. No reconnect follows. */
    disconnect(): void {
        this.intentionalDisconnect = true;
        this.device?.gatt?.disconnect();
        this.options.onState({ kind: "idle" });
    }

    /**
     * Asks the device for its stored segments, waits for the whole transfer and
     * returns the segments. Call `acknowledge` only after they are saved, since
     * the device drops acknowledged segments from its queue.
     */
    async downloadSegments(): Promise<Segment[]> {
        if (!this.segments || !this.control) {
            throw new Error("Connect to the device before downloading segments");
        }
        const timeoutMs = this.options.transferTimeoutMs ?? TRANSFER_TIMEOUT_MS;
        const finished = new Promise<Segment[]>((resolve, reject) => {
            const timer = setTimeout(() => {
                this.transfer = undefined;
                reject(new Error("The device stopped sending segments"));
            }, timeoutMs);
            this.transfer = {
                assembler: new SegmentAssembler(),
                resolve: (value) => {
                    clearTimeout(timer);
                    resolve(value);
                },
                reject: (error) => {
                    clearTimeout(timer);
                    reject(error);
                },
            };
        });
        // Stop the timer and the wait if the request itself cannot be sent.
        finished.catch(() => undefined);
        try {
            await this.control.writeValue(encodeCommand(CMD_REQUEST_SEGMENTS).slice());
        } catch (error) {
            this.transfer?.reject(error instanceof Error ? error : new Error(String(error)));
            this.transfer = undefined;
        }
        return finished;
    }

    /** Tells the device how many segments are saved, so it can drop them from its queue. */
    async acknowledge(count: number): Promise<void> {
        if (!this.control) throw new Error("Connect to the device before acknowledging");
        await this.control.writeValue(encodeCommand(CMD_ACK_SEGMENTS, count).slice());
    }

    private async connectGatt(device: BleDevice): Promise<void> {
        if (!device.gatt) throw new Error("The device does not offer a GATT connection");
        const server = await device.gatt.connect();
        const service = await server.getPrimaryService(SERVICE_UUID);
        const live = await service.getCharacteristic(CHAR_LIVE_FIX);
        this.control = await service.getCharacteristic(CHAR_CONTROL);
        this.segments = await service.getCharacteristic(CHAR_SEGMENTS);
        // One listener for the life of the connection. It only acts during a transfer.
        this.segments.addEventListener("characteristicvaluechanged", (event) => {
            this.onSegmentChunk(event.target.value);
        });
        await this.segments.startNotifications();

        live.addEventListener("characteristicvaluechanged", (event) => {
            if (!event.target.value) return;
            const bytes = new Uint8Array(
                event.target.value.buffer,
                event.target.value.byteOffset,
                event.target.value.byteLength,
            );
            try {
                this.options.onLiveFix(decodeLiveFix(bytes));
            } catch {
                // A malformed live fix is dropped. The next one arrives within a second.
            }
        });
        await live.startNotifications();
        this.options.onState({ kind: "connected", name: device.name ?? "device" });
    }

    private onSegmentChunk(value: DataView | undefined): void {
        const transfer = this.transfer;
        if (!transfer || !value) return;
        try {
            const chunk = parseChunk(
                new Uint8Array(value.buffer, value.byteOffset, value.byteLength),
            );
            transfer.assembler.push(chunk);
            if (transfer.assembler.finished) {
                this.transfer = undefined;
                transfer.resolve(transfer.assembler.take());
            }
        } catch (error) {
            this.transfer = undefined;
            transfer.reject(error instanceof Error ? error : new Error(String(error)));
        }
    }

    private async handleDisconnect(): Promise<void> {
        if (this.intentionalDisconnect || !this.device) return;
        this.transfer?.reject(new Error("The device disconnected during a transfer"));
        this.transfer = undefined;
        for (let attempt = 1; attempt <= MAX_RECONNECT_ATTEMPTS; attempt++) {
            this.options.onState({ kind: "connecting", attempt });
            await this.options.sleep(reconnectDelayMs(attempt));
            if (this.intentionalDisconnect) return;
            try {
                await this.connectGatt(this.device);
                return;
            } catch {
                // Try again after the next, longer wait.
            }
        }
        this.options.onState({
            kind: "error",
            message: `Could not reconnect after ${MAX_RECONNECT_ATTEMPTS} attempts. Press Connect to try again.`,
        });
    }
}

function errorText(error: unknown): string {
    // The chooser reports a cancelled request as NotFoundError.
    if (error instanceof Error && error.name === "NotFoundError") {
        return "No device was chosen.";
    }
    return error instanceof Error ? error.message : String(error);
}

/** What a download saved. */
export interface DownloadResult {
    received: number;
    added: number;
    duplicates: number;
}

/**
 * Downloads the device's stored segments, saves them, then acknowledges them.
 * The acknowledgement comes last, so a failure while saving never lets the device
 * drop segments that were not kept.
 */
export async function downloadToStore(
    link: Pick<BleLink, "downloadSegments" | "acknowledge">,
    save: (segments: Segment[]) => Promise<{ added: number; duplicates: number }>,
): Promise<DownloadResult> {
    const segments = await link.downloadSegments();
    const { added, duplicates } = await save(segments);
    await link.acknowledge(segments.length);
    return { received: segments.length, added, duplicates };
}
