// WebDAV storage provider — a headless JS plugin that serves `list` and `get`
// (streaming byte-range download) over the `tur:rpc` channel, plus a
// username/password connect flow (no OAuth). The host `JsStorageBackend`
// treats WebDAV like any other `StorageBackend`.
//
// Multi-instance: each configured WebDAV server is one *instance* named
// `webdav:<uuid>` (the storage row's `plugin_storage_id`; legacy-migrated
// rows use `webdav:<legacy-id>`). Per-instance config lives in `ease.db`
// (this plugin's KV) under `storage:<instance>` = JSON
// `{ alias, addr, username, isAnonymous, secretId }`; the password lives in
// `ease.secret` under that `secretId` (scope `plugin:com.ease.webdav`).
//
// Identity: this headless instance is created by `KeepBackendService` which
// stamps `PluginId("com.ease.webdav")` into the per-instance data slot.
// `ease.*` bridge fns resolve the calling plugin from that slot — no
// pluginId argument is needed (or accepted) on any call here.
//
// Host handlers (under the `webdav:` prefix):
//   - webdav:list           { instance, dir }                    -> Entry[]
//   - webdav:get            { instance, streamId, path, offset } -> { totalLength?, name?, contentType? }
//   - webdav:test           { instance?, addr, username, password?, isAnonymous } -> { result }
//   - webdav:connect        { instance?, addr, alias, username, password?, isAnonymous } -> { instance, created }
//   - webdav:removeInstance { instance }                          -> {}
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
import { registerHandler, pushChunk, endStream, errorStream } from "tur:rpc";
import { db, secret, context } from "ease";

import { md5Hex } from "./vendor/md5";
import { base64EncodeUtf8 } from "./vendor/base64";

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
            for (const b of Array.from(ch)) {
                const code = b.codePointAt(0)!;
                if (code < 0x80) {
                    out += "%" + code.toString(16).toUpperCase().padStart(2, "0");
                } else {
                    // multi-byte — encode via UTF-8 bytes
                    for (const byte of utf8Bytes(ch)) {
                        out += "%" + byte.toString(16).toUpperCase().padStart(2, "0");
                    }
                }
            }
        }
    }
    return out;
}

function utf8Bytes(s: string): number[] {
    const out: number[] = [];
    for (let i = 0; i < s.length; i++) {
        let cp = s.codePointAt(i)!;
        if (cp > 0xffff) i++;
        if (cp < 0x80) out.push(cp);
        else if (cp < 0x800) out.push(0xc0 | (cp >> 6), 0x80 | (cp & 0x3f));
        else if (cp < 0x10000) out.push(0xe0 | (cp >> 12), 0x80 | ((cp >> 6) & 0x3f), 0x80 | (cp & 0x3f));
        else out.push(0xf0 | (cp >> 18), 0x80 | ((cp >> 12) & 0x3f), 0x80 | ((cp >> 6) & 0x3f), 0x80 | (cp & 0x3f));
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

    let ha1 = md5Hex(`${username}:${realm}:${password}`);
    const cnonce = randomHex(16);
    const entry = challenges.get(addr);
    const nc = ((entry?.nc ?? 0) + 1);
    const ncHex = nc.toString(16).padStart(8, "0");
    if (entry) entry.nc = nc;
    if (algorithm === "MD5-SESS") {
        ha1 = md5Hex(`${ha1}:${nonce}:${cnonce}`);
    }
    const ha2 = md5Hex(`${method}:${uri}`);
    const response = qop
        ? md5Hex(`${ha1}:${nonce}:${ncHex}:${cnonce}:${qop}:${ha2}`)
        : md5Hex(`${ha1}:${nonce}:${ha2}`);

    let header =
        `Digest username="${username}", realm="${realm}", nonce="${nonce}", ` +
        `uri="${uri}", algorithm=${algorithm === "MD5-SESS" ? "MD5-sess" : "MD5"}, ` +
        `response="${response}"`;
    if (qop) header += `, qop=${qop}, nc=${ncHex}, cnonce="${cnonce}"`;
    if (opaque != null) header += `, opaque="${opaque}"`;
    return header;
}

function basicHeader(username: string, password: string): string {
    return "Basic " + base64EncodeUtf8(`${username}:${password}`);
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
            responseType: "text",
        });
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
            bodyText: (resp as any).bodyText ?? "",
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
// PROPFIND + minimal multistatus XML parsing
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

function decodeXmlText(s: string): string {
    const cdata = /^<!\[CDATA\[([\s\S]*)\]\]>$/g.exec(s.trim());
    if (cdata) return cdata[1];
    return s
        .replace(/&#x([0-9a-fA-F]+);/g, (_, h) => String.fromCodePoint(parseInt(h, 16)))
        .replace(/&#(\d+);/g, (_, d) => String.fromCodePoint(parseInt(d, 10)))
        .replace(/&lt;/g, "<")
        .replace(/&gt;/g, ">")
        .replace(/&quot;/g, '"')
        .replace(/&apos;/g, "'")
        .replace(/&amp;/g, "&");
}

function tagContent(block: string, localName: string): string | undefined {
    const re = new RegExp(
        `<(?:[A-Za-z0-9_.-]+:)?${localName}(?:\\s[^>]*)?>([\\s\\S]*?)</(?:[A-Za-z0-9_.-]+:)?${localName}\\s*>`,
        "i",
    );
    const m = re.exec(block);
    return m ? decodeXmlText(m[1]) : undefined;
}

function parseMultistatus(xml: string): RawEntry[] {
    const out: RawEntry[] = [];
    const responseRe =
        /<(?:[A-Za-z0-9_.-]+:)?response(?:\s[^>]*)?>([\s\S]*?)<\/(?:[A-Za-z0-9_.-]+:)?response\s*>/gi;
    let m: RegExpExecArray | null;
    while ((m = responseRe.exec(xml)) !== null) {
        const block = m[1];
        const href = tagContent(block, "href");
        if (href == null) continue;
        const resourcetype = new RegExp(
            "<(?:[A-Za-z0-9_.-]+:)?resourcetype(?:\\s[^>]*)?>([\\s\\S]*?)</(?:[A-Za-z0-9_.-]+:)?resourcetype\\s*>",
            "i",
        ).exec(block);
        const isDir = resourcetype != null && /<(?:[A-Za-z0-9_.-]+:)?collection(\s|\/|>)/i.test(resourcetype[1]);
        const lengthStr = tagContent(block, "getcontentlength");
        let size: number | undefined;
        if (lengthStr != null && /^\d+$/.test(lengthStr.trim())) {
            size = parseInt(lengthStr.trim(), 10);
        }
        out.push({
            href: href.trim(),
            displayName: tagContent(block, "displayname"),
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

async function getImpl(
    conf: InstanceConfig,
    streamId: number,
    path: string,
    offset: number,
): Promise<{ totalLength?: number; name?: string; contentType?: string }> {
    const password = loadPassword(conf);
    const { origin } = splitAddr(conf.addr);
    const url = buildUrl(conf.addr, path, false);
    const uri = url.substring(origin.length);

    const attempt = async (authScheme: string | null) => {
        const headers: Record<string, string> = { Range: `bytes=${offset}-` };
        if (authScheme != null && !conf.isAnonymous) {
            const auth = authorizationFor(conf.addr, authScheme, conf.username, password, uri, "GET");
            if (auth != null) headers["Authorization"] = auth;
        }
        return requestStream({ url, method: "GET", headers });
    };

    let resp: any;
    try {
        const cached = challenges.get(conf.addr);
        const scheme = cached ? cached.header : (!conf.isAnonymous && conf.username ? "Basic" : null);
        resp = await attempt(scheme);
    } catch (e: any) {
        // requestStream rejects (object with `message`) on transport errors;
        // HTTP-level failures resolve with a status instead.
        throw markedError(e);
    }
    if (resp.status === 401 && !conf.isAnonymous) {
        const wwwAuth = headerOf(resp.headers, "WWW-Authenticate");
        if (wwwAuth) {
            challenges.set(conf.addr, { header: wwwAuth, nc: 0 });
            resp = await attempt(wwwAuth);
        }
    }
    if (resp.status >= 400) {
        // Fail the RPC itself (throw) so the host's `get` returns Err —
        // callers treat missing entries as `None`. (errorStream would defer
        // the error into the byte stream, which stream consumers may not
        // expect.)
        const msg = `get: ${resp.status} ${resp.statusText ?? ""}`.trim();
        throw new HttpError(resp.status, resp.status === 401 ? `UNAUTHORIZED: ${msg}` : msg);
    }

    const contentRange = headerOf(resp.headers, "Content-Range");
    const rangeHonored = contentRange != null || resp.status === 206;
    const totalLength = parseTotalLength(resp.headers);
    const contentType = headerOf(resp.headers, "Content-Type");
    const name = path.split("/").pop() || path;

    // When the server ignored the Range request (full 200 body) and an
    // offset was requested, skip the offset ourselves — the host assumes
    // pushed chunks start at `offset`.
    const skip = rangeHonored ? 0 : offset;

    (async () => {
        try {
            let remaining = skip;
            for await (const chunk of resp.body) {
                let c: Uint8Array = chunk;
                if (remaining > 0) {
                    if (c.length <= remaining) {
                        remaining -= c.length;
                        continue;
                    }
                    c = c.subarray(remaining);
                    remaining = 0;
                }
                pushChunk(streamId, c);
            }
            endStream(streamId);
        } catch (e: any) {
            const msg = String(e?.message ?? e);
            errorStream(streamId, isTimeoutMessage(msg) ? `TIMEOUT: ${msg}` : msg);
        }
    })();

    return { totalLength, name, contentType };
}

// ---------------------------------------------------------------------------
// test / connect / instance lifecycle
// ---------------------------------------------------------------------------

type TestOutcome = "SUCCESS" | "UNAUTHORIZED" | "TIMEOUT" | "OTHER_ERROR";

/** Resolve the credentials for a test call: explicit values win; on edit a
 * blank password falls back to the stored one. */
function testCredentials(
    instance: string | undefined,
    addr: string,
    username: string,
    password: string,
): { addr: string; username: string; password: string } {
    if (password !== "" || instance == null) {
        return { addr, username, password };
    }
    const conf = configOf(instance);
    return {
        addr: addr !== "" ? addr : conf.addr,
        username: username !== "" ? username : conf.username,
        password: loadPassword(conf),
    };
}

async function testImpl(args: {
    instance?: string;
    addr: string;
    username: string;
    password?: string;
    isAnonymous?: boolean;
}): Promise<{ result: TestOutcome }> {
    const anon = !!args.isAnonymous;
    const cred = testCredentials(args.instance, args.addr, args.username, args.password ?? "");
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

/** RFC 4122 v4 UUID (Math.random is plenty for non-adversarial id minting). */
function uuid(): string {
    const hex = "0123456789abcdef";
    let out = "";
    for (let i = 0; i < 36; i++) {
        if (i === 8 || i === 13 || i === 18 || i === 23) {
            out += "-";
        } else if (i === 14) {
            out += "4";
        } else if (i === 19) {
            out += hex[((Math.random() * 4) | 0) | 8];
        } else {
            out += hex[(Math.random() * 16) | 0];
        }
    }
    return out;
}

/**
 * Create or update a WebDAV instance. On create (no `instance`), persist the
 * config + password secret, mint `webdav:<uuid>`, and register the host
 * storage row (`context.createStorage` — the host pops the create form). On
 * update, rewrite the kv config; a blank `password` keeps the stored secret.
 */
function connectImpl(args: {
    instance?: string;
    addr: string;
    alias?: string;
    username?: string;
    password?: string;
    isAnonymous?: boolean;
}): { instance: string; created: boolean } {
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

    if (args.instance != null) {
        // Update: keep the existing secret unless a new password is given.
        const conf = configOf(args.instance);
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
        db.singleSet(kvKey(args.instance), JSON.stringify(configToJson(updated)));
        instances.set(args.instance, updated);
        context.notifyChange();
        return { instance: args.instance, created: false };
    }

    // Create: password required for non-anonymous servers.
    if (!isAnonymous && password === "") {
        throw new Error("webdav: password cannot be empty");
    }
    const instance = `webdav:${uuid()}`;
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
    return { instance, created: true };
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
 * host trash button (`webdav:removeInstance` via `storage_plugin.remove_instance`). */
function removeInstance(args: { instance: string }): void {
    const conf = instances.get(args.instance);
    let secretId: number | null | undefined = conf?.secretId;
    if (secretId === undefined) {
        const raw = db.singleGet(kvKey(args.instance));
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
    db.singleDelete(kvKey(args.instance));
    instances.delete(args.instance);
    challenges.delete(conf?.addr ?? "");
    // Complete the disconnect on the host side: drop the storage row, then
    // reload so the dashboard + edit page reflect the removal.
    context.removeStorage(args.instance);
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
    registerHandler("webdav:list", (args) =>
        listImpl(configOf(args.instance), args.dir).catch((e: any) => {
            throw markedError(e);
        }),
    );

    registerHandler("webdav:get", (args) => {
        const { instance, streamId, path, offset } = args;
        return getImpl(configOf(instance), streamId, path, offset).catch((e: any) => {
            const err = markedError(e);
            errorStream(streamId, String(err?.message ?? err));
            return {};
        });
    });

    registerHandler("webdav:test", (args) => testImpl(args));

    registerHandler("webdav:connect", (args) => connectImpl(args));

    registerHandler("webdav:removeInstance", (args) => {
        removeInstance(args);
        return {};
    });
}
