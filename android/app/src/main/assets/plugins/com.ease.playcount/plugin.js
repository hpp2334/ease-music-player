import { Axis, Color, Column, Container, CrossAxisAlignment, Each, MainAxisAlignment, MainAxisSize, PointerInteract, Row, ScrollView, SizedBox, Switch, Text, derive, get, mutate, render, set, source, view } from "tur:std";
import { multiGetAllMulti } from "ease:storage";
var __webpack_exports__ = {};

;// CONCATENATED MODULE: external "tur:std"

;// CONCATENATED MODULE: external "ease:storage"

;// CONCATENATED MODULE: ./src/play-counts.ts
// Play Counts plugin — state, KV aggregation, and tur widget factories.
//
// Data model (KV multi-value, append-only):
//   key   = "plays:YYYY-MM-DD"
//   value = JSON `{ musicId, title, ts }`
//
// Each `music:play` event appends one row (the Kotlin `PluginRepository`
// does the append on the host side). This module reads the rows back via
// `ease:storage.multiGetAllMulti`, aggregates per musicId in JS, and renders
// a sorted list with a time-range selector.


const PLUGIN_ID = "com.ease.playcount";
// ---------------------------------------------------------------------------
// Statics
// ---------------------------------------------------------------------------
const RANGES = [
    {
        id: "today",
        label: "Today",
        days: 1,
        offset: 0
    },
    {
        id: "yesterday",
        label: "Yesterday",
        days: 1,
        offset: 1
    },
    {
        id: "3days",
        label: "Last 3 Days",
        days: 3,
        offset: 0
    },
    {
        id: "week",
        label: "Last Week",
        days: 7,
        offset: 0
    },
    {
        id: "month",
        label: "Last Month",
        days: 30,
        offset: 0
    },
    {
        id: "year",
        label: "Last Year",
        days: 365,
        offset: 0
    }
];
// ---------------------------------------------------------------------------
// Reactive state
// ---------------------------------------------------------------------------
const selectedRange$ = source(0);
const entries$ = source([]);
const loading$ = source(false);
const status$ = derive(()=>{
    if (get(loading$)) return "loading";
    return get(entries$).length === 0 ? "empty" : "ready";
});
// ---------------------------------------------------------------------------
// Date helpers (UTC, so the day-key matches what the Kotlin appender writes)
// ---------------------------------------------------------------------------
function pad2(n) {
    return n < 10 ? "0" + n : String(n);
}
function isoDate(d) {
    return d.getUTCFullYear() + "-" + pad2(d.getUTCMonth() + 1) + "-" + pad2(d.getUTCDate());
}
function dateKey(offsetDays) {
    const d = new Date();
    d.setUTCDate(d.getUTCDate() - offsetDays);
    return "plays:" + isoDate(d);
}
function dateKeysForRange(range) {
    if (range.id === "yesterday") return [
        dateKey(1)
    ];
    const keys = [];
    for(let i = 0; i < range.days; i++)keys.push(dateKey(i));
    return keys;
}
function refresh() {
    set(loading$, true);
    try {
        const range = RANGES[get(selectedRange$)];
        const keys = dateKeysForRange(range);
        const grouped = multiGetAllMulti(PLUGIN_ID, keys);
        const counts = new Map();
        for (const entry of grouped){
            for (const raw of entry.values){
                let ev;
                try {
                    ev = JSON.parse(raw);
                } catch  {
                    continue;
                }
                const id = String(ev.musicId ?? "");
                if (!id) continue;
                const title = typeof ev.title === "string" ? ev.title : "(unknown)";
                const prev = counts.get(id);
                if (prev) {
                    prev.count += 1;
                } else {
                    counts.set(id, {
                        musicId: id,
                        title,
                        count: 1
                    });
                }
            }
        }
        const sorted = Array.from(counts.values()).sort((a, b)=>b.count - a.count);
        set(entries$, sorted);
    } catch  {
        set(entries$, []);
    } finally{
        set(loading$, false);
    }
}
function selectRange(i) {
    if (i === get(selectedRange$)) return;
    set(selectedRange$, i);
    refresh();
}
// ---------------------------------------------------------------------------
// Palette
// ---------------------------------------------------------------------------
const COLOR_PRIMARY = Color.rgb(0x12, 0x59, 0x66);
const COLOR_PRIMARY_LIGHT = Color.rgb(0xee, 0xee, 0xee);
const COLOR_TEXT = Color.rgb(0x33, 0x33, 0x33);
const COLOR_TEXT_MUTED = Color.rgb(0x99, 0x99, 0x99);
const COLOR_WHITE = Color.rgb(0xff, 0xff, 0xff);
function RangeChip(props) {
    const selected = props.index === get(selectedRange$);
    return PointerInteract({
        onPointerDown: mutate(()=>selectRange(props.index)),
        child: Container({
            color: selected ? COLOR_PRIMARY : COLOR_PRIMARY_LIGHT,
            padding: 8,
            borderRadius: 16,
            children: [
                Text({
                    text: props.label,
                    fontSize: 13,
                    color: selected ? COLOR_WHITE : COLOR_TEXT_MUTED
                })
            ]
        })
    });
}
function RangeRow() {
    // Plain Row — 6 chips × ~70px ≈ 460px, well within the 1440px viewport.
    // (Previously wrapped in a horizontal `ScrollView`, which leaked an
    // unbounded cross-axis height up to the parent `Column` and starved
    // every sibling below it for vertical space.)
    const children = RANGES.flatMap((r, i)=>[
            RangeChip({
                index: i,
                label: r.label
            }),
            SizedBox({
                width: 8
            })
        ]);
    // Drop the trailing spacer.
    if (children.length > 0) children.pop();
    return Row({
        mainAlignment: MainAxisAlignment.Start,
        crossAlignment: CrossAxisAlignment.Center,
        mainAxisSize: MainAxisSize.Min,
        children
    });
}
function EntryItem(entry) {
    return Container({
        padding: 12,
        children: [
            Row({
                mainAlignment: MainAxisAlignment.SpaceBetween,
                crossAlignment: CrossAxisAlignment.Center,
                children: [
                    Row({
                        mainAlignment: MainAxisAlignment.Start,
                        crossAlignment: CrossAxisAlignment.Center,
                        mainAxisSize: MainAxisSize.Min,
                        children: [
                            Text({
                                text: "♪",
                                fontSize: 16,
                                color: COLOR_PRIMARY
                            }),
                            SizedBox({
                                width: 8
                            }),
                            Text({
                                text: entry.title,
                                fontSize: 15,
                                color: COLOR_TEXT
                            })
                        ]
                    }),
                    Text({
                        text: String(entry.count),
                        fontSize: 16,
                        color: COLOR_PRIMARY
                    })
                ]
            })
        ]
    });
}
function LoadingBody() {
    return Container({
        padding: 48,
        children: [
            Text({
                text: "Loading…",
                fontSize: 14,
                color: COLOR_TEXT_MUTED
            })
        ]
    });
}
function EmptyBody() {
    return Container({
        padding: 48,
        children: [
            Text({
                text: "No plays in this range.",
                fontSize: 14,
                color: COLOR_TEXT_MUTED
            })
        ]
    });
}
function ReadyBody() {
    // `Each` is itself a flex; lay it out as a stretch column so each
    // `EntryItem` spans the full width.
    return ScrollView({
        axis: Axis.Vertical,
        child: Each({
            items: entries$,
            build: (entry)=>EntryItem(entry),
            crossAlignment: CrossAxisAlignment.Stretch
        })
    });
}
// ---------------------------------------------------------------------------
// Root view
// ---------------------------------------------------------------------------
const rootView = view(()=>Column({
        crossAlignment: CrossAxisAlignment.Stretch,
        children: [
            SizedBox({
                height: 16
            }),
            Container({
                padding: 16,
                children: [
                    RangeRow()
                ]
            }),
            Switch({
                value: status$,
                cases: [
                    {
                        key: "loading",
                        child: LoadingBody
                    },
                    {
                        key: "empty",
                        child: EmptyBody
                    },
                    {
                        key: "ready",
                        child: ReadyBody
                    }
                ]
            })
        ]
    }));


;// CONCATENATED MODULE: ./src/index.ts
// Play Counts plugin — entry module.
//
// Mounted by the tur engine when the user opens the plugin's "main" view.
// Top-level side effects here run at module-eval time:
//   1. `render(rootView)` mounts the view tree into the engine.
//   2. `refresh()` does the initial KV scan + aggregation.


render(rootView);
refresh();


//# sourceMappingURL=plugin.js.map