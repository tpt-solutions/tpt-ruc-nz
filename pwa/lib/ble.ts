// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// Protocol handling for the device's BLE service (docs/ble-design.md, version 1).
// This module is pure. It does not touch Web Bluetooth, so it can be tested without a board.

export const SERVICE_UUID = "7b0e0001-5f4c-4a51-9c3d-7a6b5c4d3e2f";
export const CHAR_LIVE_FIX = "7b0e0002-5f4c-4a51-9c3d-7a6b5c4d3e2f";
export const CHAR_SEGMENTS = "7b0e0003-5f4c-4a51-9c3d-7a6b5c4d3e2f";
export const CHAR_CONTROL = "7b0e0004-5f4c-4a51-9c3d-7a6b5c4d3e2f";
export const CHAR_DEVICE_INFO = "7b0e0005-5f4c-4a51-9c3d-7a6b5c4d3e2f";

/** Chunk header: seq (u16 LE) and flags (u8). */
export const CHUNK_HEADER_LEN = 3;
/** Set on the last chunk of one segment. */
export const FLAG_END_OF_SEGMENT = 0x01;
/** Set on the last chunk of the transfer. */
export const FLAG_END_OF_TRANSFER = 0x02;

/** Segment log record: magic, payload length (u16 LE), CRC-32 (u32 LE). Matches the firmware log. */
export const RECORD_MAGIC = 0xa5;
export const RECORD_HEADER_LEN = 7;

export const CMD_REQUEST_SEGMENTS = 0x01;
export const CMD_ACK_SEGMENTS = 0x02;
export const CMD_SET_GNSS_RATE = 0x10;
export const RESP_OK = 0x80;
export const RESP_UNKNOWN = 0x8f;

export interface Segment {
    startMs: number;
    endMs: number;
    distanceM: number;
}

export interface Chunk {
    seq: number;
    endOfSegment: boolean;
    endOfTransfer: boolean;
    data: Uint8Array;
}

/** Errors from a malformed or corrupt transfer. */
export class TransferError extends Error {}

/** Standard CRC-32 (IEEE, reflected). Same polynomial as the firmware log. */
export function crc32(data: Uint8Array): number {
    let crc = 0xffffffff;
    for (const byte of data) {
        crc ^= byte;
        for (let bit = 0; bit < 8; bit++) {
            crc = crc & 1 ? (crc >>> 1) ^ 0xedb88320 : crc >>> 1;
        }
    }
    return (crc ^ 0xffffffff) >>> 0;
}

/** Builds a control command. The arguments are written little-endian. */
export function encodeCommand(command: number, argument?: number): Uint8Array {
    if (argument === undefined) return Uint8Array.of(command);
    if (command === CMD_ACK_SEGMENTS) {
        const out = new Uint8Array(5);
        out[0] = command;
        new DataView(out.buffer).setUint32(1, argument, true);
        return out;
    }
    return Uint8Array.of(command, argument & 0xff);
}

/** Reads one segment-transfer notification. */
export function parseChunk(bytes: Uint8Array): Chunk {
    if (bytes.length < CHUNK_HEADER_LEN) {
        throw new TransferError("chunk is shorter than its header");
    }
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    const flags = bytes[2];
    return {
        seq: view.getUint16(0, true),
        endOfSegment: (flags & FLAG_END_OF_SEGMENT) !== 0,
        endOfTransfer: (flags & FLAG_END_OF_TRANSFER) !== 0,
        data: bytes.subarray(CHUNK_HEADER_LEN),
    };
}

/**
 * Rebuilds segments from chunks. Checks the chunk order and each record's CRC.
 * Feed chunks in the order they arrive. A transfer that breaks the order or the CRC throws.
 */
export class SegmentAssembler {
    private expectedSeq = 0;
    private pending: number[] = [];
    private segments: Segment[] = [];
    private done = false;

    push(chunk: Chunk): void {
        if (this.done) throw new TransferError("chunk received after the transfer ended");
        if (chunk.seq !== this.expectedSeq) {
            throw new TransferError(
                `chunk ${chunk.seq} out of order, expected ${this.expectedSeq}`,
            );
        }
        this.expectedSeq++;
        for (const byte of chunk.data) this.pending.push(byte);

        if (chunk.endOfSegment) {
            this.segments.push(decodeRecord(Uint8Array.from(this.pending)));
            this.pending = [];
        }
        if (chunk.endOfTransfer) {
            if (this.pending.length > 0) {
                throw new TransferError("transfer ended in the middle of a segment");
            }
            this.done = true;
        }
    }

    /** Segments completed so far, oldest first. */
    take(): Segment[] {
        const out = this.segments;
        this.segments = [];
        return out;
    }

    get finished(): boolean {
        return this.done;
    }
}

/** Decodes one segment log record and checks its magic, length and CRC. */
export function decodeRecord(record: Uint8Array): Segment {
    if (record.length < RECORD_HEADER_LEN) throw new TransferError("record is too short");
    if (record[0] !== RECORD_MAGIC) throw new TransferError("record has the wrong magic byte");
    const view = new DataView(record.buffer, record.byteOffset, record.byteLength);
    const payloadLen = view.getUint16(1, true);
    const storedCrc = view.getUint32(3, true);
    const payload = record.subarray(RECORD_HEADER_LEN, RECORD_HEADER_LEN + payloadLen);
    if (payload.length !== payloadLen) throw new TransferError("record payload is truncated");
    if (crc32(payload) !== storedCrc) throw new TransferError("record CRC does not match");
    return decodeSegmentPayload(payload);
}

/**
 * Decodes the MessagePack payload `[start_ms, end_ms, distance_m]`.
 * Supports only the types the firmware writes: fixarray, unsigned integers and float64.
 */
export function decodeSegmentPayload(payload: Uint8Array): Segment {
    const view = new DataView(payload.buffer, payload.byteOffset, payload.byteLength);
    let at = 0;
    const header = payload[at++];
    if (header !== 0x93) throw new TransferError("segment payload is not a 3-element array");

    const readUint = (): number => {
        const marker = payload[at++];
        if (marker <= 0x7f) return marker;
        if (marker === 0xcc) return view.getUint8(at++);
        if (marker === 0xcd) {
            at += 2;
            return view.getUint16(at - 2, false);
        }
        if (marker === 0xce) {
            at += 4;
            return view.getUint32(at - 4, false);
        }
        if (marker === 0xcf) {
            at += 8;
            return Number(view.getBigUint64(at - 8, false));
        }
        throw new TransferError(`unexpected MessagePack marker 0x${marker.toString(16)}`);
    };

    const startMs = readUint();
    const endMs = readUint();
    if (payload[at] !== 0xcb) throw new TransferError("distance is not a float64");
    at++;
    const distanceM = view.getFloat64(at, false);
    return { startMs, endMs, distanceM };
}

/** Reads the live-fix notification's payload, version 1. Returns the raw fields. */
export interface LiveFix {
    version: number;
    timeMs: number;
    latDegrees: number;
    lonDegrees: number;
    speedMps: number | null;
    fixQuality: number;
}

export function decodeLiveFix(payload: Uint8Array): LiveFix {
    let at = 0;
    const view = new DataView(payload.buffer, payload.byteOffset, payload.byteLength);
    if (payload[at++] !== 0x96) throw new TransferError("live fix is not a 6-element array");

    const readUint = (): number => {
        const marker = payload[at++];
        if (marker <= 0x7f) return marker;
        if (marker === 0xcc) return view.getUint8(at++);
        if (marker === 0xcd) {
            at += 2;
            return view.getUint16(at - 2, false);
        }
        if (marker === 0xce) {
            at += 4;
            return view.getUint32(at - 4, false);
        }
        if (marker === 0xcf) {
            at += 8;
            return Number(view.getBigUint64(at - 8, false));
        }
        throw new TransferError(`unexpected MessagePack marker 0x${marker.toString(16)}`);
    };
    const readSint = (): number => {
        const marker = payload[at];
        if (marker >= 0xe0) {
            at++;
            return marker - 0x100;
        }
        if (marker === 0xd2) {
            at += 5;
            return view.getInt32(at - 4, false);
        }
        if (marker === 0xd3) {
            at += 9;
            return Number(view.getBigInt64(at - 8, false));
        }
        return readUint();
    };

    const version = readUint();
    const timeMs = readUint();
    const latE7 = readSint();
    const lonE7 = readSint();
    let speedMps: number | null = null;
    if (payload[at] === 0xc0) {
        at++;
    } else {
        speedMps = readUint() / 100;
    }
    const fixQuality = readUint();
    return {
        version,
        timeMs,
        latDegrees: latE7 / 1e7,
        lonDegrees: lonE7 / 1e7,
        speedMps,
        fixQuality,
    };
}
