// OneDrive storage provider — a headless JS plugin that serves `list` and
// `get` (streaming byte-range download) over the `tur:rpc` channel, plus
// OAuth add/remove instance ops. The host `JsStorageBackend` treats OneDrive
// like any other `StorageBackend`.
//
// Multi-instance: each configured OneDrive account is one *instance* named
// `onedrive:<uuid>` (the storage row's `plugin_storage_id`). Per-instance
// config lives in `ease.db` (this plugin's KV) under
// `storage:<instance>` = JSON `{ alias, secretId }`; the refresh token lives
// in `ease.secret` under that `secretId` (scope `plugin:com.ease.onedrive`).
// Access tokens are cached in module state; on a 401 the refresh token is
// rotated and persisted back to the secret store.
//
// Identity: this headless instance is created by `KeepBackendService` which
// stamps `PluginId("com.ease.onedrive")` into the per-instance data slot.
// `ease.*` bridge fns resolve the calling plugin from that slot — no
// pluginId argument is needed (or accepted) on any call here.
//
// Handlers — contract literals under hostRpc scope: identical op names for
// every storage provider, identity riding the payload (`pluginId` = this
// manifest's id; `storageId` = the `plugin_storage_id` instance). The OAuth
// flow is host-bridged (the view fires `ease.oauth.start`, the host comes
// back through the `oauth.url` / `oauth.exchange` bridges):
//   - storage:list           { pluginId, storageId, dir }          -> Entry[]
//   - storage:get            { pluginId, storageId, path, offset } -> registerStream: meta
//     { totalLength?, name?, contentType? } + credit-gated body
//   - oauth:url              { pluginId, oauthId }                 -> { url }
//   - oauth:exchange         { pluginId, oauthId, code }           -> { storageId }
//   - storage:removeInstance { pluginId, storageId }               -> {}
// The connect-form alias never crosses the host: the view stashes it in
// this plugin's KV under `oauth:<oauthId>` (see `oauth-pending.ts`) and the
// `oauth:exchange` handler consumes it.
//
// Ported from the Rust implementation at
// `rust-libs/ease-remote-storage/src/impls/onedrive.rs`.

// TextEncoder/TextDecoder polyfill FIRST — npm deps below may rely on them.
import "../../infra/string-polyfill";
import "../../infra/text-polyfill";
import { request, requestStream } from "tur:net";
import { decodeUtf8 } from "tur:std";
import { hostRpc } from "tur:rpc";
import type { StreamSource } from "tur:rpc";
import { db, secret, context } from "ease";
import { takePending } from "./oauth-pending";
import { v4 as uuidv4 } from "uuid";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const ONEDRIVE_ROOT_API = "https://graph.microsoft.com/v1.0/me/drive";
const ONEDRIVE_API_BASE = "https://login.microsoftonline.com/common/oauth2/v2.0";
const ONEDRIVE_REDIRECT_URI = "easem://oauth2redirect/";
const CLIENT_ID = "5db0dade-b21c-4161-bd4f-027e0f3e4700";
const SCOPES = "Files.Read offline_access";

// ---------------------------------------------------------------------------
// Per-instance state
// ---------------------------------------------------------------------------

interface InstanceState {
    alias: string;
    secretId: number;
    /** Cached access token; refreshed lazily / on 401. */
    accessToken: string | null;
}

/** instance ("onedrive:<uuid>") -> state. Lazily loaded on first use. */
const instances = new Map<string, InstanceState>();

class HttpError extends Error {
    constructor(public status: number, message: string) {
        super(message);
    }
}

function kvKey(instance: string): string {
    return `storage:${instance}`;
}

function configOf(instance: string): InstanceState {
    const st = instances.get(instance);
    if (st) return st;
    const raw = db.singleGet(kvKey(instance));
    if (raw == null) {
        throw new Error(`onedrive: no config for instance ${instance}`);
    }
    const cfg = JSON.parse(raw);
    const state: InstanceState = {
        alias: cfg.alias ?? instance,
        secretId: cfg.secretId,
        accessToken: null,
    };
    instances.set(instance, state);
    return state;
}

/** Read this instance's current refresh token from the secret store. */
function loadRefreshToken(secretId: number): string {
    const v = secret.get(secretId);
    if (v == null) {
        throw new Error(`onedrive: refresh token missing for secretId ${secretId}`);
    }
    return v;
}

/** Persist a (possibly rotated) refresh token to the secret store. */
function saveRefreshToken(secretId: number, token: string): void {
    secret.remove(secretId);
    // re-put to replace — secret ids are immutable per row; we reuse the id by
    // deleting + re-inserting. (The secret store has no in-place update.)
    const newId = secret.put(token);
    if (newId !== secretId) {
        // The id shifted — update the config so future loads resolve it. This
        // is rare (only if the store hands out a different id after delete);
        // we patch both the kv and the in-memory state.
        instances.forEach((s) => {
            if (s.secretId === secretId) s.secretId = newId;
        });
        patchConfigSecretId(secretId, newId);
    }
}

function patchConfigSecretId(oldId: number, newId: number): void {
    // Find the instance whose config references oldId and rewrite it. We scan
    // the loaded instances (cheap; typically a handful).
    for (const [instance, st] of instances) {
        if (st.secretId === newId) {
            db.singleSet(kvKey(instance), JSON.stringify({ alias: st.alias, secretId: newId }));
        }
    }
}

// ---------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------

function authHeaders(token: string): Record<string, string> {
    return { Authorization: `bearer ${token}` };
}

function isAuthError(e: unknown): boolean {
    return e instanceof HttpError && e.status === 401;
}

function formEncode(fields: Record<string, string>): string {
    return Object.entries(fields)
        .map(([k, v]) => `${k}=${encodeURIComponent(v)}`)
        .join("&");
}

async function redeemToken(
    grantType: "authorization_code" | "refresh_token",
    extra: Record<string, string>,
): Promise<{ access_token: string; refresh_token: string }> {
    const body = formEncode({
        client_id: CLIENT_ID,
        redirect_uri: ONEDRIVE_REDIRECT_URI,
        grant_type: grantType,
        ...extra,
    });
    const resp = await request({
        url: `${ONEDRIVE_API_BASE}/token`,
        method: "POST",
        headers: { "Content-Type": "application/x-www-form-urlencoded" },
        body,
    }).promise;
    if (!resp.ok) {
        throw new HttpError(resp.status, `token ${grantType} failed: ${resp.status} ${resp.statusText}`);
    }
    const j = JSON.parse(decodeUtf8(resp.body));
    return { access_token: j.access_token, refresh_token: j.refresh_token };
}

async function refreshAccess(instance: string): Promise<string> {
    const st = configOf(instance);
    const rt = loadRefreshToken(st.secretId);
    const t = await redeemToken("refresh_token", { refresh_token: rt });
    st.accessToken = t.access_token;
    // Persist the rotated refresh token.
    saveRefreshToken(st.secretId, t.refresh_token);
    return t.access_token;
}

async function ensureToken(instance: string): Promise<string> {
    const st = configOf(instance);
    if (st.accessToken) return st.accessToken;
    return refreshAccess(instance);
}

/** Run `fn` with a fresh access token; on 401 rotate once and retry. */
async function withRetry<T>(instance: string, fn: (token: string) => Promise<T>): Promise<T> {
    const token = await ensureToken(instance);
    try {
        return await fn(token);
    } catch (e) {
        if (isAuthError(e)) {
            const fresh = await refreshAccess(instance);
            return await fn(fresh);
        }
        throw e;
    }
}

// ---------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------

interface Entry {
    name: string;
    path: string;
    size?: number;
    isDir: boolean;
}

function computeListUrl(dir: string): string {
    const sub = dir === "/" ? "/root/children" : `/root:${dir}:/children`;
    return `${ONEDRIVE_ROOT_API}${sub}`;
}

async function listImpl(token: string, dir: string): Promise<Entry[]> {
    let url = computeListUrl(dir);
    const out: Entry[] = [];
    for (;;) {
        const resp = await request({
            url,
            method: "GET",
            headers: authHeaders(token),
        }).promise;
        if (!resp.ok) {
            throw new HttpError(resp.status, `list: ${resp.status} ${resp.statusText}`);
        }
        const j = JSON.parse(decodeUtf8(resp.body));
        const value: any[] = j.value ?? [];
        for (const item of value) {
            const name: string = item.name;
            const path = `${dir}/${name}`;
            if (item.file) {
                out.push({ name, path, size: item.size, isDir: false });
            } else if (item.folder) {
                out.push({ name, path, isDir: true });
            }
        }
        if (typeof j["@odata.nextLink"] === "string") {
            url = j["@odata.nextLink"];
        } else {
            break;
        }
    }
    // dirs first, then by path — matches the Rust ordering.
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
    const cr = headers["Content-Range"] ?? headers["content-range"];
    if (cr) {
        const m = /\/(\d+)\s*$/.exec(cr);
        if (m) return parseInt(m[1], 10);
    }
    const cl = headers["Content-Length"] ?? headers["content-length"];
    if (cl) return parseInt(cl, 10);
    return undefined;
}

/**
 * Streaming get opener: one ranged request per stream. The dispatcher pumps
 * `body` with host-granted credits; `release` cancels the Task on any pump
 * exit (host cancel included). OneDrive honors Range, so no `dataOffset`.
 */
async function openGet(
    token: string,
    path: string,
    offset: number,
): Promise<StreamSource> {
    const url = `${ONEDRIVE_ROOT_API}/root:${path}:/content`;
    const headers = authHeaders(token);
    headers["Range"] = `bytes=${offset}-`;

    const t = requestStream({
        url,
        method: "GET",
        headers,
    });
    const resp = await t.promise;
    if (resp.status >= 400) {
        // Fail the RPC itself so the host's `get` returns Err and withRetry's
        // 401 rotation can kick in — the error rides the reply, not the
        // stream.
        t.cancel();
        throw new HttpError(resp.status, `get: ${resp.status} ${resp.statusText}`);
    }
    const totalLength = parseTotalLength(resp.headers);
    const contentType = resp.headers["Content-Type"] ?? resp.headers["content-type"];
    const name = path.split("/").pop() || path;

    return {
        meta: { totalLength, name, contentType },
        body: resp.body,
        release: () => t.cancel(), // wire abort on any pump exit; no-op when done
    };
}

// ---------------------------------------------------------------------------
// OAuth + instance lifecycle
// ---------------------------------------------------------------------------

function authorizeUrl(): string {
    const q = formEncode({
        client_id: CLIENT_ID,
        response_type: "code",
        redirect_uri: ONEDRIVE_REDIRECT_URI,
        response_mode: "query",
        scope: SCOPES,
    });
    return `${ONEDRIVE_API_BASE}/authorize?${q}`;
}


// Handler args shapes (see the op table in the header comment). `pluginId`
// is this plugin's manifest id, literal-typed — a mismatch would mean the
// host routed somebody else's call here.

interface ListArgs {
    pluginId: "com.ease.onedrive";
    storageId: string;
    dir: string;
}

interface GetArgs {
    pluginId: "com.ease.onedrive";
    storageId: string;
    path: string;
    offset: number;
}

interface OauthUrlArgs {
    pluginId: "com.ease.onedrive";
    oauthId: string;
}

interface ExchangeArgs {
    pluginId: "com.ease.onedrive";
    oauthId: string;
    code: string;
}

interface RemoveInstanceArgs {
    pluginId: "com.ease.onedrive";
    storageId: string;
}

/**
 * Exchange an authorization code for tokens, mint a new `onedrive:<uuid>`
 * storage instance, persist its config + refresh token, and return its id.
 * The user-facing alias comes from this flow's pending slot
 * (`oauth:<oauthId>`, stashed by the view before `ease.oauth.start`) —
 * never from the host.
 */
async function exchangeCode(args: ExchangeArgs): Promise<{ storageId: string }> {
    const t = await redeemToken("authorization_code", { code: args.code });
    const instance = `onedrive:${uuidv4()}`;
    const secretId = secret.put(t.refresh_token);
    const pending = takePending(db, args.oauthId);
    const alias = pending?.alias && pending.alias.length > 0 ? pending.alias : "OneDrive";
    db.singleSet(
        kvKey(instance),
        JSON.stringify({ alias, secretId }),
    );
    // Prime the in-memory state so the first list/get skips a config reload.
    instances.set(instance, { alias, secretId, accessToken: t.access_token });
    return { storageId: instance };
}

/** Remove an instance: drop its config (kv) + its refresh-token secret, ask
 *  the host to delete the storage row, and reload the dashboard. Called only
 *  from the host — the edit view's top-bar trash icon and the storages-page
 *  trash both route through `context.removeStorage` / the host's
 *  `storage_plugin.remove_instance` bridge, which invokes this op
 *  (`storage:removeInstance`, hostRpc scope) before deleting the row. */
function removeInstance(args: RemoveInstanceArgs): void {
    const st = instances.get(args.storageId);
    let secretId: number | undefined = st?.secretId;
    if (secretId === undefined) {
        const raw = db.singleGet(kvKey(args.storageId));
        if (raw != null) {
            try {
                secretId = JSON.parse(raw).secretId;
            } catch {
                /* ignore — config corrupt; still delete the kv row */
            }
        }
    }
    if (secretId !== undefined) {
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
// ---------------------------------------------------------------------------

// The module lifecycle contract: the engine calls `start()` after eval (and
// runs the returned cleanup before the next load / at destroy). Handlers are
// per-instance and die with the instance, so no cleanup is needed.
export function start(): void {
    hostRpc.registerHandler("storage:list", (args: ListArgs) =>
        withRetry(args.storageId, (token) => listImpl(token, args.dir)),
    );

    hostRpc.registerStream("storage:get", (args: GetArgs) =>
        withRetry(args.storageId, (token) => openGet(token, args.path, args.offset)),
    );

    hostRpc.registerHandler("oauth:url", (_args: OauthUrlArgs) => ({ url: authorizeUrl() }));

    hostRpc.registerHandler("oauth:exchange", (args: ExchangeArgs) => exchangeCode(args));

    hostRpc.registerHandler("storage:removeInstance", (args: RemoveInstanceArgs) => {
        removeInstance(args);
        return {};
    });
}
