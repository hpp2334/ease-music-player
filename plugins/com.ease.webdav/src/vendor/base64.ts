// Base64 encoding of a UTF-8 string — vendored (no `btoa` in the boa engine).

import { utf8Encode } from "./md5";

const TABLE = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

export function base64EncodeUtf8(s: string): string {
    const bytes = utf8Encode(s);
    let out = "";
    for (let i = 0; i < bytes.length; i += 3) {
        const b0 = bytes[i];
        const b1 = i + 1 < bytes.length ? bytes[i + 1] : 0;
        const b2 = i + 2 < bytes.length ? bytes[i + 2] : 0;
        out += TABLE[b0 >> 2];
        out += TABLE[((b0 & 0x03) << 4) | (b1 >> 4)];
        out += i + 1 < bytes.length ? TABLE[((b1 & 0x0f) << 2) | (b2 >> 6)] : "=";
        out += i + 2 < bytes.length ? TABLE[b2 & 0x3f] : "=";
    }
    return out;
}
