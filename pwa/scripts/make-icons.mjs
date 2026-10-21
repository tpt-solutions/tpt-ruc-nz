// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions
//
// Writes the PNG app icons from the same mark as public/icon.svg, using only
// Node's built-in zlib. Run with `node scripts/make-icons.mjs`.

import { deflateSync } from "node:zlib";
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const publicDir = join(here, "..", "public");

const BACKGROUND = [0x1f, 0x29, 0x37, 255];
const FOREGROUND = [0xf9, 0xfa, 0xfb, 255];

// Triangle from the 64-unit icon grid: (12,44), (32,16), (52,44).
const TRIANGLE = [
    [12, 44],
    [32, 16],
    [52, 44],
];

function sign(px, py, [ax, ay], [bx, by]) {
    return (px - bx) * (ay - by) - (ax - bx) * (py - by);
}

function insideTriangle(x, y) {
    const d1 = sign(x, y, TRIANGLE[0], TRIANGLE[1]);
    const d2 = sign(x, y, TRIANGLE[1], TRIANGLE[2]);
    const d3 = sign(x, y, TRIANGLE[2], TRIANGLE[0]);
    const hasNeg = d1 < 0 || d2 < 0 || d3 < 0;
    const hasPos = d1 > 0 || d2 > 0 || d3 > 0;
    return !(hasNeg && hasPos);
}

function renderRgba(size) {
    const scale = 64 / size;
    const rows = [];
    for (let y = 0; y < size; y++) {
        const row = Buffer.alloc(1 + size * 4);
        row[0] = 0; // filter type: none
        for (let x = 0; x < size; x++) {
            const colour = insideTriangle((x + 0.5) * scale, (y + 0.5) * scale)
                ? FOREGROUND
                : BACKGROUND;
            row.set(colour, 1 + x * 4);
        }
        rows.push(row);
    }
    return Buffer.concat(rows);
}

const CRC_TABLE = (() => {
    const table = new Uint32Array(256);
    for (let n = 0; n < 256; n++) {
        let c = n;
        for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
        table[n] = c >>> 0;
    }
    return table;
})();

function crc32(buffer) {
    let c = 0xffffffff;
    for (const byte of buffer) c = CRC_TABLE[(c ^ byte) & 0xff] ^ (c >>> 8);
    return (c ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
    const length = Buffer.alloc(4);
    length.writeUInt32BE(data.length);
    const typed = Buffer.concat([Buffer.from(type, "ascii"), data]);
    const crc = Buffer.alloc(4);
    crc.writeUInt32BE(crc32(typed));
    return Buffer.concat([length, typed, crc]);
}

function encodePng(size) {
    const header = Buffer.alloc(13);
    header.writeUInt32BE(size, 0);
    header.writeUInt32BE(size, 4);
    header[8] = 8; // bit depth
    header[9] = 6; // colour type: RGBA
    header[10] = 0; // compression
    header[11] = 0; // filter method
    header[12] = 0; // no interlace
    return Buffer.concat([
        Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
        chunk("IHDR", header),
        chunk("IDAT", deflateSync(renderRgba(size))),
        chunk("IEND", Buffer.alloc(0)),
    ]);
}

for (const size of [192, 512]) {
    const path = join(publicDir, `icon-${size}.png`);
    writeFileSync(path, encodePng(size));
    console.log(`wrote ${path}`);
}
