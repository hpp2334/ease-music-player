// One-off icon generator for plugins/com.ease.s3 (run: node scripts/make-icon.mjs).
// Draws a simple S3 "bucket" glyph — rounded-square background, bucket rim +
// tapered body — and writes a 256×256 RGBA PNG using only node:zlib (hand-
// rolled PNG chunks + CRC32). The result is committed as icon.png; the script
// stays so the art can be tweaked without external tools.

import { deflateSync } from "node:zlib";
import { writeFileSync } from "node:fs";

const SIZE = 256;

// --- tiny canvas -----------------------------------------------------------
const px = new Uint8Array(SIZE * SIZE * 4);

function setPixel(x, y, r, g, b, a) {
    const i = (y * SIZE + x) * 4;
    // alpha-composite over
    const ia = a / 255;
    const oa = px[i + 3] / 255;
    const na = ia + oa * (1 - ia);
    if (na === 0) return;
    px[i] = Math.round((r * ia + px[i] * oa * (1 - ia)) / na);
    px[i + 1] = Math.round((g * ia + px[i + 1] * oa * (1 - ia)) / na);
    px[i + 2] = Math.round((b * ia + px[i + 2] * oa * (1 - ia)) / na);
    px[i + 3] = Math.round(na * 255);
}

const inRoundedSquare = (x, y, x0, y0, x1, y1, r) => {
    if (x < x0 || x > x1 || y < y0 || y > y1) return false;
    const cx = Math.max(x0 + r, Math.min(x, x1 - r));
    const cy = Math.max(y0 + r, Math.min(y, y1 - r));
    return (x - cx) ** 2 + (y - cy) ** 2 <= r * r || (x >= x0 + r && x <= x1 - r) || (y >= y0 + r && y <= y1 - r);
};

// Coverage (0..1) of an anti-aliased band around distance d from center.
const band = (d, halfWidth) => Math.max(0, Math.min(1, halfWidth - Math.abs(d) + 0.5));

// --- palette ---------------------------------------------------------------
const BG = [37, 84, 154]; // deep blue
const BG_LIGHT = [52, 108, 186]; // upper-part highlight
const WHITE = [255, 255, 255];

for (let y = 0; y < SIZE; y++) {
    for (let x = 0; x < SIZE; x++) {
        // Background: rounded square with a subtle vertical two-tone split.
        if (inRoundedSquare(x, y, 12, 12, 244, 244, 56)) {
            const t = Math.max(0, Math.min(1, (y - 40) / 176));
            const c = BG.map((v, i) => Math.round(v + (BG_LIGHT[i] - v) * t));
            setPixel(x, y, c[0], c[1], c[2], 255);
        }

        // Bucket rim: ellipse ring centered (128, 92), rx 72, ry 26.
        const rimD = Math.sqrt(((x - 128) / 72) ** 2 + ((y - 92) / 26) ** 2) - 1;
        const rimA = band(rimD * 26, 3.2); // ~6.4px thick ring
        if (rimA > 0) setPixel(x, y, WHITE[0], WHITE[1], WHITE[2], Math.round(rimA * 235));

        // Handle: upper part of an ellipse (rx 70, ry 32) whose ends meet
        // the rim's left/right edges just above its midline.
        const hd = Math.sqrt(((x - 128) / 70) ** 2 + ((y - 92) / 32) ** 2) - 1;
        if (y < 86) {
            const ha = band(hd * 32, 2.6);
            if (ha > 0) setPixel(x, y, WHITE[0], WHITE[1], WHITE[2], Math.round(ha * 235));
        }

        // Bucket body: tapered outline from y=100 to y=196, half-width
        // 62 -> 42, plus the bottom edge.
        if (y >= 100 && y <= 196) {
            const t = (y - 100) / 96;
            const hw = 62 + (42 - 62) * t;
            const edge = Math.abs(Math.abs(x - 128) - hw);
            const bodyA = band(edge, 3.0);
            if (bodyA > 0) setPixel(x, y, WHITE[0], WHITE[1], WHITE[2], Math.round(bodyA * 235));
        }
        if (y >= 190 && y <= 196 && Math.abs(x - 128) <= 44) {
            const bottomA = band(Math.abs(y - 193), 3.0);
            setPixel(x, y, WHITE[0], WHITE[1], WHITE[2], Math.round(bottomA * 235));
        }
    }
}

// --- PNG encode ------------------------------------------------------------
const crcTable = new Int32Array(256);
for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    crcTable[n] = c;
}
const crc32 = (buf) => {
    let c = 0xffffffff;
    for (const b of buf) c = crcTable[(c ^ b) & 0xff] ^ (c >>> 8);
    return (c ^ 0xffffffff) >>> 0;
};

function chunk(type, data) {
    const len = Buffer.alloc(4);
    len.writeUInt32BE(data.length);
    const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
    const crc = Buffer.alloc(4);
    crc.writeUInt32BE(crc32(body));
    return Buffer.concat([len, body, crc]);
}

// Filter byte 0 per scanline.
const raw = Buffer.alloc(SIZE * (SIZE * 4 + 1));
for (let y = 0; y < SIZE; y++) {
    raw[y * (SIZE * 4 + 1)] = 0;
    Buffer.from(px.buffer, y * SIZE * 4, SIZE * 4).copy(raw, y * (SIZE * 4 + 1) + 1);
}

const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(SIZE, 0);
ihdr.writeUInt32BE(SIZE, 4);
ihdr[8] = 8; // bit depth
ihdr[9] = 6; // color type RGBA
const png = Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr),
    chunk("IDAT", deflateSync(raw, { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
]);

writeFileSync(new URL("../icon.png", import.meta.url), png);
console.log(`icon.png written: ${png.length} bytes`);
