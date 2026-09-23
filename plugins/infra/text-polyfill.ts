// TextEncoder / TextDecoder polyfill for the tur (boa) runtime.
//
// boa implements ECMAScript but not the Web Platform text codecs. `tur:std`
// already ships the UTF-8 primitives (`encodeUtf8` / `decodeUtf8`), so these
// classes are thin WHATWG-shaped wrappers over them instead of another
// hand-rolled encoder. Import this module FIRST from every bundle entry
// (side-effect import) — npm dependencies bundled after it see the globals.
//
// Semantics notes:
// - `encode`: WHATWG replaces lone surrogates with U+FFFD; boa's
//   `encodeUtf8` renders them as literal `\uD800` text, so the input is
//   normalized with `String.prototype.toWellFormed()` (ES2024, implemented
//   by boa) first.
// - `encodeInto`: never splits a code point's multi-byte sequence — the cut
//   is walked back to a UTF-8 lead-byte boundary.
// - `decode`: `decodeUtf8` is strict (throws on invalid bytes), which maps
//   1:1 to the `fatal: true` option; the default non-fatal mode replaces
//   each invalid byte with U+FFFD by greedily re-decoding the longest valid
//   prefix (binary search).
// - `decode(bytes, { stream: true })` decodes each chunk independently (no
//   cross-call decoder state) — the same trade-off the common
//   `fast-text-encoding` polyfill makes.
// - UTF-8 labels only; any other label throws `RangeError`.

import { decodeUtf8, encodeUtf8 } from "tur:std";

interface TextDecoderOptions {
    fatal?: boolean;
    ignoreBOM?: boolean;
}

interface TextEncodeIntoResult {
    read: number;
    written: number;
}

export interface TextEncoderInstance {
    readonly encoding: "utf-8";
    encode(input?: string): Uint8Array;
    encodeInto(source: string, destination: Uint8Array): TextEncodeIntoResult;
}

export interface TextDecoderInstance {
    readonly encoding: "utf-8";
    readonly fatal: boolean;
    readonly ignoreBOM: boolean;
    decode(input?: Uint8Array | ArrayBuffer, options?: { stream?: boolean }): string;
}

declare global {
    // lib ES2020 has no DOM types, so declare the runtime shape we install.
    var TextEncoder: { new (): TextEncoderInstance; prototype: TextEncoderInstance };
    var TextDecoder: {
        new (label?: string, options?: TextDecoderOptions): TextDecoderInstance;
        prototype: TextDecoderInstance;
    };
}

const UTF8_LABELS = [
    "utf-8",
    "utf8",
    "unicode-1-1-utf-8",
    "unicode11utf8",
    "unicode20utf8",
    "x-unicode20utf8",
];

/** View of a BufferSource as bytes, honoring byteOffset / byteLength. */
function toBytes(input: Uint8Array | ArrayBuffer): Uint8Array {
    if (input instanceof ArrayBuffer) return new Uint8Array(input);
    return new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
}

/** `String.prototype.toWellFormed()` (ES2024) — implemented by boa, but not
 *  declared in the ES2020 lib the plugins typecheck against, hence the
 *  local cast instead of a program-wide lib bump. */
function wellFormed(s: string): string {
    return (s as string & { toWellFormed(): string }).toWellFormed();
}

/** Non-fatal decode: U+FFFD per invalid byte, via longest-valid-prefix
 *  binary search over `decodeUtf8` (which is strict). */
function decodeLossy(bytes: Uint8Array): string {
    try {
        return decodeUtf8(bytes);
    } catch {
        let out = "";
        let start = 0;
        while (start < bytes.length) {
            let lo = 0;
            let hi = bytes.length - start;
            while (lo < hi) {
                const mid = (lo + hi + 1) >> 1;
                let ok = true;
                try {
                    decodeUtf8(bytes.subarray(start, start + mid));
                } catch {
                    ok = false;
                }
                if (ok) lo = mid;
                else hi = mid - 1;
            }
            if (lo > 0) {
                out += decodeUtf8(bytes.subarray(start, start + lo));
                start += lo;
            } else {
                out += "\uFFFD";
                start += 1;
            }
        }
        return out;
    }
}

if (typeof globalThis.TextEncoder === "undefined") {
    class TextEncoder {
        get encoding(): "utf-8" {
            return "utf-8";
        }

        encode(input?: string): Uint8Array {
            const s = input == null ? "" : String(input);
            return encodeUtf8(wellFormed(s));
        }

        encodeInto(source: string, destination: Uint8Array): TextEncodeIntoResult {
            if (!(destination instanceof Uint8Array)) {
                throw new TypeError("encodeInto: destination must be a Uint8Array");
            }
            const bytes = encodeUtf8(wellFormed(String(source ?? "")));
            // Cut at a lead-byte boundary so no code point splits; a
            // continuation byte matches 10xxxxxx (0xC0 mask).
            let n = Math.min(bytes.length, destination.length);
            while (n > 0 && n < bytes.length && (bytes[n] & 0xc0) === 0x80) n--;
            destination.set(bytes.subarray(0, n));
            return {
                read: n === 0 ? 0 : decodeUtf8(bytes.subarray(0, n)).length,
                written: n,
            };
        }
    }
    globalThis.TextEncoder = TextEncoder as unknown as typeof globalThis.TextEncoder;
}

if (typeof globalThis.TextDecoder === "undefined") {
    class TextDecoder {
        readonly fatal: boolean;
        readonly ignoreBOM: boolean;

        constructor(label?: string, options?: TextDecoderOptions) {
            const norm = String(label ?? "utf-8").trim().toLowerCase();
            if (!UTF8_LABELS.includes(norm)) {
                throw new RangeError(
                    `TextDecoder: unsupported encoding label '${label}' (UTF-8 only)`,
                );
            }
            this.fatal = !!options?.fatal;
            this.ignoreBOM = !!options?.ignoreBOM;
        }

        get encoding(): "utf-8" {
            return "utf-8";
        }

        decode(
            input?: Uint8Array | ArrayBuffer,
            _options?: { stream?: boolean },
        ): string {
            if (input == null) return "";
            let bytes = toBytes(input);
            if (
                !this.ignoreBOM &&
                bytes.length >= 3 &&
                bytes[0] === 0xef &&
                bytes[1] === 0xbb &&
                bytes[2] === 0xbf
            ) {
                bytes = bytes.subarray(3);
            }
            return this.fatal ? decodeUtf8(bytes) : decodeLossy(bytes);
        }
    }
    globalThis.TextDecoder = TextDecoder as unknown as typeof globalThis.TextDecoder;
}

export {};
