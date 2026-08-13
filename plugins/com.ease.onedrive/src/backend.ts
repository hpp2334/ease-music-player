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
// Host handlers (under the `onedrive:` prefix):
//   - onedrive:list           { instance, dir }                  -> Entry[]
//   - onedrive:get            { instance, streamId, path, offset } -> { totalLength?, name?, contentType? }
//   - onedrive:oauth.url      {}                                 -> { url }
//   - onedrive:oauth.exchange { code, alias }                    -> { instance }
//   - onedrive:removeInstance { instance }                       -> {}
//
// Ported from the Rust implementation at
// `rust-libs/ease-remote-storage/src/impls/onedrive.rs`.

import { request, requestStream } from "tur:net";
import { registerHandler, pushChunk, endStream, errorStream } from "tur:rpc";
import { db, secret } from "ease";

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
        responseType: "text",
    });
    if (!resp.ok) {
        throw new HttpError(resp.status, `token ${grantType} failed: ${resp.status} ${resp.statusText}`);
    }
    const j = JSON.parse(resp.bodyText ?? "{}");
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
            responseType: "text",
        });
        if (!resp.ok) {
            throw new HttpError(resp.status, `list: ${resp.status} ${resp.statusText}`);
        }
        const j = JSON.parse(resp.bodyText ?? "{}");
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

async function getImpl(
    token: string,
    streamId: number,
    path: string,
    offset: number,
): Promise<{ totalLength?: number; name?: string; contentType?: string }> {
    const url = `${ONEDRIVE_ROOT_API}/root:${path}:/content`;
    const headers = authHeaders(token);
    headers["Range"] = `bytes=${offset}-`;

    const resp = await requestStream({
        url,
        method: "GET",
        headers,
    });
    if (!resp.ok) {
        errorStream(streamId, `get: ${resp.status} ${resp.statusText}`);
        return {};
    }
    const totalLength = parseTotalLength(resp.headers);
    const contentType = resp.headers["Content-Type"] ?? resp.headers["content-type"];
    const name = path.split("/").pop() || path;

    // Pump the body asynchronously: the host returns from open_stream as soon
    // as this metadata lands, then drains the stream as chunks arrive.
    (async () => {
        try {
            for await (const chunk of resp.body) {
                pushChunk(streamId, chunk);
            }
            endStream(streamId);
        } catch (e: any) {
            errorStream(streamId, String(e?.message ?? e));
        }
    })();

    return { totalLength, name, contentType };
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

/** RFC 4122 v4 UUID (crypto.getRandomValues may be absent in boa; Math.random
 *  is plenty for non-adversarial instance-id minting). */
function uuid(): string {
    const hex = "0123456789abcdef";
    let out = "";
    for (let i = 0; i < 36; i++) {
        if (i === 8 || i === 13 || i === 18 || i === 23) {
            out += "-";
        } else if (i === 14) {
            out += "4";
        } else if (i === 19) {
            out += hex[(Math.random() * 4) | 0 | 8];
        } else {
            out += hex[(Math.random() * 16) | 0];
        }
    }
    return out;
}

/**
 * Exchange an authorization code for tokens, mint a new `onedrive:<uuid>`
 * instance, persist its config + refresh token, and return the instance id.
 * `args.alias` is the user-facing display name.
 */
async function exchangeCode(args: { code: string; alias?: string }): Promise<{ instance: string }> {
    const t = await redeemToken("authorization_code", { code: args.code });
    const instance = `onedrive:${uuid()}`;
    const secretId = secret.put(t.refresh_token);
    const alias = args.alias && args.alias.length > 0 ? args.alias : "OneDrive";
    db.singleSet(
        kvKey(instance),
        JSON.stringify({ alias, secretId }),
    );
    // Prime the in-memory state so the first list/get skips a config reload.
    instances.set(instance, { alias, secretId, accessToken: t.access_token });
    return { instance };
}

/** Remove an instance: drop its config (kv) + its refresh-token secret. */
function removeInstance(args: { instance: string }): void {
    const st = instances.get(args.instance);
    let secretId: number | undefined = st?.secretId;
    if (secretId === undefined) {
        const raw = db.singleGet(kvKey(args.instance));
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
    db.singleDelete(kvKey(args.instance));
    instances.delete(args.instance);
}

// ---------------------------------------------------------------------------
// Register handlers
// ---------------------------------------------------------------------------

registerHandler("onedrive:list", (args) =>
    withRetry(args.instance, (token) => listImpl(token, args.dir)),
);

registerHandler("onedrive:get", (args) => {
    const { instance, streamId, path, offset } = args;
    return withRetry(instance, (token) => getImpl(token, streamId, path, offset)).catch((e: any) => {
        // surface non-401 failures as a stream error
        errorStream(streamId, String(e?.message ?? e));
        return {};
    });
});

registerHandler("onedrive:oauth.url", () => ({ url: authorizeUrl() }));

registerHandler("onedrive:oauth.exchange", (args) => exchangeCode(args));

registerHandler("onedrive:removeInstance", (args) => {
    removeInstance(args);
    return {};
});
