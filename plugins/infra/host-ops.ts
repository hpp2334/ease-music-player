// Host contract op sigs — the typed catalog of ops the Rust host invokes on
// plugin backends (`hostRpc.registerHandler` / `hostRpc.registerStream`).
// Each sig binds the wire op string to its args and result shapes, so
// providers register without hand-declaring arg types; extract a shape
// elsewhere via `EaseRpcOpArg<typeof StorageListSig>` /
// `EaseRpcOpResult<typeof StorageListSig>`.
//
// SOURCE OF TRUTH: the shapes mirror the Rust call sites —
// - storage contract: `rust-libs/ease-js-storage/src/lib.rs` (args, entry
//   decode, stream meta decode),
// - OAuth flow: `rust-libs/ease-client-backend/src/bridge/dispatch.rs`
//   (`oauth.url` / `oauth.exchange` arms),
// - lyric parsing: `rust-libs/ease-client-backend/src/services/lyrics/mod.rs`
// — update both sides together. Ops are contract literals, identical names
// for every provider; identity (`pluginId`, `storageId`, `oauthId`) rides
// the payload, typed `string` (the host routes it — a provider-side literal
// guard would only restate the manifest id).
//
// This module is plain bundled TS (imported relatively by plugins, like the
// polyfills): the runtime value of a sig is just `{ op }`, so no host
// provisioning is involved.

import type { EaseRpcOp } from "tur:rpc";

// ---------------------------------------------------------------------------
// Storage contract shapes
// ---------------------------------------------------------------------------

/** One `storage:list` result entry (camelCase `isDir`, matching the host's
 *  `JsEntry` decode in ease-js-storage). */
export interface StorageEntry {
    name: string;
    path: string;
    size?: number;
    isDir: boolean;
}

/** `storage:get` stream meta — replied up front, before the body flows
 *  (decoded by the host's `GetMeta`). */
export interface StorageGetMeta {
    /** Full file size (Content-Range total when the server honors Range). */
    totalLength?: number;
    /** File name hint for the host (defaults to the path's last segment). */
    name?: string;
    contentType?: string;
    /** Byte offset the pushed chunks actually start at — a server that
     *  ignored the Range request reports `0` and the host drops the prefix.
     *  Defaults to the requested offset. */
    dataOffset?: number;
}

// ---------------------------------------------------------------------------
// Lyric contract shapes (host ↔ plugin; canonical copies — the lyricformats
// plugin's parsers re-use these instead of redeclaring them)
// ---------------------------------------------------------------------------

export interface LyricLine {
    timeMs: number;
    durationMs?: number;
    text: string;
}

export interface LyricMetadata {
    artist?: string;
    album?: string;
    title?: string;
    lyricist?: string;
    author?: string;
    length?: string;
    offset?: string;
}

export interface LyricParseResult {
    lines: LyricLine[];
    metadata?: LyricMetadata;
}

// ---------------------------------------------------------------------------
// Host op sigs
// ---------------------------------------------------------------------------

/** `storage:list` — directory listing. */
export const StorageListSig: EaseRpcOp<
    "storage:list",
    { pluginId: string; storageId: string; dir: string },
    StorageEntry[]
> = { op: "storage:list" };

/** `storage:get` — streaming byte-range download; the Result slot is the
 *  stream meta shape (see `registerStream`). */
export const StorageGetSig: EaseRpcOp<
    "storage:get",
    { pluginId: string; storageId: string; path: string; offset: number },
    StorageGetMeta
> = { op: "storage:get" };

/** `storage:removeInstance` — drop this instance's config + secrets; the
 *  host ignores the reply. */
export const StorageRemoveInstanceSig: EaseRpcOp<
    "storage:removeInstance",
    { pluginId: string; storageId: string },
    void
> = { op: "storage:removeInstance" };

/** `oauth:url` — the authorize URL for the host to open in the browser. */
export const OauthUrlSig: EaseRpcOp<
    "oauth:url",
    { pluginId: string; oauthId: string },
    { url: string }
> = { op: "oauth:url" };

/** `oauth:exchange` — redeem the browser callback's code; the host reads the
 *  minted `storageId` back to (re)bind the storage row. */
export const OauthExchangeSig: EaseRpcOp<
    "oauth:exchange",
    { pluginId: string; oauthId: string; code: string },
    { storageId: string }
> = { op: "oauth:exchange" };

/** `lyric:parse` — parse candidate lyric bytes; `null` means "not mine /
 *  unrecognized content" and the host falls through to the next parser. */
export const LyricParseSig: EaseRpcOp<
    "lyric:parse",
    {
        pluginId: string;
        parserId: string;
        fileName: string;
        size: number;
        contentBase64: string;
    },
    LyricParseResult | null
> = { op: "lyric:parse" };
