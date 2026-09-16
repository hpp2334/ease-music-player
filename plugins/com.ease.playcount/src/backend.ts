// Play Counts plugin — backend module (long-lived).
//
// Loaded by `KeepBackendService` into a headless tur instance stamped with
// `PluginId("com.ease.playcount")`. Subscribes to the `music:play` event on
// the plugin-event bus channel (fire-and-forget, one channel per plugin
// instance); the host (`PluginRepository.bindPlayerEvents`) calls
// `plugin.event { pluginId, type, payload }` for each play and the Rust
// side emits it to this handler via the plugin's RpcClient. The payload
// type is bound on the shared event sig (`plugins/infra/events.ts`).
//
// Data model (KV multi-value, append-only):
//   key   = "plays:YYYY-MM-DD"
//   value = JSON `{ musicId, title, ts }`
// The view module (`view.ts` → `play-counts.ts`) reads the rows back via
// `db.multiGetAllMulti` and aggregates per musicId.

// TextEncoder/TextDecoder polyfill FIRST — npm deps may rely on them.
import "../../infra/string-polyfill";
import "../../infra/text-polyfill";
import { hostRpc } from "tur:rpc";
import { MusicPlaySig } from "../../infra/events";
import { db } from "ease";

function pad2(n: number): string {
    return n < 10 ? "0" + n : String(n);
}

/** Local-time day key, matching what the view's range selector expects. */
function dayKey(ts: number): string {
    const d = new Date(ts);
    return `plays:${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`;
}

/** Local-time day key `offset` days before now (0 = today). */
function dayKeyOffset(offset: number): string {
    const d = new Date();
    d.setDate(d.getDate() - offset);
    return `plays:${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`;
}

// ---------------------------------------------------------------------------
// One-time compaction of duplicated play rows (0.0.8).
//
// Host bug fixed in the same era: `PluginRepository.bindPlayerEvents`
// installed a new event forwarder on every activity start, so every
// `music:play` was delivered — and appended — ×N, where N = activity
// starts in the host process's lifetime (measured peaks of 6). The
// duplicates are byte-identical (the same event object serialized N times,
// down to the `ts` millisecond), and two genuine plays can never share one
// millisecond, so "first occurrence wins" is exact.
//
// Runs BEFORE the `onEvent` registration in [start]: an event fired while
// no handler is registered is silently never delivered (fire-and-forget
// bus), so nothing can race the read → rewrite window. Rows are rewritten
// only where a duplicate was actually found; the flag makes it once per
// install (it survives backend reloads).
// ---------------------------------------------------------------------------

const REPAIR_FLAG_KEY = "repair:duplicate-rows";
/** Scan horizon: one key per day that has plays, newest first. Generous
 *  for this schema's lifetime; a key older than the window would simply
 *  stay un-compacted, never lost. */
const REPAIR_SCAN_DAYS = 400;

function compactDuplicateRows(): void {
    if (db.singleGet(REPAIR_FLAG_KEY) !== null) return;
    const keys: string[] = [];
    for (let i = 0; i < REPAIR_SCAN_DAYS; i++) keys.push(dayKeyOffset(i));
    const changedKeys: string[] = [];
    // NOTE: `multiAppendMulti` takes FLAT `{ key, value }` entries — one
    // per appended row (the Rust bridge reads `entry.value`; its d.ts
    // claimed a `{ key, values }` grouping and cost us a data-loss bug
    // the first time around).
    const reAppend: { key: string; value: string }[] = [];
    for (const entry of db.multiGetAllMulti(keys)) {
        const seen = new Set<string>();
        const kept: string[] = [];
        for (const raw of entry.values) {
            if (seen.has(raw)) continue;
            seen.add(raw);
            kept.push(raw);
        }
        // Only days that actually shrank are rewritten — untouched days
        // must NOT be re-appended (they'd duplicate; v2 fixed exactly
        // that bug in the first attempt).
        if (kept.length !== entry.values.length) {
            changedKeys.push(entry.key);
            for (const v of kept) reAppend.push({ key: entry.key, value: v });
        }
    }
    if (changedKeys.length > 0) {
        db.multiDeleteMulti(changedKeys);
        db.multiAppendMulti(reAppend);
    }
    db.singleSet(REPAIR_FLAG_KEY, "done");
}

// The module lifecycle contract: the engine calls `start()` after eval (and
// runs the returned cleanup before the next load / at destroy). The event
// subscription dies with the instance, so no cleanup is needed.
export function start(): void {
    compactDuplicateRows();
    hostRpc.onEvent(MusicPlaySig, (args) => {
        db.multiAppend(
            dayKey(args.ts),
            JSON.stringify({ musicId: args.musicId, title: args.title, ts: args.ts }),
        );
    });
}
