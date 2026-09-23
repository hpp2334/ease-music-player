// ListObjectsV2 response handling — pure (fast-xml-parser is an npm dep that
// runs identically under Node and the plugin runtime). The backend feeds
// fetched XML pages in and gets `StorageEntry[]` out; the selftest exercises
// the mapping against fixture XML.
//
// S3 quirks handled here:
//   - Keys AND CommonPrefixes arrive URL-encoded from AWS; MinIO returns
//     them raw. A tolerant decode (failure keeps the input verbatim) covers
//     both — the decoded form is the canonical key the plugin re-encodes for
//     `storage:get`, so the round-trip stays exact for either flavor.
//   - "Folder" markers: a zero-byte key ending in `/` (the listed dir's own
//     marker, e.g. `music/` inside a `music/`-prefixed listing, or any
//     `a/b/` marker surfaced at the root) is a directory placeholder, not a
//     file — elided.
//   - `<IsTruncated>` + `<NextContinuationToken>` drive the backend's
//     pagination loop (and the token rides the signed query of the next
//     page).

import { XMLParser } from "fast-xml-parser";
import type { StorageEntry } from "../../infra/host-ops";
import { parseIsoDate } from "../../infra/http-dates.ts";
import { s3Decode } from "./s3path.ts";

const xmlParser = new XMLParser({
    removeNSPrefix: true,
    // Keep tag text verbatim — Sizes are digit strings we parse ourselves,
    // and strnum would mangle display-ish values.
    parseTagValue: false,
    // Contents / CommonPrefixes repeat; force arrays so single-element
    // pages take the same code path.
    isArray: (name) => /(?:^|:)(contents|commonprefixes)$/i.test(name),
});

/** Case-insensitive child lookup (matches the WebDAV plugin's `pick`). */
function pick(obj: unknown, name: string): unknown {
    if (obj == null || typeof obj !== "object") return undefined;
    for (const k of Object.keys(obj)) {
        if (k.toLowerCase() === name) return (obj as Record<string, unknown>)[k];
    }
    return undefined;
}

function pickAll(obj: unknown, name: string): unknown[] {
    if (obj == null || typeof obj !== "object") return [];
    const out: unknown[] = [];
    for (const k of Object.keys(obj)) {
        if (k.toLowerCase() === name) {
            const v = (obj as Record<string, unknown>)[k];
            out.push(...(Array.isArray(v) ? v : [v]));
        }
    }
    return out;
}

/** One page of a ListObjectsV2 response, after decoding. */
export interface ListPage {
    keys: Array<{ key: string; size?: number; modifiedAt?: number }>;
    prefixes: string[];
    truncated: boolean;
    nextToken: string | null;
}

/** Parse one ListObjectsV2 XML body. Throws on malformed XML (a silent `[]`
 * once hid a runtime gap as an "empty directory" for weeks — same policy as
 * the WebDAV plugin's multistatus parser). */
export function parseListPage(xml: string): ListPage {
    const parsed: unknown = xmlParser.parse(xml);
    const roots = pickAll(parsed, "listbucketresult");
    if (roots.length === 0) {
        throw new Error("s3: ListObjectsV2 response has no ListBucketResult root");
    }

    const keys: Array<{ key: string; size?: number; modifiedAt?: number }> = [];
    for (const contents of roots.flatMap((r) => pickAll(r, "contents"))) {
        const key = pick(contents, "key");
        if (typeof key !== "string" || key === "") continue;
        let size: number | undefined;
        const sizeStr = pick(contents, "size");
        if (typeof sizeStr === "string" && /^\d+$/.test(sizeStr.trim())) {
            size = parseInt(sizeStr.trim(), 10);
        }
        const lastmodStr = pick(contents, "lastmodified");
        const modifiedAt =
            typeof lastmodStr === "string" ? parseIsoDate(lastmodStr) : undefined;
        keys.push({ key: s3Decode(key), size, modifiedAt });
    }

    const prefixes: string[] = [];
    for (const cp of roots.flatMap((r) => pickAll(r, "commonprefixes"))) {
        // <CommonPrefixes><Prefix>…</Prefix></CommonPrefixes> — the inner
        // element is what repeats in practice; accept either nesting.
        const inner = pickAll(cp, "prefix");
        for (const p of inner) {
            if (typeof p === "string" && p !== "") prefixes.push(s3Decode(p));
        }
    }

    const truncated = String(pick(roots[0], "istruncated") ?? "false").toLowerCase() === "true";
    const token = pick(roots[0], "nextcontinuationtoken");
    return {
        keys,
        prefixes,
        truncated,
        nextToken: truncated && typeof token === "string" && token !== "" ? token : null,
    };
}

/** Last path segment, tolerating a trailing slash. */
function lastSegment(p: string): string {
    const t = p.endsWith("/") ? p.slice(0, -1) : p;
    const parts = t.split("/");
    return parts[parts.length - 1] ?? t;
}

/** Map one page to host entries for a listing of `prefix` (already
 *  dirToPrefix-shaped). Folder-marker keys are elided; dirs sort first. */
export function pageToEntries(page: ListPage, prefix: string): StorageEntry[] {
    const out: StorageEntry[] = [];
    const seen = new Set<string>();

    for (const { key, size, modifiedAt } of page.keys) {
        if (key === prefix) continue; // the listed dir's own marker
        if (key.endsWith("/")) continue; // nested folder markers, not files
        if (seen.has(key)) continue;
        seen.add(key);
        out.push({
            name: lastSegment(key),
            path: "/" + key,
            size,
            isDir: false,
            // S3 has no creation time; LastModified is the only timestamp.
            modifiedAt,
        });
    }
    for (const p of page.prefixes) {
        const trimmed = p.endsWith("/") ? p.slice(0, -1) : p;
        if (trimmed === "" || trimmed === prefix.slice(0, -1)) continue;
        if (seen.has(trimmed)) continue;
        seen.add(trimmed);
        out.push({
            name: lastSegment(trimmed),
            path: "/" + trimmed,
            isDir: true,
        });
    }

    out.sort((a, b) => {
        if (a.isDir !== b.isDir) return a.isDir ? -1 : 1;
        if (a.path < b.path) return -1;
        if (a.path > b.path) return 1;
        return 0;
    });
    return out;
}

/** Parse an S3 `<Error>` XML body (operation failures) for its Code/Message —
 *  gives diagnostics like `NoSuchKey` / `RequestTimeTooSkewed` instead of a
 *  bare status. Returns null when the body isn't an S3 error document. */
export function parseS3Error(xml: string): { code: string; message: string } | null {
    try {
        const parsed: unknown = xmlParser.parse(xml);
        const err = pick(parsed, "error");
        if (err == null || typeof err !== "object") return null;
        const code = pick(err, "code");
        const message = pick(err, "message");
        if (typeof code !== "string") return null;
        return {
            code,
            message: typeof message === "string" ? message : "",
        };
    } catch {
        return null;
    }
}
