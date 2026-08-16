// Play Counts plugin — view module.
//
// Mounted by the tur engine when the user opens the plugin's "main" view.
// The module lifecycle contract requires a `start()` export: the engine
// invokes it after eval (root-tree lifecycle is engine-owned, so no
// cleanup is returned). `start` does two things:
//   1. `mount(rootView)` mounts the view tree into the engine.
//   2. `refresh()` does the initial KV scan + aggregation.

import { mount } from "tur:std";
import { rootView, refresh } from "./play-counts";

export function start(): void {
    mount(rootView);
    refresh();
}
