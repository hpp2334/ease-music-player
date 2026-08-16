// Ambient type declaration for the unified host-provided `"ease"` module.
//
// Registered by the Rust plugin runtime (`plugin_runtime::plugin.rs`) as a
// synthetic tur module with grouped namespace consts:
//
//     import { db, secret, oauth, themes, rpc, context } from "ease";
//     db.singleGet("key");               // identity resolved in Rust
//     secret.put("refresh-token");
//     oauth.start("onedrive", alias);
//     themes.color("primary");
//     rpc.call("onedrive:list", { ... }); // view → its backend handler
//     get(context.storageId$);            // null = create, id = edit
//
// Per-instance identity: the Kotlin host stamps a `PluginId` into each tur
// instance at build time (via `TurAppBuilder::instance_data`). Bridge fns
// read it back via `extract_js_ctx` + `js_ctx.data::<PluginId>()` and pass
// `pid.as_str()` to the SQLite / secret-store layer — identity never crosses
// the JS↔Rust boundary as an argument. Plugins never need (and cannot spoof)
// a pluginId argument.

declare module "ease" {
    import type { Readable } from "tur:std";
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
         * Fire-and-forget OAuth trigger. The host fetches the provider's
         * authorize URL, stashes `(provider, alias)`, and opens the system
         * browser; the `easem://oauth2redirect` callback completes the
         * exchange asynchronously.
         */
        start(provider: string, alias: string | null): void;
    };

    // ---- themes namespace -------------------------------------------------

    export const themes: {
        /**
         * Read the host app's Material 3 color by name. Returns `"#RRGGBBAA"`
         * (RGBA hex) or `""` if the host hasn't pushed a value yet.
         */
        color(name: string): string;
        /** Reports the resolved dark/light flag from the host theme. */
        isDark(): boolean;
    };

    // ---- rpc namespace ----------------------------------------------------

    export const rpc: {
        /**
         * Invoke handler `op` on this plugin's headless backend (the instance
         * wired via `wireServiceRpc`) with JSON-serializable `args`. Returns
         * a promise of the handler's result (or rejection with its error).
         * The backend's `RpcClient` is reused — no cross-bus relay. Used by
         * view instances to reach plugin-owned domain logic (e.g.
         * `onedrive:removeInstance`).
         */
        call(op: string, args?: unknown): Promise<any>;
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
