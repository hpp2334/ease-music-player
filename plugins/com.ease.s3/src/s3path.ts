// S3 path & endpoint helpers — pure (no tur imports) so the Node selftest can
// exercise them directly (`node scripts/selftest.ts`).
//
// Path model mirroring the WebDAV plugin's conventions, mapped onto S3's flat
// key space:
//   - plugin paths are `/`-rooted: "/music/album/track.mp3"
//   - a `dir` from the host is "" or "/" for the root, else "/music/album"
//   - an S3 prefix always ends with "/" ("" for the root)
//   - keys are stored WITHOUT the leading slash: "music/album/track.mp3"
//
// URI encoding follows the SigV4 rules (everything but `A-Za-z0-9-._~`
// percent-encoded, hex uppercase; `/` preserved in paths). The SAME encoded
// string is used for both the request URL and the canonical request — they
// must match byte-for-byte or the signature fails. reqwest's `Url` passes
// already-encoded paths through unchanged (the WebDAV plugin relies on the
// same property).

/** Endpoint as the user entered it, normalized for requests + signing. */
export interface S3Endpoint {
    /** Scheme + authority, no trailing slash, default port stripped —
     *  exactly the URL prefix we hand to `tur:net`. */
    origin: string;
    /** Value reqwest will put in the `Host` header (the URL authority after
     *  default-port normalization — signing anything else fails). */
    host: string;
    /** Base path prefix (for gateways that serve S3 under a subpath), "" or
     *  "/prefix" — normalized to start with "/" and not end with "/". */
    basePath: string;
}

/**
 * Normalize an endpoint string. Accepts `host`, `host:port`, `http(s)://…`,
 * optional base path, trailing slashes. Throws on anything without a parseable
 * host (no `URL` in the boa engine — pure string handling, like the WebDAV
 * plugin's `splitAddr`).
 */
export function normalizeEndpoint(input: string): S3Endpoint {
    let s = input.trim();
    if (s === "") throw new Error("s3: endpoint cannot be empty");
    const hasScheme = /^[a-zA-Z][a-zA-Z0-9+.-]*:\/\//.test(s);
    if (!hasScheme) s = "https://" + s;

    const schemeDelim = s.indexOf("://");
    const scheme = s.slice(0, schemeDelim).toLowerCase();
    if (scheme !== "http" && scheme !== "https") {
        throw new Error(`s3: endpoint scheme must be http or https: ${input}`);
    }
    const rest = s.slice(schemeDelim + 3);

    const slash = rest.indexOf("/");
    const authority = slash === -1 ? rest : rest.slice(0, slash);
    let basePath = slash === -1 ? "" : rest.slice(slash).replace(/\/+$/, "");
    if (authority === "" || /\s/.test(authority) || authority.startsWith(":")) {
        throw new Error(`s3: endpoint has no host: ${input}`);
    }
    // Authority must be `host` or `host:port` (IPv6 brackets tolerated).
    const colon = authority.lastIndexOf(":");
    let host = authority;
    if (colon !== -1) {
        const port = authority.slice(colon + 1);
        if (!/^\d+$/.test(port)) {
            throw new Error(`s3: endpoint has an invalid port: ${input}`);
        }
        if (authority.slice(0, colon) === "") {
            throw new Error(`s3: endpoint has no host: ${input}`);
        }
        // Strip explicit default ports — the `Url` crate drops them when
        // parsing, so the Host header (and the signed `host` value) must match.
        if ((scheme === "http" && port === "80") || (scheme === "https" && port === "443")) {
            host = authority.slice(0, colon);
        }
    }

    return { origin: `${scheme}://${host}${basePath}`, host, basePath: basePath === "" ? "" : basePath };
}

/** Strict RFC 3986 percent-encoding (SigV4 rules): everything but
 *  `A-Za-z0-9-._~`; slashes preserved unless `keepSlash` is false. Hex is
 *  uppercase per the spec. */
export function s3Encode(s: string, keepSlash: boolean): string {
    const unreserved = /[A-Za-z0-9\-._~]/;
    const bytes = new TextEncoder().encode(s);
    let out = "";
    for (const b of bytes) {
        const ch = String.fromCharCode(b);
        if (unreserved.test(ch) || (keepSlash && ch === "/")) {
            out += ch;
        } else {
            out += "%" + b.toString(16).toUpperCase().padStart(2, "0");
        }
    }
    return out;
}

/** Tolerant percent-decoding (S3 list responses return keys URL-encoded;
 *  MinIO returns them raw — decode failures keep the input verbatim). */
export function s3Decode(s: string): string {
    if (!s.includes("%")) return s;
    try {
        return decodeURIComponent(s);
    } catch {
        return s;
    }
}

/** Host `dir` ("", "/", "/music/album") → S3 list prefix ("", "music/album/"). */
export function dirToPrefix(dir: string): string {
    let d = dir.replace(/^\/+/, "").replace(/\/+$/, "");
    if (d === "") return "";
    return d + "/";
}

/** Plugin path ("/music/a.mp3") → object key ("music/a.mp3"). */
export function pathToKey(path: string): string {
    return path.replace(/^\/+/, "");
}

/** Object key ("music/a.mp3") → plugin path ("/music/a.mp3"). */
export function keyToPath(key: string): string {
    return "/" + key.replace(/^\/+/, "");
}

/**
 * Canonical URI for a request (also used verbatim as the URL's path):
 * `/[basePath/]bucket[/key]`, strictly encoded with slashes preserved.
 * The bucket charset is validated by the connect flow, but encode anyway so
 * nothing surprising ever reaches the wire unescaped.
 */
export function canonicalUri(endpoint: S3Endpoint, bucket: string, key: string | null): string {
    const base = endpoint.basePath === "" ? "" : s3Encode(endpoint.basePath, true);
    let uri = `${base}/${s3Encode(bucket, false)}`;
    if (key != null && key !== "") uri += "/" + s3Encode(key, true);
    return uri;
}
