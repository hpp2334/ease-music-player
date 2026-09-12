// Ambient type declaration for the unified host-provided `"ease"` module.
//
// Registered by the Rust plugin runtime (`plugin_runtime::plugin.rs`) as a
// synthetic tur module with grouped namespace consts:
//
//     import { db, secret, oauth, themes, rpc, context, library } from "ease";
//     db.singleGet("key");               // identity resolved in Rust
//     secret.put("refresh-token");
//     const oauthId = oauth.new();
//     oauth.start(oauthId);              // fires the host OAuth flow
//     themes.color("primary");           // throws on unknown names
//     rpc.call(WebdavTestSig, { ... });  // view → its backend handler (op sig)
//     store.get(context.storageId$);     // null = create, id = edit
//     library.playlists();               // read-only host playlist roster
//
// Per-instance identity: the Kotlin host stamps a `PluginId` into each tur
// instance at build time (via `TurAppBuilder::instance_data`). Bridge fns
// read it back via `extract_js_ctx` + `js_ctx.data::<PluginId>()` and pass
// `pid.as_str()` to the SQLite / secret-store layer — identity never crosses
// the JS↔Rust boundary as an argument. Plugins never need (and cannot spoof)
// a pluginId argument.

declare module "ease" {
    import type { Readable } from "tur:std";
    import type { EaseRpcOp, EaseRpcOpArg, EaseRpcOpResult } from "tur:rpc";
    // ---- db entry types ----------------------------------------------------

    /** Single-value entry returned by `singleGetMulti`. */
    export interface StorageEntry {
        key: string;
        value: string;
    }
    /** Multi-value entry returned by `multiGetAllMulti`. */
    export interface StorageMultiEntry {
        key: string;
        values: string[];
    }
    /** Per-key count returned by `multiCountMulti`. */
    export interface StorageCountEntry {
        key: string;
        count: number;
    }
    /** Key listing entry returned by `listKeys`. `kind`: 0 = single, 1 = multi. */
    export interface StorageKeyInfo {
        key: string;
        kind: number;
    }

    // ---- db namespace -----------------------------------------------------

    export const db: {
        // ----- single-value (overwrite) -----
        /** Returns the value, or `null` if the key doesn't exist. */
        singleGet(key: string): string | null;
        singleGetMulti(keys: string[]): StorageEntry[];
        singleSet(key: string, value: string): void;
        singleSetMulti(entries: StorageEntry[]): void;
        singleDelete(key: string): void;
        singleDeleteMulti(keys: string[]): void;

        // ----- multi-value (append-only) -----
        multiAppend(key: string, value: string): void;
        multiAppendMulti(entries: StorageMultiEntry[]): void;
        /** Returns all values for one key (in append order). */
        multiGetAll(key: string): string[];
        /** Returns all values for each of `keys`. */
        multiGetAllMulti(keys: string[]): StorageMultiEntry[];
        multiCount(key: string): number;
        multiCountMulti(keys: string[]): StorageCountEntry[];
        multiDelete(key: string): void;
        multiDeleteMulti(keys: string[]): void;

        // ----- listing -----
        /** Lists keys under `prefix` (empty/undefined prefix = all). */
        listKeys(prefix?: string): StorageKeyInfo[];
    };

    // ---- secret namespace -------------------------------------------------

    export const secret: {
        /**
         * Returns the secret's value, or `null` if it doesn't exist OR isn't
         * owned by the calling plugin (no existence leak).
         */
        get(secretId: number): string | null;
        /** Stores a new secret owned by the calling plugin; returns its id. */
        put(secret: string): number;
        /**
         * No-op if the secret doesn't exist or isn't owned by the calling
         * plugin.
         */
        remove(secretId: number): void;
    };

    // ---- oauth namespace --------------------------------------------------

    export const oauth: {
        /**
         * Mint a fresh, opaque OAuth flow id. The host keeps no state for
         * it — key your pending flow data (alias, …) by this id in your own
         * KV and consume it when your `oauth:exchange` handler runs.
         * Enables concurrent flows. (`"new"` is quoted only because `new`
         * is a reserved word in type-literal position.)
         */
        "new"(): string;
        /**
         * Fire-and-forget OAuth trigger for the flow `oauthId` (from
         * `new()`). The host fetches the authorize URL from your backend
         * (`oauth:url { pluginId, oauthId }`), stashes the pair, and opens
         * the system browser; the `easem://oauth2redirect` callback
         * completes the exchange asynchronously (`oauth:exchange
         * { pluginId, oauthId, code }`). Your plugin's identity comes from
         * the instance — never an argument.
         */
        start(oauthId: string): void;
    };

    // ---- themes namespace -------------------------------------------------

    export const themes: {
        /**
         * Read the host app's Material 3 color by name. Returns `"#RRGGBBAA"`
         * (RGBA hex). **Throws** if `name` is not a known theme role — views
         * load long after the host pushes its theme, so a miss is a bug
         * (typo), not a startup race. Known names (pushed by
         * `EaseMusicPlayerTheme`): `primary`, `onPrimary`,
         * `primaryContainer`, `onPrimaryContainer`, `secondary`,
         * `onSecondary`, `secondaryContainer`, `onSecondaryContainer`,
         * `tertiary`, `onTertiary`, `background`, `onBackground`, `surface`,
         * `onSurface`, `surfaceVariant`, `onSurfaceVariant`,
         * `surfaceContainer`, `outline`, `outlineVariant`, `error`,
         * `onError`.
         */
        color(name: string): string;
        /** Reports the resolved dark/light flag from the host theme. */
        isDark(): boolean;
    };

    // ---- rpc namespace ----------------------------------------------------

    export const rpc: {
        /**
         * Invoke the op bound on `sig` on this plugin's headless backend
         * (the instance wired via `wireServiceRpc`) with JSON-serializable
         * args, and await its result. Returns a promise of the handler's
         * result typed by the sig's bound Result (or rejection with its
         * error). The backend's `RpcClient` is reused — no cross-bus relay.
         *
         * Calls land in the **view scope**: they resolve handlers the backend
         * registered via `viewRpc.registerHandler`; host-side ops
         * (`hostRpc.registerHandler` / `registerStream`) are out of reach.
         * Plugin-private sigs are declared once in the plugin and imported by
         * both its view and backend modules.
         */
        call<S extends EaseRpcOp<string, any, any>>(
            sig: S,
            args: EaseRpcOpArg<S>,
        ): Promise<EaseRpcOpResult<S>>;
    };

    // ---- library namespace -----------------------------------------------

    /** Playlist summary returned by [`library.playlists`]. */
    export interface PlaylistInfo {
        /**
         * Stringified playlist row id — the same id form the app uses for
         * playlists. Stable for the playlist's lifetime.
         */
        id: string;
        title: string;
        /**
         * Stringified music ids of the playlist's members (unordered) —
         * the same id form `music:play` event payloads carry, so members
         * match stored play rows with plain string comparison.
         */
        musicIds: string[];
    }

    export const library: {
        /**
         * Snapshot of the host's playlists in the app's playlist order:
         * `[{ id, title, musicIds }]`. Read-only host library info —
         * synchronous (blocks the engine thread ~ms for the two SQLite
         * reads). Membership is the host's CURRENT state: a snapshot taken
         * at view load does not track later playlist edits.
         */
        playlists(): PlaylistInfo[];
    };

    // ---- context namespace ------------------------------------------------

    export const context: {
        /**
         * The storage id this view instance represents: `null` for a
         * create-mode setup view, or the storage's `plugin_storage_id`
         * (e.g. `"onedrive:abc123"`) for an edit view. Read via `get(...)`.
         * Seeded once per instance; never changes for the instance lifetime.
         */
        readonly storageId$: Readable<string | null>;
        /**
         * Ask the host to reload its storage list so kv-side changes (an
         * alias rename) or a removal propagate to the dashboard + edit page.
         */
        notifyChange(): void;
        /**
         * Find-or-create the host storage row for
         * `(pluginId, pluginStorageId)` and notify the host so a create-mode
         * form can pop. Called by a plugin backend after persisting a new
         * instance's config + secret — the non-OAuth counterpart of the
         * OAuth exchange flow (e.g. the WebDAV plugin's `webdav:connect`).
         * Asynchronous (fire-and-forget): returns immediately; the host row
         * is created + the host notified on a background task.
         */
        createStorage(pluginStorageId: string): void;
        /**
         * Delete the host storage row for `(pluginId, pluginStorageId)`.
         * Called by a plugin backend after it wipes its own kv + secret,
         * completing the disconnect. No-op if no row matches.
         */
        removeStorage(pluginStorageId: string): void;
    };
}
