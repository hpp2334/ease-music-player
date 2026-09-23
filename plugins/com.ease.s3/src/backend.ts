// S3 / S3-compatible storage provider — a headless JS plugin that serves
// `list` (ListObjectsV2 with a `/` delimiter) and `get` (streaming
// byte-range download), both SigV4-signed, over the `tur:rpc` channel, plus
// an access-key/secret-key connect flow (no OAuth). Any S3-compatible
// endpoint works: AWS S3, Cloudflare R2, MinIO, B2, … — addressing is always
// path-style (`<endpoint>/<bucket>/<key>`), which every flavor accepts.
//
// Multi-instance: each configured endpoint+bucket is one *instance* named
// `s3:<uuid>` (the storage row's `plugin_storage_id`). Per-instance config
// lives in `ease.db` (this plugin's KV) under `storage:<instance>` = JSON
// `{ alias, endpoint, region, bucket, accessKeyId, secretId }`; the secret
// access key lives in `ease.secret` under that `secretId`
// (scope `plugin:com.ease.s3`).
//
// Identity: this headless instance is created by `KeepBackendService` which
// stamps `PluginId("com.ease.s3")` into the per-instance data slot. `ease.*`
// bridge fns resolve the calling plugin from that slot — no pluginId
// argument is needed (or accepted) on any call here.
//
// Handlers, split by caller (the dispatcher routes strictly by scope). The
// host-called ops are contract literals — identical names for every storage
// provider, identity riding the payload (`pluginId` = this manifest's id,
// `storageId` = the `plugin_storage_id` instance) — typed by the shared sigs
// in `../../infra/host-ops.ts` (args/results below describe those sigs):
//
// hostRpc — the Rust host invokes these:
//   - storage:list           { pluginId, storageId, dir }          -> Entry[]
//   - storage:get            { pluginId, storageId, path, offset } -> registerStream: meta
//     { totalLength?, name?, contentType?, dataOffset? } + credit-gated body
//   - storage:removeInstance { pluginId, storageId }               -> {}
//
// viewRpc — this plugin's own view invokes these via `ease.rpc.call`
// (plugin-private sigs in `./rpc.ts`):
//   - s3:test      { storageId?, endpoint, region?, bucket, accessKeyId, secretAccessKey? } -> { result }
//   - s3:connect   { storageId?, endpoint, region?, bucket, alias?, accessKeyId, secretAccessKey? }
//                  -> { storageId, created }
//
// Signing: every request is bodyless GET, so the payload hash is the
// empty-string SHA-256 constant; signed headers are
// `host;range;x-amz-content-sha256;x-amz-date` (range only on downloads).
// The URL and the canonical request share the same encoded path + query
// strings byte-for-byte (see `s3path.ts` / `sigv4.ts`). The `host` header is
// signed but never sent — reqwest derives it from the URL, so its value must
// be the URL authority after default-port normalization.
//
// Error contract: messages prefixed `UNAUTHORIZED` / `TIMEOUT` are mapped by
// the host (`ease-js-storage`) to typed errors so the UI distinguishes auth
// failures (S3 signals bad keys / bad signatures as 403) and timeouts. S3
// `<Error>` documents surface their Code (e.g. `NoSuchKey`,
// `RequestTimeTooSkewed`).

import { request, requestStream } from "tur:net";
// TextEncoder/TextDecoder polyfill FIRST — npm deps below may rely on them.
import "../../infra/string-polyfill";
import "../../infra/text-polyfill";
import type { StreamResponse } from "tur:net";
import { decodeUtf8 } from "tur:std";
import { hostRpc, viewRpc } from "tur:rpc";
import type { EaseRpcOpArg, StreamSource } from "tur:rpc";
import {
    StorageListSig,
    StorageGetSig,
    StorageRemoveInstanceSig,
} from "../../infra/host-ops";
import type { StorageEntry, StorageGetMeta } from "../../infra/host-ops";
import { S3TestSig, S3ConnectSig } from "./rpc";
import type { TestOutcome, S3TestArgs, S3ConnectArgs } from "./rpc";
import { db, secret, context } from "ease";

// npm deps — bundled by rspack (only `tur:*` / `ease` are externals).
// `@noble/hashes` is pure JS over Uint8Array (no WebCrypto), so it runs in
// the boa runtime; `uuid` uses the host-installed `crypto.getRandomValues`.
import { v4 as uuidv4 } from "uuid";

import {
    EMPTY_PAYLOAD_SHA256,
    S3_SERVICE,
    amzTimestamps,
    canonicalQueryString,
    signRequest,
} from "./sigv4";
import {
    normalizeEndpoint,
    dirToPrefix,
    pathToKey,
    canonicalUri,
} from "./s3path";
import { parseListPage, pageToEntries, parseS3Error } from "./listing";

// ---------------------------------------------------------------------------
// Per-instance state
// ---------------------------------------------------------------------------

interface InstanceConfig {
    alias: string;
    endpoint: string;
    region: string;
    bucket: string;
    accessKeyId: string;
    secretId: number | null;
    /** Cached secret access key (from the secret store). */
    secretAccessKey: string | null;
}

/** instance ("s3:<uuid>") -> config. Lazily loaded on first use. */
const instances = new Map<string, InstanceConfig>();

const DEFAULT_REGION = "us-east-1";

class S3HttpError extends Error {
    constructor(
        public status: number,
        public code: string,
        message: string,
    ) {
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
        throw new Error(`s3: no config for instance ${instance}`);
    }
    const cfg = JSON.parse(raw);
    const conf: InstanceConfig = {
        alias: cfg.alias ?? instance,
        endpoint: cfg.endpoint ?? "",
        region: cfg.region ?? DEFAULT_REGION,
        bucket: cfg.bucket ?? "",
        accessKeyId: cfg.accessKeyId ?? "",
        secretId: cfg.secretId ?? null,
        secretAccessKey: null,
    };
    instances.set(instance, conf);
    return conf;
}

function loadSecret(conf: InstanceConfig): string {
    if (conf.secretAccessKey != null) return conf.secretAccessKey;
    if (conf.secretId == null) return "";
    const v = secret.get(conf.secretId);
    conf.secretAccessKey = v == null ? "" : v;
    return conf.secretAccessKey;
}

// ---------------------------------------------------------------------------
// Signed request core
// ---------------------------------------------------------------------------

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
 *  errors. S3 reports credential/signature failures as 401/403. */
function markedError(e: unknown): Error {
    if (e instanceof S3HttpError) {
        if (e.status === 401 || e.status === 403) {
            return new Error(`UNAUTHORIZED: HTTP ${e.status} ${e.code}: ${e.message}`);
        }
        return new Error(`HTTP ${e.status} ${e.code}: ${e.message}`);
    }
    const msg = String((e as any)?.message ?? e);
    if (isTimeoutMessage(msg)) {
        return new Error(`TIMEOUT: ${msg}`);
    }
    return new Error(msg);
}

interface SignedUrl {
    url: string;
    /** Headers to actually send (Authorization + x-amz-*; `host` is implied
     *  by the URL and never sent explicitly). */
    sendHeaders: Record<string, string>;
}

/** Build the URL + signed headers for one request. `extraHeaders` (lowercase
 *  names, e.g. `range`) join the signed set AND the wire set. */
function buildSignedRequest(
    conf: InstanceConfig,
    secretKey: string,
    key: string | null,
    query: Array<[string, string]>,
    extraHeaders: Record<string, string>,
): SignedUrl {
    const endpoint = normalizeEndpoint(conf.endpoint);
    const uri = canonicalUri(endpoint, conf.bucket, key);
    const canonicalQuery = canonicalQueryString(query);
    const { amzDate, dateStamp } = amzTimestamps(new Date());
    const region = conf.region || DEFAULT_REGION;

    const headers: Record<string, string> = {
        host: endpoint.host,
        "x-amz-content-sha256": EMPTY_PAYLOAD_SHA256,
        "x-amz-date": amzDate,
        ...extraHeaders,
    };
    const { authorization } = signRequest(
        {
            method: "GET",
            canonicalUri: uri,
            canonicalQuery,
            headers,
            amzDate,
            dateStamp,
            region,
            service: S3_SERVICE,
        },
        { accessKey: conf.accessKeyId, secretKey },
    );
    const url = endpoint.origin + uri + (canonicalQuery !== "" ? `?${canonicalQuery}` : "");
    const sendHeaders: Record<string, string> = {
        "x-amz-content-sha256": headers["x-amz-content-sha256"],
        "x-amz-date": headers["x-amz-date"],
        Authorization: authorization,
    };
    for (const [name, value] of Object.entries(extraHeaders)) {
        // Signed names are lowercase per the SigV4 spec; send conventional
        // HTTP casing on the wire (`Range`).
        sendHeaders[name === "range" ? "Range" : name] = value;
    }
    return { url, sendHeaders };
}

/** Signed bodyless GET returning the full body as text (list / test). */
async function s3GetText(
    conf: InstanceConfig,
    key: string | null,
    query: Array<[string, string]>,
): Promise<{ status: number; bodyText: string }> {
    const { url, sendHeaders } = buildSignedRequest(conf, loadSecret(conf), key, query, {});
    const resp = await request({ url, method: "GET", headers: sendHeaders }).promise;
    const bodyText = decodeUtf8(resp.body);
    if (resp.status >= 400) {
        const err = parseS3Error(bodyText);
        throw new S3HttpError(
            resp.status,
            err?.code ?? "",
            err?.message ?? `${resp.statusText || ""}`.trim(),
        );
    }
    return { status: resp.status, bodyText };
}

// ---------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------

/** Safety cap: 100 pages x 1000 keys = 100k entries per directory. */
const MAX_LIST_PAGES = 100;

async function listImpl(conf: InstanceConfig, dir: string): Promise<StorageEntry[]> {
    const prefix = dirToPrefix(dir);
    const out: StorageEntry[] = [];
    let token: string | null = null;
    let pages = 0;
    do {
        const params: Array<[string, string]> = [
            ["list-type", "2"],
            ["delimiter", "/"],
            ["max-keys", "1000"],
            ["prefix", prefix],
        ];
        if (token != null) params.push(["continuation-token", token]);
        const resp = await s3GetText(conf, null, params);
        const page = parseListPage(resp.bodyText);
        out.push(...pageToEntries(page, prefix));
        token = page.nextToken;
        pages += 1;
    } while (token != null && pages < MAX_LIST_PAGES);
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
 * Streaming get opener: one ranged request per stream, Range included in the
 * signed headers. The dispatcher pumps `body` with host-granted credits and
 * calls `release` on any exit (host cancel included) — `t.cancel()`
 * wire-aborts the download. S3 always honors Range (206 + Content-Range),
 * but a gateway that doesn't gets the same `dataOffset: 0` fallback as the
 * WebDAV plugin: the host drops the prefix bytes itself.
 */
async function openGet(
    conf: InstanceConfig,
    path: string,
    offset: number,
): Promise<StreamSource<StorageGetMeta>> {
    const { url, sendHeaders } = buildSignedRequest(
        conf,
        loadSecret(conf),
        pathToKey(path),
        [],
        { range: `bytes=${offset}-` },
    );
    // Keep the Task, not just the promise — it is the abort handle.
    const task = requestStream({ url, method: "GET", headers: sendHeaders });
    let resp: StreamResponse;
    try {
        // requestStream's promise rejects (object with `message`) on transport
        // errors; HTTP-level failures resolve with a status instead.
        resp = await task.promise;
    } catch (e: any) {
        throw markedError(e);
    }
    if (resp.status >= 400) {
        // Fail the RPC itself (throw) so the host's `get` returns Err —
        // callers treat missing entries as `None`. The error rides the reply,
        // not the byte stream.
        task.cancel();
        throw markedError(new S3HttpError(resp.status, "", (resp.statusText ?? "").trim()));
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
        release: () => task.cancel(), // wire abort on any pump exit; no-op when done
        mapError: (e) => markedError(e), // keep TIMEOUT classification mid-body
    };
}

// ---------------------------------------------------------------------------
// test / connect / instance lifecycle
// ---------------------------------------------------------------------------

/** Resolve the credentials for a test call: explicit values win; on edit a
 *  blank secret falls back to the stored one. */
function testCredentials(args: S3TestArgs): { conf: InstanceConfig; fresh: boolean } {
    const endpoint = args.endpoint.trim();
    const bucket = args.bucket.trim();
    const accessKeyId = args.accessKeyId.trim();
    const secretAccessKey = args.secretAccessKey ?? "";
    if (secretAccessKey !== "" || args.storageId == null) {
        return {
            conf: {
                alias: "",
                endpoint,
                region: (args.region ?? "").trim() || DEFAULT_REGION,
                bucket,
                accessKeyId,
                secretId: null,
                secretAccessKey: secretAccessKey === "" ? null : secretAccessKey,
            },
            fresh: true,
        };
    }
    const conf = configOf(args.storageId);
    return {
        conf: {
            ...conf,
            endpoint: endpoint !== "" ? endpoint : conf.endpoint,
            bucket: bucket !== "" ? bucket : conf.bucket,
            accessKeyId: accessKeyId !== "" ? accessKeyId : conf.accessKeyId,
        },
        fresh: false,
    };
}

async function testImpl(args: S3TestArgs): Promise<{ result: TestOutcome }> {
    try {
        const { conf } = testCredentials(args);
        // One-key list validates endpoint + bucket + keys + signature + clock
        // skew in a single round trip.
        await s3GetText(conf, null, [
            ["list-type", "2"],
            ["max-keys", "1"],
        ]);
        return { result: "SUCCESS" };
    } catch (e) {
        if (e instanceof S3HttpError && (e.status === 401 || e.status === 403)) {
            return { result: "UNAUTHORIZED" };
        }
        const msg = String((e as any)?.message ?? e);
        if (isTimeoutMessage(msg)) return { result: "TIMEOUT" };
        return { result: "OTHER_ERROR" };
    }
}

/** Bucket naming: 2–63 chars from the S3 charset (lowercase-strict AWS rules
 *  relaxed to allow uppercase for permissive MinIO setups — the server
 *  rejects what it doesn't like). */
const BUCKET_RE = /^[A-Za-z0-9][A-Za-z0-9._-]{0,61}[A-Za-z0-9]$/;

/**
 * Create or update an S3 instance. On create (no `storageId`), persist the
 * config + secret key, mint `s3:<uuid>`, and register the host storage row
 * (`context.createStorage` — the host pops the create form). On update,
 * rewrite the kv config; a blank `secretAccessKey` keeps the stored secret.
 */
function connectImpl(args: S3ConnectArgs): { storageId: string; created: boolean } {
    const endpoint = args.endpoint.trim();
    const region = (args.region ?? "").trim() || DEFAULT_REGION;
    const bucket = args.bucket.trim();
    const alias = (args.alias ?? "").trim() || "S3";
    const accessKeyId = args.accessKeyId.trim();
    const secretAccessKey = args.secretAccessKey ?? "";

    // normalizeEndpoint throws on garbage endpoints — validate before
    // persisting anything.
    normalizeEndpoint(endpoint);
    if (!BUCKET_RE.test(bucket)) {
        throw new Error("s3: invalid bucket name (2-63 chars, letters/digits/./-/_)");
    }
    if (accessKeyId === "") {
        throw new Error("s3: access key id cannot be empty");
    }

    if (args.storageId != null) {
        // Update: keep the existing secret unless a new one is given.
        const conf = configOf(args.storageId);
        let secretId = conf.secretId;
        if (secretAccessKey !== "") {
            const newId = secret.put(secretAccessKey);
            if (secretId != null) secret.remove(secretId);
            secretId = newId;
        }
        const updated: InstanceConfig = {
            alias,
            endpoint,
            region,
            bucket,
            accessKeyId,
            secretId,
            secretAccessKey: secretAccessKey !== "" ? secretAccessKey : null,
        };
        db.singleSet(kvKey(args.storageId), JSON.stringify(configToJson(updated)));
        instances.set(args.storageId, updated);
        context.notifyChange();
        return { storageId: args.storageId, created: false };
    }

    // Create: the secret key is required.
    if (secretAccessKey === "") {
        throw new Error("s3: secret access key cannot be empty");
    }
    const instance = `s3:${uuidv4()}`;
    const secretId = secret.put(secretAccessKey);
    const conf: InstanceConfig = {
        alias,
        endpoint,
        region,
        bucket,
        accessKeyId,
        secretId,
        secretAccessKey,
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
        endpoint: conf.endpoint,
        region: conf.region,
        bucket: conf.bucket,
        accessKeyId: conf.accessKeyId,
        secretId: conf.secretId,
    };
}

/** Remove an instance: drop its config (kv) + secret key, ask the host to
 *  delete the storage row, and reload the dashboard. Called from the host
 *  trash button (`storage:removeInstance` via `storage_plugin.remove_instance`). */
function removeInstance(args: EaseRpcOpArg<typeof StorageRemoveInstanceSig>): void {
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
    hostRpc.registerHandler(StorageListSig, (args) =>
        listImpl(configOf(args.storageId), args.dir).catch((e: any) => {
            throw markedError(e);
        }),
    );

    hostRpc.registerStream(StorageGetSig, (args) =>
        openGet(configOf(args.storageId), args.path, args.offset).catch((e: any) => {
            throw markedError(e);
        }),
    );

    viewRpc.registerHandler(S3TestSig, (args) => testImpl(args));

    viewRpc.registerHandler(S3ConnectSig, (args) => connectImpl(args));

    hostRpc.registerHandler(StorageRemoveInstanceSig, (args) => {
        removeInstance(args);
    });
}
