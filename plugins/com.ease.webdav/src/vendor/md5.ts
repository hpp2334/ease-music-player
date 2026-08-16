// MD5 message digest (RFC 1321) — vendored so the WebDAV plugin's Digest
// auth needs no npm dependency. Classic public-domain implementation
// structure (same algorithm as Joseph Myers' js-md5), operating on bytes.
//
// Vectors (RFC 1321 §A.5):
//   ""       d41d8cd98f00b204e9800998ecf8427e
//   "a"      0cc175b9c0f1b6a831c399e269772661
//   "abc"    900150983cd24fb0d6963f7d28e17f72
//   "message digest" f96b697d7cb7938d525a2f31aaf161d0

const SH = new Uint8Array([
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22,
    5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20,
    4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23,
    6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
]);

const K = new Uint32Array(64);
for (let i = 0; i < 64; i++) {
    K[i] = Math.floor(Math.abs(Math.sin(i + 1)) * 4294967296) >>> 0;
}

function rotl(x: number, c: number): number {
    return (((x << c) | (x >>> (32 - c))) >>> 0);
}

/** Raw MD5 over bytes. */
export function md5Bytes(msg: Uint8Array): Uint8Array {
    const len = msg.length;
    const bitLenLo = (len * 8) >>> 0;
    const bitLenHi = Math.floor((len * 8) / 4294967296);
    const totalLen = (((len + 8) >>> 6) + 1) << 6;

    const buf = new Uint8Array(totalLen);
    buf.set(msg);
    buf[len] = 0x80;
    const dv = new DataView(buf.buffer);
    dv.setUint32(totalLen - 8, bitLenLo, true);
    dv.setUint32(totalLen - 4, bitLenHi, true);

    let a0 = 0x67452301;
    let b0 = 0xefcdab89;
    let c0 = 0x98badcfe;
    let d0 = 0x10325476;

    const M = new Uint32Array(16);
    for (let off = 0; off < totalLen; off += 64) {
        for (let i = 0; i < 16; i++) {
            M[i] = dv.getUint32(off + i * 4, true);
        }
        let A = a0;
        let B = b0;
        let C = c0;
        let D = d0;
        for (let i = 0; i < 64; i++) {
            let F: number;
            let g: number;
            if (i < 16) {
                F = (B & C) | (~B & D);
                g = i;
            } else if (i < 32) {
                F = (D & B) | (~D & C);
                g = (5 * i + 1) % 16;
            } else if (i < 48) {
                F = B ^ C ^ D;
                g = (3 * i + 5) % 16;
            } else {
                F = C ^ (B | ~D);
                g = (7 * i) % 16;
            }
            const sum = (F + A + K[i] + M[g]);
            A = D;
            D = C;
            C = B;
            B = (B + rotl(sum >>> 0, SH[i])) >>> 0;
        }
        a0 = (a0 + A) >>> 0;
        b0 = (b0 + B) >>> 0;
        c0 = (c0 + C) >>> 0;
        d0 = (d0 + D) >>> 0;
    }

    const out = new Uint8Array(16);
    const odv = new DataView(out.buffer);
    odv.setUint32(0, a0, true);
    odv.setUint32(4, b0, true);
    odv.setUint32(8, c0, true);
    odv.setUint32(12, d0, true);
    return out;
}

/** MD5 of a UTF-8 string, hex-encoded (lowercase). */
export function md5Hex(s: string): string {
    return toHex(md5Bytes(utf8Encode(s)));
}

function toHex(bytes: Uint8Array): string {
    let out = "";
    for (let i = 0; i < bytes.length; i++) {
        out += (bytes[i] >>> 4).toString(16) + (bytes[i] & 0xf).toString(16);
    }
    return out;
}

/** UTF-8 encode a JS string into bytes. */
export function utf8Encode(s: string): Uint8Array {
    const out: number[] = [];
    for (let i = 0; i < s.length; i++) {
        let cp = s.codePointAt(i)!;
        if (cp > 0xffff) i++; // surrogate pair consumed
        if (cp < 0x80) {
            out.push(cp);
        } else if (cp < 0x800) {
            out.push(0xc0 | (cp >> 6), 0x80 | (cp & 0x3f));
        } else if (cp < 0x10000) {
            out.push(0xe0 | (cp >> 12), 0x80 | ((cp >> 6) & 0x3f), 0x80 | (cp & 0x3f));
        } else {
            out.push(
                0xf0 | (cp >> 18),
                0x80 | ((cp >> 12) & 0x3f),
                0x80 | ((cp >> 6) & 0x3f),
                0x80 | (cp & 0x3f),
            );
        }
    }
    return new Uint8Array(out);
}
