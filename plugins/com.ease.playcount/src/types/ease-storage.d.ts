/**
 * Ambient type declaration for the synthetic `"ease:storage"` host module.
 *
 * Runtime is registered by the Rust plugin runtime
 * (`plugin_runtime::storage_bridge::build_host_fns`) and exposes 15
 * synchronous, SQLite-backed KV primitives. The first parameter is always the
 * plugin id (namespacing); subsequent parameters vary per op.
 *
 * Two key namespaces:
 *   - `single*`  — overwrite semantics (one value per key)
 *   - `multi*`   — append-only semantics (a sorted list of values per key)
 */

declare module "ease:storage" {
    /** Single-value entry returned by `singleGetMulti`. */
    export interface Entry {
        key: string;
        value: string;
    }

    /** Multi-value entry returned by `multiGetAllMulti`. */
    export interface MultiEntry {
        key: string;
        values: string[];
    }

    /** Per-key count returned by `multiCountMulti`. */
    export interface CountEntry {
        key: string;
        count: number;
    }

    /** Key listing entry returned by `listKeys`. `kind` is the SQLite kind
     *  discriminator (0 = single, 1 = multi). */
    export interface KeyInfo {
        key: string;
        kind: number;
    }

    // ----- single-value (overwrite) -----
    /** Returns the value, or `null` if the key doesn't exist. */
    export function singleGet(pluginId: string, key: string): string | null;
    export function singleGetMulti(pluginId: string, keys: string[]): Entry[];
    export function singleSet(
        pluginId: string,
        key: string,
        value: string,
    ): void;
    export function singleSetMulti(pluginId: string, entries: Entry[]): void;
    export function singleDelete(pluginId: string, key: string): void;
    export function singleDeleteMulti(pluginId: string, keys: string[]): void;

    // ----- multi-value (append-only) -----
    export function multiAppend(
        pluginId: string,
        key: string,
        value: string,
    ): void;
    export function multiAppendMulti(
        pluginId: string,
        entries: MultiEntry[],
    ): void;
    /** Returns all values for one key (in append order). */
    export function multiGetAll(pluginId: string, key: string): string[];
    /** Returns all values for each of `keys`. */
    export function multiGetAllMulti(
        pluginId: string,
        keys: string[],
    ): MultiEntry[];
    export function multiCount(pluginId: string, key: string): number;
    export function multiCountMulti(
        pluginId: string,
        keys: string[],
    ): CountEntry[];
    export function multiDelete(pluginId: string, key: string): void;
    export function multiDeleteMulti(pluginId: string, keys: string[]): void;

    // ----- listing -----
    /** Lists keys under `prefix` (empty prefix = all). */
    export function listKeys(
        pluginId: string,
        prefix?: string,
    ): KeyInfo[];
}
