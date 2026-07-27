import { Axis, Color, Column, Container, CrossAxisAlignment, Each, Expanded, MainAxisAlignment, MainAxisSize, PointerInteract, Row, ScrollView, SizedBox, Switch, Text, derive, get as external_tur_std_get, mutate, render, set, source, view } from "tur:std";
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
    if (external_tur_std_get(loading$)) return "loading";
    return external_tur_std_get(entries$).length === 0 ? "empty" : "ready";
});
// ---------------------------------------------------------------------------
// Date helpers (local time, so the day-key matches what the Kotlin appender
// writes via `LocalDate.now()` — which uses the device default timezone).
// ---------------------------------------------------------------------------
function pad2(n) {
    return n < 10 ? "0" + n : String(n);
}
function isoDate(d) {
    return d.getFullYear() + "-" + pad2(d.getMonth() + 1) + "-" + pad2(d.getDate());
}
function dateKey(offsetDays) {
    const d = new Date();
    d.setDate(d.getDate() - offsetDays);
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
        const range = RANGES[external_tur_std_get(selectedRange$)];
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
                    // Older rows (written before the title field existed)
                    // fall back to "(unknown)"; prefer a real title if any
                    // row for this music provides one.
                    if (prev.title === "(unknown)" && title !== "(unknown)") {
                        prev.title = title;
                    }
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
    if (i === external_tur_std_get(selectedRange$)) return;
    set(selectedRange$, i);
    refresh();
}
// ---------------------------------------------------------------------------
// Palette — aligned with EaseMusicPlayerTheme (Material 3 light scheme):
//   primary         = #2E89B0
//   secondary       = #C9EBFA
//   surfaceVariant  = #E3E3E3
// ---------------------------------------------------------------------------
const COLOR_PRIMARY = Color.hex("#2E89B0");
const COLOR_PRIMARY_SOFT = Color.hex("#C9EBFA");
const COLOR_PAGE_BG = Color.hex("#F8FAFC");
const COLOR_CARD = Color.hex("#FFFFFF");
const COLOR_TEXT = Color.hex("#0F172A");
const COLOR_TEXT_MUTED = Color.hex("#64748B");
const COLOR_DIVIDER = Color.hex("#E2E8F0");
const COLOR_BAR_TRACK = Color.hex("#E2E8F0");
// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------
const PAGE_PADDING = 16;
const CARD_RADIUS = 12;
const CHIP_RADIUS = 999;
const BAR_HEIGHT = 6;
function RangeChip(props) {
    return PointerInteract({
        onPointerDown: mutate(()=>selectRange(props.index)),
        child: Container({
            color: derive(({ get })=>props.index === get(selectedRange$) ? COLOR_PRIMARY : Color.hex("#FFFFFF")),
            borderColor: derive(({ get })=>props.index === get(selectedRange$) ? COLOR_PRIMARY : COLOR_DIVIDER),
            borderWidth: 1,
            borderRadius: CHIP_RADIUS,
            padding: 10,
            children: [
                Text({
                    text: props.label,
                    fontSize: 12,
                    color: derive(({ get })=>props.index === get(selectedRange$) ? Color.hex("#FFFFFF") : COLOR_TEXT_MUTED)
                })
            ]
        })
    });
}
function RangeRow() {
    const children = [];
    RANGES.forEach((r, i)=>{
        if (i > 0) children.push(SizedBox({
            width: 8
        }));
        children.push(RangeChip({
            index: i,
            label: r.label
        }));
    });
    return Row({
        mainAlignment: MainAxisAlignment.Start,
        crossAlignment: CrossAxisAlignment.Center,
        mainAxisSize: MainAxisSize.Min,
        children
    });
}
function Header() {
    return Column({
        crossAlignment: CrossAxisAlignment.Start,
        mainAxisSize: MainAxisSize.Min,
        children: [
            Text({
                text: "Play Counts",
                fontSize: 22,
                color: COLOR_TEXT
            }),
            SizedBox({
                height: 4
            }),
            Text({
                text: derive(()=>{
                    const range = RANGES[external_tur_std_get(selectedRange$)];
                    const total = external_tur_std_get(entries$).reduce((n, e)=>n + e.count, 0);
                    return `${range.label} · ${total} play${total === 1 ? "" : "s"}`;
                }),
                fontSize: 13,
                color: COLOR_TEXT_MUTED
            })
        ]
    });
}
function BarEntry(props) {
    const fillFlex = props.entry.count;
    const trackFlex = Math.max(0, props.maxCount - props.entry.count);
    // Bar row — two Expanded segments share the width in proportion to
    // count / maxCount. When the entry owns the max, trackFlex is 0 and
    // the fill takes the whole row.
    const barChildren = [
        Expanded({
            flex: fillFlex,
            child: Container({
                color: COLOR_PRIMARY,
                borderRadius: BAR_HEIGHT / 2
            })
        })
    ];
    if (trackFlex > 0) {
        barChildren.push(Expanded({
            flex: trackFlex,
            child: Container({
                color: COLOR_BAR_TRACK,
                borderRadius: BAR_HEIGHT / 2
            })
        }));
    }
    return Container({
        color: COLOR_CARD,
        borderColor: COLOR_DIVIDER,
        borderWidth: 1,
        borderRadius: CARD_RADIUS,
        padding: 14,
        children: [
            Column({
                crossAlignment: CrossAxisAlignment.Stretch,
                mainAxisSize: MainAxisSize.Min,
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
                                        fontSize: 14,
                                        color: COLOR_PRIMARY
                                    }),
                                    SizedBox({
                                        width: 6
                                    }),
                                    Text({
                                        text: props.entry.title,
                                        fontSize: 14,
                                        color: COLOR_TEXT
                                    })
                                ]
                            }),
                            Container({
                                color: COLOR_PRIMARY_SOFT,
                                borderRadius: 999,
                                padding: 6,
                                children: [
                                    Text({
                                        text: String(props.entry.count),
                                        fontSize: 12,
                                        color: COLOR_PRIMARY
                                    })
                                ]
                            })
                        ]
                    }),
                    SizedBox({
                        height: 10
                    }),
                    Container({
                        height: BAR_HEIGHT,
                        children: [
                            Row({
                                children: barChildren
                            })
                        ]
                    })
                ]
            })
        ]
    });
}
function ReadyBody() {
    return ScrollView({
        axis: Axis.Vertical,
        child: Column({
            crossAlignment: CrossAxisAlignment.Stretch,
            mainAxisSize: MainAxisSize.Min,
            children: [
                Each({
                    items: entries$,
                    build: (entry, index)=>{
                        const maxCount = external_tur_std_get(entries$).reduce((m, e)=>Math.max(m, e.count), 0);
                        return Column({
                            crossAlignment: CrossAxisAlignment.Stretch,
                            mainAxisSize: MainAxisSize.Min,
                            children: [
                                index === 0 ? SizedBox({
                                    width: 0,
                                    height: 0
                                }) : SizedBox({
                                    height: 8
                                }),
                                BarEntry({
                                    entry,
                                    maxCount
                                })
                            ]
                        });
                    }
                })
            ]
        })
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
            Column({
                crossAlignment: CrossAxisAlignment.Center,
                mainAxisSize: MainAxisSize.Min,
                children: [
                    Text({
                        text: "♪",
                        fontSize: 32,
                        color: COLOR_DIVIDER
                    }),
                    SizedBox({
                        height: 12
                    }),
                    Text({
                        text: "No plays in this range.",
                        fontSize: 14,
                        color: COLOR_TEXT_MUTED
                    })
                ]
            })
        ]
    });
}
// ---------------------------------------------------------------------------
// Root view
// ---------------------------------------------------------------------------
const rootView = view(()=>Column({
        crossAlignment: CrossAxisAlignment.Stretch,
        children: [
            Container({
                color: COLOR_PAGE_BG,
                padding: PAGE_PADDING,
                children: [
                    Header(),
                    SizedBox({
                        height: 16
                    }),
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