// AWS Signature Version 4 — pure module (no tur imports) so the Node selftest
// can verify it against known vectors and an independent node:crypto
// reference signer. Uses `@noble/hashes` (pure-JS SHA-256 / HMAC — the plugin
// runtime's `crypto` global only exposes getRandomValues/randomUUID, no
// WebCrypto subtle) and the global `TextEncoder` (native in Node, provided by
// `infra/text-polyfill` in the plugin runtime — imported first by backend.ts).
//
// Only what S3 needs: bodyless GETs (list + ranged object download), static
// credentials, path-style addressing. The canonical URI/query strings passed
// in must already be SigV4-encoded (see `s3path.ts`) — the same strings go
// into the request URL and the canonical request, byte for byte.

import { sha256 } from "@noble/hashes/sha2.js";
import { hmac } from "@noble/hashes/hmac.js";
import { bytesToHex } from "@noble/hashes/utils.js";

/** SHA-256 of the empty payload — every request this plugin signs is a
 *  bodyless GET. (Hardcoded so the constant is testable against itself.) */
export const EMPTY_PAYLOAD_SHA256 =
    "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

export const S3_SERVICE = "s3";
export const ALGORITHM = "AWS4-HMAC-SHA256";
export const AMZ_TERMINATOR = "aws4_request";

const encoder = (): { encode(s: string): Uint8Array } => new TextEncoder();

/** `{ amzDate: "20150830T123600Z", dateStamp: "20150830" }` for `now`
 *  (UTC — SigV4 has no notion of local time). */
export function amzTimestamps(now: Date): { amzDate: string; dateStamp: string } {
    const p = (n: number, w: number): string => String(n).padStart(w, "0");
    const dateStamp =
        String(now.getUTCFullYear()).padStart(4, "0") +
        p(now.getUTCMonth() + 1, 2) +
        p(now.getUTCDate(), 2);
    const amzDate =
        dateStamp + "T" + p(now.getUTCHours(), 2) + p(now.getUTCMinutes(), 2) + p(now.getUTCSeconds(), 2) + "Z";
    return { amzDate, dateStamp };
}

/**
 * Canonical query string: params sorted by key (then value), strictly
 * RFC 3986-encoded, joined `k=v` with `&`. The SAME string doubles as the
 * request URL's query — servers do not care about order, the signature does.
 * `params` may be given in any order; keys must not be pre-encoded.
 */
export function canonicalQueryString(params: Array<[string, string]>): string {
    const enc = (s: string): string => s3UriEncode(s);
    return params
        .slice()
        .sort((a, b) => (a[0] === b[0] ? (a[1] < b[1] ? -1 : a[1] > b[1] ? 1 : 0) : a[0] < b[0] ? -1 : 1))
        .map(([k, v]) => `${enc(k)}=${enc(v)}`)
        .join("&");
}

// Local import-free copy of the strict encoder (s3path.ts keeps its own so
// the two pure modules stay independently testable — `s3Encode(s, false)`).
function s3UriEncode(s: string): string {
    const unreserved = /[A-Za-z0-9\-._~]/;
    const bytes = new TextEncoder().encode(s);
    let out = "";
    for (const b of bytes) {
        const ch = String.fromCharCode(b);
        out += unreserved.test(ch) ? ch : "%" + b.toString(16).toUpperCase().padStart(2, "0");
    }
    return out;
}

/** SigV4 signing key: HMAC chain over date / region / service / terminator. */
export function deriveSigningKey(
    secretKey: string,
    dateStamp: string,
    region: string,
    service: string,
): Uint8Array {
    const enc = encoder();
    let key = hmac(sha256, enc.encode("AWS4" + secretKey), enc.encode(dateStamp));
    key = hmac(sha256, key, enc.encode(region));
    key = hmac(sha256, key, enc.encode(service));
    return hmac(sha256, key, enc.encode(AMZ_TERMINATOR));
}

/** Canonical request string (the SigV4 spec's step 1) — exported for tests.
 *  Header lines are rendered in sorted (lowercase) name order, as the spec
 *  requires — insertion order is only accidentally sorted for list ops. */
export function canonicalRequest(p: SigV4Request): string {
    const headerLines = Object.keys(p.headers)
        .sort()
        .map((name) => `${name}:${trimAll(p.headers[name])}\n`)
        .join("");
    const signedHeaders = Object.keys(p.headers).sort().join(";");
    return (
        p.method + "\n" +
        p.canonicalUri + "\n" +
        p.canonicalQuery + "\n" +
        headerLines + "\n" +
        signedHeaders + "\n" +
        (p.payloadSha256 ?? EMPTY_PAYLOAD_SHA256)
    );
}

/** String to sign (step 2) — exported for tests. */
export function stringToSign(p: SigV4Request): string {
    const hashedCanonical = bytesToHex(sha256(encoder().encode(canonicalRequest(p))));
    return (
        ALGORITHM + "\n" +
        p.amzDate + "\n" +
        `${p.dateStamp}/${p.region}/${p.service}/${AMZ_TERMINATOR}` + "\n" +
        hashedCanonical
    );
}

/** `trimAll`: sequential spaces collapse to one, then trim — the spec's
 *  header-value canonicalization. */
function trimAll(v: string): string {
    return v.replace(/\s+/g, " ").trim();
}

export interface SigV4Request {
    method: string;
    /** SigV4-encoded, slash-preserving — identical to the URL path. */
    canonicalUri: string;
    /** From `canonicalQueryString` — identical to the URL query. */
    canonicalQuery: string;
    /** Header name (lowercase) → value. Must include `host` and both
     *  `x-amz-*` headers; add `range` for ranged downloads. */
    headers: Record<string, string>;
    amzDate: string;
    dateStamp: string;
    region: string;
    service: string;
    /** Hex sha256 of the payload; defaults to the empty-payload hash. */
    payloadSha256?: string;
}

export interface SignedRequest {
    authorization: string;
    signedHeaders: string;
}

/** Steps 3–5: derive the signing key, sign the string-to-sign, assemble the
 *  `Authorization` header. */
export function signRequest(
    p: SigV4Request,
    credentials: { accessKey: string; secretKey: string },
): SignedRequest {
    const signedHeaders = Object.keys(p.headers).sort().join(";");
    const key = deriveSigningKey(credentials.secretKey, p.dateStamp, p.region, p.service);
    const signature = bytesToHex(hmac(sha256, key, encoder().encode(stringToSign(p))));
    const authorization =
        `${ALGORITHM} Credential=${credentials.accessKey}` +
        `/${p.dateStamp}/${p.region}/${p.service}/${AMZ_TERMINATOR}` +
        `, SignedHeaders=${signedHeaders}, Signature=${signature}`;
    return { authorization, signedHeaders };
}
