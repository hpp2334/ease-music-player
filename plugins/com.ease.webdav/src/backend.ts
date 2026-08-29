// WebDAV storage provider — a headless JS plugin that serves `list` and `get`
// (streaming byte-range download) over the `tur:rpc` channel, plus a
// username/password connect flow (no OAuth). The host `JsStorageBackend`
// treats WebDAV like any other `StorageBackend`.
//
// Multi-instance: each configured WebDAV server is one *instance* named
// `webdav:<uuid>` (the storage row's `plugin_storage_id`;
// legacy-migrated rows use `webdav:<legacy-id>`). Per-instance config lives in `ease.db`
// (this plugin's KV) under `storage:<instance>` = JSON
// `{ alias, addr, username, isAnonymous, secretId }`; the password lives in
// `ease.secret` under that `secretId` (scope `plugin:com.ease.webdav`).
//
// Identity: this headless instance is created by `KeepBackendService` which
// stamps `PluginId("com.ease.webdav")` into the per-instance data slot.
// `ease.*` bridge fns resolve the calling plugin from that slot — no
// pluginId argument is needed (or accepted) on any call here.
//
// Handlers, split by caller (the dispatcher routes strictly by scope). The
// host-called ops are contract literals — identical names for every storage
// provider, identity riding the payload (`pluginId` = this manifest's id,
// `storageId` = the `plugin_storage_id` instance):
//
// hostRpc — the Rust host invokes these:
//   - storage:list           { pluginId, storageId, dir }          -> Entry[]
//   - storage:get            { pluginId, storageId, path, offset } -> registerStream: meta
//     { totalLength?, name?, contentType?, dataOffset? } + credit-gated body
//   - storage:removeInstance { pluginId, storageId }               -> {}
//     (the storages-page trash button, via the host's
//     `storage_plugin.remove_instance` bridge)
//
// viewRpc — this plugin's own view invokes these via `ease.rpc.call`
// (plugin-private names, no identity needed — the backend knows itself):
//   - webdav:test           { storageId?, addr, username, password?, isAnonymous } -> { result }
//   - webdav:connect        { storageId?, addr, alias, username?, password?, isAnonymous } -> { storageId, created }
//     (the add/edit-storage form)
//
// Auth: Basic preemptively when credentials exist; on a 401 the
// `WWW-Authenticate` challenge is cached per server and Digest (MD5 /
// MD5-sess, qop=auth) is answered on the retry — the same challenge/retry
// shape as the previous Rust implementation
// (`rust-libs/ease-remote-storage/src/impls/webdav.rs`).
//
// Error contract: messages prefixed `UNAUTHORIZED` / `TIMEOUT` are mapped by
// the host (`ease-js-storage`) to typed errors so the UI distinguishes auth
// failures and timeouts.
//
// Ported from the Rust implementation noted above.

import { request, requestStream } from "tur:net";
// TextEncoder/TextDecoder polyfill FIRST — npm deps below may rely on them.
import "../../infra/string-polyfill";
import "../../infra/text-polyfill";
import type { StreamResponse } from "tur:net";
import { decodeUtf8 } from "tur:std";
import { hostRpc, viewRpc } from "tur:rpc";
import type { StreamSource } from "tur:rpc";
import { db, secret, context } from "ease";

// npm deps — bundled by rspack (only `tur:*` / `ease` are externals). The
// runtime provides Web Platform globals the usual way: `crypto` (OS entropy,
// installed Rust-side by `plugin_runtime/webapi.rs`) and
// TextEncoder/TextDecoder (via the text-polyfill import above) — so
// crypto-dependent packages like `uuid` work unmodified.
import { v4 as uuidv4 } from "uuid";
import { md5 } from "js-md5";
import { Base64 } from "js-base64";
import { XMLParser } from "fast-xml-parser";

// ---------------------------------------------------------------------------
// Per-instance state
// ---------------------------------------------------------------------------

interface InstanceConfig {
    alias: string;
    addr: string;
    username: string;
    isAnonymous: boolean;
    secretId: number | null;
    /** Cached password (from the secret store). */
    password: string | null;
}

/** instance ("webdav:<uuid>") -> config. Lazily loaded on first use. */
const instances = new Map<string, InstanceConfig>();

/** addr -> cached Digest challenge (raw `WWW-Authenticate` value) + nonce count. */
const challenges = new Map<string, { header: string; nc: number }>();

class HttpError extends Error {
    constructor(public status: number, message: string) {
        super(message);
    }
}

function kvKey(instance: string): string {
    return `storage:${instance}`;
}

function configOf(instance: string): InstanceConfig {
    const st = instances.get(instance);
    if (st) return st;
    const raw = db.singleGet(kvKey(instance));
    if (raw == null) {
        throw new Error(`webdav: no config for instance ${instance}`);
    }
    const cfg = JSON.parse(raw);
    const conf: InstanceConfig = {
        alias: cfg.alias ?? instance,
        addr: cfg.addr ?? "",
        username: cfg.username ?? "",
        isAnonymous: !!cfg.isAnonymous,
        secretId: cfg.secretId ?? null,
        password: null,
    };
    instances.set(instance, conf);
    return conf;
}

function loadPassword(conf: InstanceConfig): string {
    if (conf.isAnonymous) return "";
    if (conf.password != null) return conf.password;
    if (conf.secretId == null) return "";
    const v = secret.get(conf.secretId);
    conf.password = v == null ? "" : v;
    return conf.password;
}

// ---------------------------------------------------------------------------
// URL helpers (no `URL` in the boa engine — pure string handling)
// ---------------------------------------------------------------------------

interface AddrParts {
    origin: string;
    /** Base path of the server root, normalized to start (not end) with '/'. */
    basePath: string;
}

function splitAddr(addr: string): AddrParts {
    const scheme = addr.indexOf("://");
    if (scheme <= 0) {
        throw new Error(`webdav: invalid server address: ${addr}`);
    }
    const slash = addr.indexOf("/", scheme + 3);
    if (slash === -1) {
        return { origin: addr, basePath: "/" };
    }
    return { origin: addr.slice(0, slash), basePath: addr.slice(slash) };
}

/** Percent-encode a path, preserving existing `%XX` escapes and RFC 3986
 * path-safe characters. Mirrors reqwest `Url::set_path` closely enough for
 * already-encoded WebDAV paths. */
function encodePath(p: string): string {
    const safe = /[A-Za-z0-9\-._~!$&'()*+,;=:@%]/;
    let out = "";
    for (const ch of p) {
        if (ch === "/" || safe.test(ch)) {
            out += ch;
        } else {
            // encodeURIComponent percent-encodes as UTF-8 by spec (uppercase
            // hex); every char it leaves unescaped is already in `safe`
            out += encodeURIComponent(ch).toUpperCase();
        }
    }
    return out;
}

/** Build the request URL for a WebDAV path (dirs get a trailing slash). */
function buildUrl(addr: string, path: string, isDir: boolean): string {
    const { origin, basePath } = splitAddr(addr);
    let p = basePath.replace(/\/+$/, "") + "/" + path.replace(/^\/+/, "");
    if (isDir && !p.endsWith("/")) p += "/";
    return origin + encodePath(p);
}

/** Map a PROPFIND `<href>` back to a plugin path (percent-encoded, base-path
 * stripped). Handles path-relative and absolute-URL hrefs. */
function hrefToPath(addr: string, href: string): string {
    const { origin, basePath } = splitAddr(addr);
    let h = href;
    if (/^https?:\/\//i.test(h)) {
        if (h.startsWith(origin)) {
            h = h.slice(origin.length);
        } else {
            const scheme = h.indexOf("://");
            const slash = h.indexOf("/", scheme + 3);
            h = slash === -1 ? "/" : h.slice(slash);
        }
    }
    if (h.startsWith(basePath)) {
        h = h.slice(basePath.length);
    }
    if (!h.startsWith("/")) h = "/" + h;
    return h;
}

// ---------------------------------------------------------------------------
// Digest auth (RFC 7616 subset: MD5 / MD5-sess, qop=auth)
// ---------------------------------------------------------------------------

function parseChallenge(header: string): Map<string, string> | null {
    const m = /^\s*Digest\s*(.*)$/i.exec(header);
    if (!m) return null;
    const params = new Map<string, string>();
    const re = /([a-zA-Z0-9_-]+)\s*=\s*(?:"((?:[^"\\]|\\.)*)"|([^\s,"]+))/g;
    let mm: RegExpExecArray | null;
    while ((mm = re.exec(m[1])) !== null) {
        params.set(mm[1].toLowerCase(), mm[2] !== undefined ? mm[2] : mm[3]);
    }
    return params;
}

function randomHex(len: number): string {
    let out = "";
    for (let i = 0; i < len; i++) {
        out += ((Math.random() * 16) | 0).toString(16);
    }
    return out;
}

/** Build the `Authorization` header for a cached challenge, or `null` when
 * the scheme is unsupported. `uri` is the request path (as sent). */
function authorizationFor(
    addr: string,
    scheme: string,
    username: string,
    password: string,
    uri: string,
    method: string,
): string | null {
    if (/^\s*Basic/i.test(scheme)) {
        return basicHeader(username, password);
    }
    const p = parseChallenge(scheme);
    if (!p) return null;
    const realm = p.get("realm") ?? "";
    const nonce = p.get("nonce") ?? "";
    const qop = (p.get("qop") ?? "").split(",").map((v) => v.trim()).includes("auth") ? "auth" : null;
    const opaque = p.get("opaque");
    const algorithm = (p.get("algorithm") ?? "MD5").toUpperCase();
    if (algorithm !== "MD5" && algorithm !== "MD5-SESS") {
        return null; // SHA-256 variants unsupported
    }

    let ha1 = md5(`${username}:${realm}:${password}`);
    const cnonce = randomHex(16);
    const entry = challenges.get(addr);
    const nc = ((entry?.nc ?? 0) + 1);
    const ncHex = nc.toString(16).padStart(8, "0");
    if (entry) entry.nc = nc;
    if (algorithm === "MD5-SESS") {
        ha1 = md5(`${ha1}:${nonce}:${cnonce}`);
    }
    const ha2 = md5(`${method}:${uri}`);
    const response = qop
        ? md5(`${ha1}:${nonce}:${ncHex}:${cnonce}:${qop}:${ha2}`)
        : md5(`${ha1}:${nonce}:${ha2}`);

    let header =
        `Digest username="${username}", realm="${realm}", nonce="${nonce}", ` +
        `uri="${uri}", algorithm=${algorithm === "MD5-SESS" ? "MD5-sess" : "MD5"}, ` +
        `response="${response}"`;
    if (qop) header += `, qop=${qop}, nc=${ncHex}, cnonce="${cnonce}"`;
    if (opaque != null) header += `, opaque="${opaque}"`;
    return header;
}

function basicHeader(username: string, password: string): string {
    return "Basic " + Base64.encode(`${username}:${password}`);
}

// ---------------------------------------------------------------------------
// HTTP core (challenge/retry wrapper around tur:net)
// ---------------------------------------------------------------------------

interface DavOpts {
    method: string;
    url: string;
    /** Request path as sent (for Digest uri). */
    uri: string;
    headers?: Record<string, string>;
    body?: string;
}

interface DavResponse {
    status: number;
    statusText: string;
    headers: Record<string, string>;
    bodyText: string;
}

function headerOf(headers: Record<string, string>, name: string): string | undefined {
    const lower = name.toLowerCase();
    for (const k of Object.keys(headers)) {
        if (k.toLowerCase() === lower) return headers[k];
    }
    return undefined;
}

function isTimeoutMessage(message: string): boolean {
    return /timed?\s?out|timeout/i.test(message);
}

/** Mark transport/auth errors with the prefixes the host maps to typed
 * errors. `fallback` wraps everything else. */
function markedError(e: unknown): Error {
    if (e instanceof HttpError) {
        if (e.status === 401) {
            return new Error(`UNAUTHORIZED: HTTP 401: ${e.message}`);
        }
        return new Error(`HTTP ${e.status}: ${e.message}`);
    }
    const msg = String((e as any)?.message ?? e);
    if (isTimeoutMessage(msg)) {
        return new Error(`TIMEOUT: ${msg}`);
    }
    return new Error(msg);
}

/**
 * Perform a single-shot WebDAV request with the challenge/retry flow:
 * preemptive Basic when credentials exist, one 401 retry answering the
 * server's `WWW-Authenticate` challenge (Basic or Digest).
 */
async function davRequest(
    addr: string,
    username: string,
    password: string,
    isAnonymous: boolean,
    opts: DavOpts,
): Promise<DavResponse> {
    const attempt = async (authScheme: string | null): Promise<DavResponse> => {
        const headers: Record<string, string> = { ...(opts.headers ?? {}) };
        if (authScheme != null && !isAnonymous) {
            const auth = authorizationFor(addr, authScheme, username, password, opts.uri, opts.method);
            if (auth != null) headers["Authorization"] = auth;
        }
        const resp = await request({
            url: opts.url,
            method: opts.method,
            headers,
            ...(opts.body !== undefined ? { body: opts.body } : {}),
        }).promise;
        if (resp.status >= 400) {
            const wwwAuth = headerOf(resp.headers, "WWW-Authenticate");
            throw Object.assign(
                new HttpError(resp.status, `${resp.statusText || ""} ${wwwAuth ?? ""}`.trim()),
                { wwwAuthenticate: wwwAuth },
            );
        }
        return {
            status: resp.status,
            statusText: resp.statusText,
            headers: resp.headers,
            bodyText: decodeUtf8(resp.body),
        };
    };

    let r: DavResponse;
    try {
        // Preemptive Basic when we have credentials and no cached Digest
        // challenge; unauthenticated otherwise (anonymous / first contact).
        const cached = challenges.get(addr);
        const scheme = cached ? cached.header : (!isAnonymous && username ? "Basic" : null);
        r = await attempt(scheme);
    } catch (e: any) {
        const wwwAuth: string | undefined = e?.wwwAuthenticate;
        if (e instanceof HttpError && e.status === 401 && wwwAuth && !isAnonymous) {
            challenges.set(addr, { header: wwwAuth, nc: 0 });
            r = await attempt(wwwAuth);
        } else {
            throw e;
        }
    }
    return r;
}

// ---------------------------------------------------------------------------
// PROPFIND + multistatus parsing (fast-xml-parser)
// ---------------------------------------------------------------------------

const PROPFIND_BODY =
    '<?xml version="1.0" ?>\n' +
    '<D:propfind xmlns:D="DAV:">\n' +
    "  <D:allprop/>\n" +
    "</D:propfind>";

interface RawEntry {
    href: string;
    displayName?: string;
    isDir: boolean;
    size?: number;
}

// fast-xml-parser configuration:
// - `removeNSPrefix` normalizes `D:href` / `d:href` / bare `href` alike (the
//   old regex scraper was prefix-agnostic too).
// - `parseTagValue: false` keeps tag text verbatim — strnum would otherwise
//   turn a displayname of "123" into a number.
// - response/propstat always become arrays: servers reply with a single
//   <D:response> or with 200 + 404 propstat blocks, and merging every
//   propstat's props mirrors the old whole-block scan.
const xmlParser = new XMLParser({
    removeNSPrefix: true,
    parseTagValue: false,
    isArray: (name) => /(?:^|:)(response|propstat)$/i.test(name),
});

/** Case-insensitive child lookup — servers occasionally vary prop-name
 *  casing even though RFC 4918 defines them lowercase. Returns the first
 *  match; used for leaf values. */
function pick(obj: unknown, name: string): unknown {
    if (obj == null || typeof obj !== "object") return undefined;
    for (const k of Object.keys(obj)) {
        if (k.toLowerCase() === name) return (obj as Record<string, unknown>)[k];
    }
    return undefined;
}

/** All children whose key case-insensitively matches `name`, non-array
 *  values wrapped — like `pick`, but merging casing variants instead of
 *  stopping at the first. Used for the response/propstat levels. */
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

/** Parse a PROPFIND `207 Multistatus` body — real XML semantics (nesting,
 *  attributes, self-closing tags, CDATA + entity decoding) via
 *  fast-xml-parser, replacing the previous regex scraping. Throws on
 *  malformed bodies — a silent `[]` here once hid a runtime gap (missing
 *  `substr` in boa) as an "empty directory" for weeks of debugging. */
function parseMultistatus(xml: string): RawEntry[] {
    const parsed: unknown = xmlParser.parse(xml);
    const out: RawEntry[] = [];
    for (const response of pickAll(parsed, "multistatus").flatMap((ms) => pickAll(ms, "response"))) {
        const href = pick(response, "href");
        if (typeof href !== "string") continue;
        const props: Record<string, unknown> = {};
        for (const ps of pickAll(response, "propstat")) {
            Object.assign(props, pick(ps, "prop") ?? {});
        }
        const resourcetype = pick(props, "resourcetype");
        const isDir =
            resourcetype != null &&
            typeof resourcetype === "object" &&
            pick(resourcetype, "collection") != null;
        let size: number | undefined;
        const lengthStr = pick(props, "getcontentlength");
        if (typeof lengthStr === "string" && /^\d+$/.test(lengthStr.trim())) {
            size = parseInt(lengthStr.trim(), 10);
        }
        const displayName = pick(props, "displayname");
        out.push({
            href: href.trim(),
            displayName: typeof displayName === "string" ? displayName : undefined,
            isDir,
            size,
        });
    }
    return out;
}

interface Entry {
    name: string;
    path: string;
    size?: number;
    isDir: boolean;
}

function urlDecode(s: string): string {
    try {
        return decodeURIComponent(s);
    } catch {
        return s;
    }
}

async function listImpl(conf: InstanceConfig, dir: string): Promise<Entry[]> {
    const password = loadPassword(conf);
    const url = buildUrl(conf.addr, dir, true);
    const resp = await davRequest(conf.addr, conf.username, password, conf.isAnonymous, {
        method: "PROPFIND",
        url,
        uri: url.substring(splitAddr(conf.addr).origin.length),
        headers: {
            "Content-Type": "application/xml",
            Accept: "application/xml",
            Depth: "1",
        },
        body: PROPFIND_BODY,
    });

    const raw = parseMultistatus(resp.bodyText);
    const out: Entry[] = [];
    const dirTrimmed = dir.endsWith("/") ? dir.slice(0, -1) : dir;
    for (const item of raw) {
        let path = hrefToPath(conf.addr, item.href);
        if (path === "/" || path === "") continue;
        if (path.endsWith("/")) path = path.slice(0, -1);
        if (path === dirTrimmed) continue;
        let name = item.displayName ?? "";
        if (name === "") {
            const parts = path.split("/");
            name = parts[parts.length - 1] ?? path;
        } else {
            name = urlDecode(name);
        }
        out.push({ name, path, size: item.size, isDir: item.isDir });
    }

    // dirs first, then by path — matches the previous Rust ordering.
    out.sort((a, b) => {
        if (a.isDir !== b.isDir) return a.isDir ? -1 : 1;
        if (a.path < b.path) return -1;
        if (a.path > b.path) return 1;
        return 0;
    });
    return out;
}

// ---------------------------------------------------------------------------
// get (streaming byte-range download)
// ---------------------------------------------------------------------------

function parseTotalLength(headers: Record<string, string>): number | undefined {
    // Prefer Content-Range's total (`bytes start-end/total`) — it gives the
    // full file size; Content-Length on a Range response is the partial length.
    const cr = headerOf(headers, "Content-Range");
    if (cr) {
        const m = /\/(\d+)\s*$/.exec(cr);
        if (m) return parseInt(m[1], 10);
    }
    const cl = headerOf(headers, "Content-Length");
    if (cl && /^\d+$/.test(cl.trim())) return parseInt(cl.trim(), 10);
    return undefined;
}

/**
 * Streaming get opener: one ranged request per stream. The dispatcher pumps
 * `body` with host-granted credits and calls `release` on any exit (host
 * cancel included) — `t.cancel()` wire-aborts the download. When the server
 * ignores the Range request (full 200 body from byte 0), `meta.dataOffset: 0`
 * tells the host to drop the `offset` prefix bytes itself.
 */
async function openGet(
    conf: InstanceConfig,
    path: string,
    offset: number,
): Promise<StreamSource> {
    const password = loadPassword(conf);
    const { origin } = splitAddr(conf.addr);
    const url = buildUrl(conf.addr, path, false);
    const uri = url.substring(origin.length);

    // Keep the Task, not just the promise — it is the abort handle.
    const attempt = (authScheme: string | null) => {
        const headers: Record<string, string> = { Range: `bytes=${offset}-` };
        if (authScheme != null && !conf.isAnonymous) {
            const auth = authorizationFor(conf.addr, authScheme, conf.username, password, uri, "GET");
            if (auth != null) headers["Authorization"] = auth;
        }
        return requestStream({ url, method: "GET", headers });
    };

    let t = attempt(challenges.get(conf.addr)?.header ??
        (!conf.isAnonymous && conf.username ? "Basic" : null));
    let resp: StreamResponse;
    try {
        // requestStream's promise rejects (object with `message`) on transport
        // errors; HTTP-level failures resolve with a status instead.
        resp = await t.promise;
    } catch (e: any) {
        throw markedError(e);
    }
    if (resp.status === 401 && !conf.isAnonymous) {
        const wwwAuth = headerOf(resp.headers, "WWW-Authenticate");
        if (wwwAuth) {
            challenges.set(conf.addr, { header: wwwAuth, nc: 0 });
            t.cancel(); // discard the 401 body before the Digest retry
            t = attempt(wwwAuth);
            resp = await t.promise;
        }
    }
    if (resp.status >= 400) {
        // Fail the RPC itself (throw) so the host's `get` returns Err —
        // callers treat missing entries as `None`. The error rides the reply,
        // not the byte stream.
        t.cancel();
        const msg = `get: ${resp.status} ${resp.statusText ?? ""}`.trim();
        throw new HttpError(resp.status, resp.status === 401 ? `UNAUTHORIZED: ${msg}` : msg);
    }

    const contentRange = headerOf(resp.headers, "Content-Range");
    const rangeHonored = contentRange != null || resp.status === 206;

    return {
        meta: {
            totalLength: parseTotalLength(resp.headers),
            name: path.split("/").pop() || path,
            contentType: headerOf(resp.headers, "Content-Type"),
            // The host assumes pushed chunks start at `offset`; a server that
            // ignored the Range request sends them from 0 — say so, and the
            // host drops the prefix.
            dataOffset: rangeHonored ? offset : 0,
        },
        body: resp.body,
        release: () => t.cancel(), // wire abort on any pump exit; no-op when done
        mapError: (e) => markedError(e), // keep TIMEOUT classification mid-body
    };
}

// ---------------------------------------------------------------------------
// test / connect / instance lifecycle
// ---------------------------------------------------------------------------

type TestOutcome = "SUCCESS" | "UNAUTHORIZED" | "TIMEOUT" | "OTHER_ERROR";

/** Resolve the credentials for a test call: explicit values win; on edit a
 * blank password falls back to the stored one. */
function testCredentials(
    storageId: string | undefined,
    addr: string,
    username: string,
    password: string,
): { addr: string; username: string; password: string } {
    if (password !== "" || storageId == null) {
        return { addr, username, password };
    }
    const conf = configOf(storageId);
    return {
        addr: addr !== "" ? addr : conf.addr,
        username: username !== "" ? username : conf.username,
        password: loadPassword(conf),
    };
}

// Handler args shapes (see the op table in the header comment). Host-op
// `pluginId` is literal-typed — a mismatch would mean the host routed
// somebody else's call here.

interface ListArgs {
    pluginId: "com.ease.webdav";
    storageId: string;
    dir: string;
}

interface GetArgs {
    pluginId: "com.ease.webdav";
    storageId: string;
    path: string;
    offset: number;
}

interface TestArgs {
    storageId?: string;
    addr: string;
    username: string;
    password?: string;
    isAnonymous?: boolean;
}

interface ConnectArgs {
    storageId?: string;
    addr: string;
    alias?: string;
    username?: string;
    password?: string;
    isAnonymous?: boolean;
}

interface RemoveInstanceArgs {
    pluginId: "com.ease.webdav";
    storageId: string;
}

async function testImpl(args: TestArgs): Promise<{ result: TestOutcome }> {
    const anon = !!args.isAnonymous;
    const cred = testCredentials(args.storageId, args.addr, args.username, args.password ?? "");
    try {
        const url = buildUrl(cred.addr, "/", true);
        await davRequest(cred.addr, cred.username, cred.password, anon, {
            method: "PROPFIND",
            url,
            uri: url.substring(splitAddr(cred.addr).origin.length),
            headers: {
                "Content-Type": "application/xml",
                Accept: "application/xml",
                Depth: "1",
            },
            body: PROPFIND_BODY,
        });
        return { result: "SUCCESS" };
    } catch (e) {
        const msg = String((e as any)?.message ?? e);
        if (e instanceof HttpError && e.status === 401) return { result: "UNAUTHORIZED" };
        if (isTimeoutMessage(msg)) return { result: "TIMEOUT" };
        return { result: "OTHER_ERROR" };
    }
}

// (id minting: `uuidv4()` from the uuid package — `crypto.getRandomValues`
// is provided as a Web Platform global by the host)

/**
 * Create or update a WebDAV instance. On create (no `storageId`), persist the
 * config + password secret, mint `webdav:<uuid>`, and register the host
 * storage row (`context.createStorage` — the host pops the create form). On
 * update, rewrite the kv config; a blank `password` keeps the stored secret.
 */
function connectImpl(args: ConnectArgs): { storageId: string; created: boolean } {
    const isAnonymous = !!args.isAnonymous;
    const addr = args.addr.trim();
    const alias = (args.alias ?? "").trim() || "WebDAV";
    const username = isAnonymous ? "" : (args.username ?? "").trim();
    const password = isAnonymous ? "" : (args.password ?? "");

    if (addr === "") {
        throw new Error("webdav: address cannot be empty");
    }
    if (!isAnonymous && username === "") {
        throw new Error("webdav: username cannot be empty");
    }

    if (args.storageId != null) {
        // Update: keep the existing secret unless a new password is given.
        const conf = configOf(args.storageId);
        let secretId = conf.secretId;
        if (password !== "") {
            const newId = secret.put(password);
            if (secretId != null) secret.remove(secretId);
            secretId = newId;
        }
        const updated: InstanceConfig = {
            alias,
            addr,
            username,
            isAnonymous,
            secretId,
            password: password !== "" ? password : null,
        };
        db.singleSet(kvKey(args.storageId), JSON.stringify(configToJson(updated)));
        instances.set(args.storageId, updated);
        context.notifyChange();
        return { storageId: args.storageId, created: false };
    }

    // Create: password required for non-anonymous servers.
    if (!isAnonymous && password === "") {
        throw new Error("webdav: password cannot be empty");
    }
    const instance = `webdav:${uuidv4()}`;
    const secretId = password !== "" ? secret.put(password) : null;
    const conf: InstanceConfig = {
        alias,
        addr,
        username,
        isAnonymous,
        secretId,
        password: password !== "" ? password : null,
    };
    db.singleSet(kvKey(instance), JSON.stringify(configToJson(conf)));
    instances.set(instance, conf);
    // Register the host storage row; the upcall pops the create form.
    context.createStorage(instance);
    return { storageId: instance, created: true };
}

function configToJson(conf: InstanceConfig): Record<string, unknown> {
    return {
        alias: conf.alias,
        addr: conf.addr,
        username: conf.username,
        isAnonymous: conf.isAnonymous,
        secretId: conf.secretId,
    };
}

/** Remove an instance: drop its config (kv) + password secret, ask the host
 * to delete the storage row, and reload the dashboard. Called from the
 * host trash button (`storage:removeInstance` via `storage_plugin.remove_instance`). */
function removeInstance(args: RemoveInstanceArgs): void {
    const conf = instances.get(args.storageId);
    let secretId: number | null | undefined = conf?.secretId;
    if (secretId === undefined) {
        const raw = db.singleGet(kvKey(args.storageId));
        if (raw != null) {
            try {
                secretId = JSON.parse(raw).secretId ?? null;
            } catch {
                secretId = null;
            }
        }
    }
    if (secretId != null) {
        secret.remove(secretId);
    }
    db.singleDelete(kvKey(args.storageId));
    instances.delete(args.storageId);
    challenges.delete(conf?.addr ?? "");
    // Complete the disconnect on the host side: drop the storage row, then
    // reload so the dashboard + edit page reflect the removal.
    context.removeStorage(args.storageId);
    context.notifyChange();
}

// ---------------------------------------------------------------------------
// Register handlers
//
// The module lifecycle contract: the engine calls `start()` after eval (and
// runs the returned cleanup before the next load / at destroy). Handlers are
// per-instance and die with the instance, so no cleanup is needed.
// ---------------------------------------------------------------------------

export function start(): void {
    hostRpc.registerHandler("storage:list", (args: ListArgs) =>
        listImpl(configOf(args.storageId), args.dir).catch((e: any) => {
            throw markedError(e);
        }),
    );

    hostRpc.registerStream("storage:get", (args: GetArgs) =>
        openGet(configOf(args.storageId), args.path, args.offset).catch((e: any) => {
            throw markedError(e);
        }),
    );

    viewRpc.registerHandler("webdav:test", (args: TestArgs) => testImpl(args));

    viewRpc.registerHandler("webdav:connect", (args: ConnectArgs) => connectImpl(args));

    hostRpc.registerHandler("storage:removeInstance", (args: RemoveInstanceArgs) => {
        removeInstance(args);
        return {};
    });
}
