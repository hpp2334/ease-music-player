import { request, requestStream } from "tur:net";
import { endStream, errorStream, pushChunk, registerHandler } from "tur:rpc";
var __webpack_exports__ = {};

;// CONCATENATED MODULE: external "tur:net"

;// CONCATENATED MODULE: external "tur:rpc"

;// CONCATENATED MODULE: ./src/index.ts
function _define_property(obj, key, value) {
    if (key in obj) {
        Object.defineProperty(obj, key, {
            value: value,
            enumerable: true,
            configurable: true,
            writable: true
        });
    } else {
        obj[key] = value;
    }
    return obj;
}
// OneDrive storage provider — a headless JS plugin that serves `list` and
// `get` (byte-range download) over the `tur:rpc` channel, so the host
// `JsStorageBackend` can treat OneDrive like any other `StorageBackend`.
//
// Ported from the Rust implementation at
// `rust-libs/ease-remote-storage/src/impls/onedrive.rs`. The host calls handlers
// under the `onedrive:` prefix (matching the manifest's storage contribution
// id `onedrive`):
//   - onedrive:list          { dir }                       -> Entry[]
//   - onedrive:get           { streamId, path, offset }    -> { totalLength?, name?, contentType? }
//   - onedrive:configure     { refreshToken }              -> {}      (restore saved credentials)
//   - onedrive:oauth.url     {}                            -> { url }
//   - onedrive:oauth.exchange{ code }                      -> { refreshToken }
//
// Auth uses the OAuth2 authorization-code flow with refresh-token rotation.
// The access token is held in module state; on a 401 the refresh is rotated and
// the call retried once (mirroring the Rust `*_with_retry_impl`).


// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------
const ONEDRIVE_ROOT_API = "https://graph.microsoft.com/v1.0/me/drive";
const ONEDRIVE_API_BASE = "https://login.microsoftonline.com/common/oauth2/v2.0";
const ONEDRIVE_REDIRECT_URI = "easem://oauth2redirect/";
const CLIENT_ID = "5db0dade-b21c-4161-bd4f-027e0f3e4700";
const SCOPES = "Files.Read offline_access";
// ---------------------------------------------------------------------------
// Auth state
// ---------------------------------------------------------------------------
let refreshToken = null;
let accessToken = null;
class HttpError extends Error {
    constructor(status, message){
        super(message), _define_property(this, "status", void 0), this.status = status;
    }
}
function authHeaders() {
    const h = {};
    if (accessToken) {
        h["Authorization"] = `bearer ${accessToken}`;
    }
    return h;
}
function isAuthError(e) {
    return e instanceof HttpError && e.status === 401;
}
function formEncode(fields) {
    return Object.entries(fields).map(([k, v])=>`${k}=${encodeURIComponent(v)}`).join("&");
}
async function redeemToken(grantType, extra) {
    const body = formEncode({
        client_id: CLIENT_ID,
        redirect_uri: ONEDRIVE_REDIRECT_URI,
        grant_type: grantType,
        ...extra
    });
    const resp = await request({
        url: `${ONEDRIVE_API_BASE}/token`,
        method: "POST",
        headers: {
            "Content-Type": "application/x-www-form-urlencoded"
        },
        body,
        responseType: "text"
    });
    if (!resp.ok) {
        throw new HttpError(resp.status, `token ${grantType} failed: ${resp.status} ${resp.statusText}`);
    }
    const j = JSON.parse(resp.bodyText ?? "{}");
    return {
        access_token: j.access_token,
        refresh_token: j.refresh_token
    };
}
async function refreshAccess() {
    if (!refreshToken) {
        throw new Error("onedrive: no refresh token configured");
    }
    const t = await redeemToken("refresh_token", {
        refresh_token: refreshToken
    });
    accessToken = t.access_token;
    refreshToken = t.refresh_token; // rotated — caller should persist
}
async function ensureToken() {
    if (accessToken) return;
    await refreshAccess();
}
function computeListUrl(dir) {
    const sub = dir === "/" ? "/root/children" : `/root:${dir}:/children`;
    return `${ONEDRIVE_ROOT_API}${sub}`;
}
async function listImpl(dir) {
    let url = computeListUrl(dir);
    const out = [];
    for(;;){
        const resp = await request({
            url,
            method: "GET",
            headers: authHeaders(),
            responseType: "text"
        });
        if (!resp.ok) {
            throw new HttpError(resp.status, `list: ${resp.status} ${resp.statusText}`);
        }
        const j = JSON.parse(resp.bodyText ?? "{}");
        const value = j.value ?? [];
        for (const item of value){
            const name = item.name;
            const path = `${dir}/${name}`;
            if (item.file) {
                out.push({
                    name,
                    path,
                    size: item.size,
                    isDir: false
                });
            } else if (item.folder) {
                out.push({
                    name,
                    path,
                    isDir: true
                });
            }
        }
        if (typeof j["@odata.nextLink"] === "string") {
            url = j["@odata.nextLink"];
        } else {
            break;
        }
    }
    // dirs first, then by path — matches the Rust ordering.
    out.sort((a, b)=>{
        if (a.isDir !== b.isDir) return a.isDir ? -1 : 1;
        if (a.path < b.path) return -1;
        if (a.path > b.path) return 1;
        return 0;
    });
    return out;
}
async function listWithRetry(dir) {
    await ensureToken();
    try {
        return await listImpl(dir);
    } catch (e) {
        if (isAuthError(e)) {
            await refreshAccess();
            return await listImpl(dir);
        }
        throw e;
    }
}
// ---------------------------------------------------------------------------
// get (streaming byte-range download)
// ---------------------------------------------------------------------------
function parseTotalLength(headers) {
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
async function getImpl(streamId, path, offset) {
    const url = `${ONEDRIVE_ROOT_API}/root:${path}:/content`;
    const headers = authHeaders();
    headers["Range"] = `bytes=${offset}-`;
    const resp = await requestStream({
        url,
        method: "GET",
        headers
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
    (async ()=>{
        try {
            for await (const chunk of resp.body){
                pushChunk(streamId, chunk);
            }
            endStream(streamId);
        } catch (e) {
            errorStream(streamId, String(e?.message ?? e));
        }
    })();
    return {
        totalLength,
        name,
        contentType
    };
}
async function getWithRetry(args) {
    await ensureToken();
    try {
        return await getImpl(args.streamId, args.path, args.offset);
    } catch (e) {
        if (isAuthError(e)) {
            await refreshAccess();
            return await getImpl(args.streamId, args.path, args.offset);
        }
        // surface as a stream error so the host's StreamFile reports it
        errorStream(args.streamId, String(e?.message ?? e));
        return {};
    }
}
// ---------------------------------------------------------------------------
// OAuth helpers
// ---------------------------------------------------------------------------
function authorizeUrl() {
    const q = formEncode({
        client_id: CLIENT_ID,
        response_type: "code",
        redirect_uri: ONEDRIVE_REDIRECT_URI,
        response_mode: "query",
        scope: SCOPES
    });
    return `${ONEDRIVE_API_BASE}/authorize?${q}`;
}
async function exchangeCode(code) {
    const t = await redeemToken("authorization_code", {
        code
    });
    accessToken = t.access_token;
    refreshToken = t.refresh_token;
    return t.refresh_token;
}
// ---------------------------------------------------------------------------
// Register handlers
// ---------------------------------------------------------------------------
registerHandler("onedrive:list", (args)=>listWithRetry(args.dir));
registerHandler("onedrive:get", (args)=>getWithRetry(args));
registerHandler("onedrive:configure", (args)=>{
    refreshToken = args.refreshToken ?? null;
    accessToken = null;
    return {};
});
registerHandler("onedrive:oauth.url", ()=>({
        url: authorizeUrl()
    }));
registerHandler("onedrive:oauth.exchange", (args)=>exchangeCode(args.code).then((rt)=>({
            refreshToken: rt
        })));


//# sourceMappingURL=plugin.js.map