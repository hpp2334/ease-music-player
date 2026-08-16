// Play Counts plugin — backend module (long-lived).
//
// Loaded by `KeepBackendService` into a headless tur instance stamped with
// `PluginId("com.ease.playcount")`. Subscribes to the `music:play` event on
// the plugin-event bus channel (fire-and-forget, one channel per plugin
// instance); the host (`PluginRepository.bindPlayerEvents`) calls
// `plugin.event { pluginId, type, payload }` for each play and the Rust
// side emits it to this handler via the plugin's RpcClient.
//
// Data model (KV multi-value, append-only):
//   key   = "plays:YYYY-MM-DD"
//   value = JSON `{ musicId, title, ts }`
// The view module (`view.ts` → `play-counts.ts`) reads the rows back via
// `db.multiGetAllMulti` and aggregates per musicId.

import { onEvent } from "tur:rpc";
import { db } from "ease";

interface MusicPlayPayload {
    musicId: number;
    title: string;
    ts: number;
}

function pad2(n: number): string {
    return n < 10 ? "0" + n : String(n);
}

/** Local-time day key, matching what the view's range selector expects. */
function dayKey(ts: number): string {
    const d = new Date(ts);
    return `plays:${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`;
}

// The module lifecycle contract: the engine calls `start()` after eval (and
// runs the returned cleanup before the next load / at destroy). The event
// subscription dies with the instance, so no cleanup is needed.
export function start(): void {
    onEvent("music:play", (args: MusicPlayPayload) => {
        db.multiAppend(
            dayKey(args.ts),
            JSON.stringify({ musicId: args.musicId, title: args.title, ts: args.ts }),
        );
    });
}
