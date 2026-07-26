// Play Counts plugin — entry module.
//
// Mounted by the tur engine when the user opens the plugin's "main" view.
// Top-level side effects here run at module-eval time:
//   1. `render(rootView)` mounts the view tree into the engine.
//   2. `refresh()` does the initial KV scan + aggregation.

import { render } from "tur:std";
import { rootView, refresh } from "./play-counts";

render(rootView);
refresh();
