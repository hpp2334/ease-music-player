// Play Counts plugin — view module.
//
// Mounted by the tur engine when the user opens the plugin's "main" view.
// The module lifecycle contract requires a `start({ store })` export: the
// engine invokes it after eval, handing the instance-owned store (one per
// instance since tur #207 — there is no `createStore`). Root-tree lifecycle
// is engine-owned, so no cleanup is returned. `start` does two things:
//   1. `mount(rootView)` binds the view tree to that instance store
//      (declarations in the tree materialize into it).
//   2. `store.set(refresh$)` dispatches the initial KV scan + aggregation
//      (`refresh$` is a mutation so it writes through the store ctx).

// TextEncoder/TextDecoder polyfill FIRST — npm deps may rely on them.
import "../../infra/string-polyfill";
import "../../infra/text-polyfill";
import { mount } from "tur:std";
import type { Store } from "tur:std";
import { rootView, refresh$ } from "./play-counts";

export function start({ store }: { store: Store }): void {
    mount(rootView);
    store.set(refresh$);
}
