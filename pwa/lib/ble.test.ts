// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import { describe, expect, it } from "vitest";
import {
    CMD_ACK_SEGMENTS,
    CMD_REQUEST_SEGMENTS,
    FLAG_END_OF_SEGMENT,
    FLAG_END_OF_TRANSFER,
    SegmentAssembler,
    TransferError,
    crc32,
    decodeLiveFix,
    decodeRecord,
    decodeSegmentPayload,
    encodeCommand,
    parseChunk,
} from "./ble";

const hex = (s: string) => Uint8Array.from(s.match(/../g)!.map((b) => parseInt(b, 16)));

// Golden bytes from the firmware encoder (crates/tpt-ruc-firmware/src/payload.rs and log.rs).
// Segment [start 1, end 60001, distance 1.0].
const SEGMENT_PAYLOAD = hex("9301ce0000ea61cb3ff0000000000000");
const SEGMENT_RECORD = hex("a5100046c8bd369301ce0000ea61cb3ff0000000000000");
// Live fix [version 1, 1700000000123 ms, -412865000, 1747762000, nil speed, fix quality 1].
const LIVE_FIX = hex("9601cf0000018bcfe5687bd2e7642e18ce682cbb50c001");

describe("crc32", () => {
    it("matches the standard check value", () => {
        expect(crc32(new TextEncoder().encode("123456789"))).toBe(0xcbf43926);
    });
});

describe("encodeCommand", () => {
    it("writes a bare command", () => {
        expect(Array.from(encodeCommand(CMD_REQUEST_SEGMENTS))).toEqual([0x01]);
    });

    it("writes the acknowledged count as little-endian u32", () => {
        expect(Array.from(encodeCommand(CMD_ACK_SEGMENTS, 0x01020304))).toEqual([
            0x02, 0x04, 0x03, 0x02, 0x01,
        ]);
    });
});

describe("decodeSegmentPayload and decodeRecord", () => {
    it("decodes the golden segment payload", () => {
        expect(decodeSegmentPayload(SEGMENT_PAYLOAD)).toEqual({
            startMs: 1,
            endMs: 60001,
            distanceM: 1.0,
        });
    });

    it("decodes the golden record and checks its CRC", () => {
        expect(decodeRecord(SEGMENT_RECORD)).toEqual({
            startMs: 1,
            endMs: 60001,
            distanceM: 1.0,
        });
    });

    it("rejects a record whose payload was changed", () => {
        const damaged = SEGMENT_RECORD.slice();
        damaged[damaged.length - 1] ^= 0x01;
        expect(() => decodeRecord(damaged)).toThrow(TransferError);
    });

    it("rejects a record with the wrong magic byte", () => {
        const damaged = SEGMENT_RECORD.slice();
        damaged[0] = 0x00;
        expect(() => decodeRecord(damaged)).toThrow("wrong magic byte");
    });
});

describe("parseChunk", () => {
    it("reads the sequence number and flags", () => {
        const chunk = parseChunk(
            Uint8Array.of(0x05, 0x00, FLAG_END_OF_SEGMENT | FLAG_END_OF_TRANSFER, 0xaa),
        );
        expect(chunk.seq).toBe(5);
        expect(chunk.endOfSegment).toBe(true);
        expect(chunk.endOfTransfer).toBe(true);
        expect(Array.from(chunk.data)).toEqual([0xaa]);
    });

    it("rejects a notification shorter than its header", () => {
        expect(() => parseChunk(Uint8Array.of(0x00))).toThrow(TransferError);
    });
});

describe("SegmentAssembler", () => {
    /** Splits a record into chunks of `size` bytes, with the flags a device would set. */
    function chunksOf(record: Uint8Array, size: number, last: boolean): Uint8Array[] {
        const out: Uint8Array[] = [];
        for (let at = 0; at < record.length; at += size) {
            const data = record.subarray(at, at + size);
            const endOfSegment = at + size >= record.length;
            let flags = endOfSegment ? FLAG_END_OF_SEGMENT : 0;
            if (endOfSegment && last) flags |= FLAG_END_OF_TRANSFER;
            out.push(Uint8Array.of(out.length & 0xff, (out.length >> 8) & 0xff, flags, ...data));
        }
        return out;
    }

    it("rebuilds a segment split across several chunks", () => {
        const assembler = new SegmentAssembler();
        for (const bytes of chunksOf(SEGMENT_RECORD, 5, true)) {
            assembler.push(parseChunk(bytes));
        }
        expect(assembler.take()).toEqual([{ startMs: 1, endMs: 60001, distanceM: 1.0 }]);
        expect(assembler.finished).toBe(true);
    });

    it("rebuilds two segments back to back", () => {
        const assembler = new SegmentAssembler();
        // Chunk each segment separately, numbering the chunks across the whole transfer.
        const first = chunksOf(SEGMENT_RECORD, 6, false);
        const second = chunksOf(SEGMENT_RECORD, 6, true).map((c, i) => {
            const seq = first.length + i;
            return Uint8Array.of(seq & 0xff, seq >> 8, c[2], ...c.subarray(3));
        });
        for (const bytes of [...first, ...second]) assembler.push(parseChunk(bytes));
        expect(assembler.take()).toHaveLength(2);
    });

    it("refuses chunks that arrive out of order", () => {
        const assembler = new SegmentAssembler();
        const chunks = chunksOf(SEGMENT_RECORD, 5, true);
        assembler.push(parseChunk(chunks[0]));
        expect(() => assembler.push(parseChunk(chunks[2]))).toThrow("out of order");
    });

    it("refuses a segment whose CRC does not match", () => {
        const assembler = new SegmentAssembler();
        const damaged = SEGMENT_RECORD.slice();
        damaged[damaged.length - 1] ^= 0x01;
        const chunks = chunksOf(damaged, 8, true);
        expect(() => chunks.forEach((c) => assembler.push(parseChunk(c)))).toThrow("CRC");
    });

    it("refuses a transfer that ends in the middle of a segment", () => {
        const assembler = new SegmentAssembler();
        // The final chunk of the record is withheld, but the transfer is flagged as ended.
        const chunks = chunksOf(SEGMENT_RECORD, 5, false);
        const truncated = chunks.slice(0, -1);
        const lastOfTruncated = truncated[truncated.length - 1].slice();
        lastOfTruncated[2] = FLAG_END_OF_TRANSFER;
        truncated[truncated.length - 1] = lastOfTruncated;
        expect(() => truncated.forEach((c) => assembler.push(parseChunk(c)))).toThrow(
            "middle of a segment",
        );
    });
});

describe("decodeLiveFix", () => {
    it("decodes the golden live fix", () => {
        const fix = decodeLiveFix(LIVE_FIX);
        expect(fix.version).toBe(1);
        expect(fix.timeMs).toBe(1700000000123);
        expect(fix.latDegrees).toBeCloseTo(-41.2865, 7);
        expect(fix.lonDegrees).toBeCloseTo(174.7762, 7);
        expect(fix.speedMps).toBeNull();
        expect(fix.fixQuality).toBe(1);
    });
});
