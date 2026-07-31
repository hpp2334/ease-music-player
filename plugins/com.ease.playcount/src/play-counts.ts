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
//
// Layout note: the tur `Container` only lays out its FIRST child
// (`tur-engine/.../container/layout.rs`), so every decorated `Container`
// below wraps a single `Column`/`Row`. Flex (`Row`/`Column`) lays out all
// children.

import {
    Column,
    Container,
    Row,
    SizedBox,
    Text,
    Color,
    Axis,
    CrossAxisAlignment,
    MainAxisAlignment,
    MainAxisSize,
    Switch,
    Expanded,
    Stack,
    LazyList,
    view,
    source,
    derive,
    set,
    get,
} from "tur:std";
import type { Source, Readable, Element } from "tur:core";
import * as Storage from "ease:storage";
import { createSelector } from "./ui/selector";

const PLUGIN_ID = "com.ease.playcount";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface RangeDef {
    id: string;
    label: string;
    days: number;
    offset: number;
}

interface PlayCountEntry {
    musicId: string;
    title: string;
    count: number;
}

type Status = "loading" | "empty" | "ready";

// ---------------------------------------------------------------------------
// Statics
// ---------------------------------------------------------------------------

const RANGES: RangeDef[] = [
    { id: "today",     label: "Today",       days: 1,   offset: 0 },
    { id: "yesterday", label: "Yesterday",   days: 1,   offset: 1 },
    { id: "3days",     label: "Last 3 Days", days: 3,   offset: 0 },
    { id: "week",      label: "Last Week",   days: 7,   offset: 0 },
    { id: "month",     label: "Last Month",  days: 30,  offset: 0 },
    { id: "year",      label: "Last Year",   days: 365, offset: 0 },
];

// ---------------------------------------------------------------------------
// Reactive state
// ---------------------------------------------------------------------------

const selectedRange$: Source<number> = source(0);
const entries$: Source<PlayCountEntry[]> = source<PlayCountEntry[]>([]);
const loading$: Source<boolean> = source(false);

const status$: Readable<Status> = derive<Status>(() => {
    if (get(loading$)) return "loading";
    return get(entries$).length === 0 ? "empty" : "ready";
});

// ---------------------------------------------------------------------------
// Date helpers (local time, so the day-key matches what the Kotlin appender
// writes via `LocalDate.now()` — which uses the device default timezone).
// ---------------------------------------------------------------------------

function pad2(n: number): string {
    return n < 10 ? "0" + n : String(n);
}

function isoDate(d: Date): string {
    return (
        d.getFullYear() +
        "-" +
        pad2(d.getMonth() + 1) +
        "-" +
        pad2(d.getDate())
    );
}

function dateKey(offsetDays: number): string {
    const d = new Date();
    d.setDate(d.getDate() - offsetDays);
    return "plays:" + isoDate(d);
}

function dateKeysForRange(range: RangeDef): string[] {
    if (range.id === "yesterday") return [dateKey(1)];
    const keys: string[] = [];
    for (let i = 0; i < range.days; i++) keys.push(dateKey(i));
    return keys;
}

// ---------------------------------------------------------------------------
// Refresh — synchronous KV scan + JS aggregation
// ---------------------------------------------------------------------------

interface PlayEventRow {
    musicId?: unknown;
    title?: unknown;
    ts?: unknown;
}

function refresh(): void {
    set(loading$, true);
    try {
        const range = RANGES[get(selectedRange$)];
        const keys = dateKeysForRange(range);
        const grouped = Storage.multiGetAllMulti(PLUGIN_ID, keys);

        const counts = new Map<string, PlayCountEntry>();
        for (const entry of grouped) {
            for (const raw of entry.values) {
                let ev: PlayEventRow;
                try {
                    ev = JSON.parse(raw) as PlayEventRow;
                } catch {
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
                    counts.set(id, { musicId: id, title, count: 1 });
                }
            }
        }

        const sorted = Array.from(counts.values()).sort(
            (a, b) => b.count - a.count,
        );
        set(entries$, sorted);
    } catch {
        set(entries$, []);
    } finally {
        set(loading$, false);
    }
}

// Range selection is driven by the shared `rangeSel` selector handle
// (built below, after the palette/constants). Its `onSelect` maps a range
// id back to the `selectedRange$` index and refreshes.

// ---------------------------------------------------------------------------
// Palette — aligned with EaseMusicPlayerTheme (Material 3 light scheme):
//   primary         = #2E89B0
//   secondary       = #C9EBFA
//   surfaceVariant  = #E3E3E3
// ---------------------------------------------------------------------------

const COLOR_PRIMARY: Color = Color.hex("#2E89B0");
const COLOR_PRIMARY_SOFT: Color = Color.hex("#C9EBFA");
const COLOR_PAGE_BG: Color = Color.hex("#F8FAFC");
const COLOR_CARD: Color = Color.hex("#FFFFFF");
const COLOR_TEXT: Color = Color.hex("#0F172A");
const COLOR_TEXT_MUTED: Color = Color.hex("#64748B");
const COLOR_DIVIDER: Color = Color.hex("#E2E8F0");
const COLOR_BAR_TRACK: Color = Color.hex("#E2E8F0");
const COLOR_SHADOW: Color = Color.rgba(15, 23, 42, 31);
const COLOR_CLEAR: Color = Color.rgba(0, 0, 0, 0);

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

const PAGE_PADDING = 20;
const CARD_RADIUS = 14;
const CHIP_RADIUS = 999;
const BAR_HEIGHT = 6;

// ---------------------------------------------------------------------------
// Range selector — built from the reusable `createSelector` (CompositedTransform
// anchored to the trigger; the menu floats over the list, no absolute coords).
// ---------------------------------------------------------------------------

const rangeSel = createSelector<string>({
    options: RANGES.map((r) => ({ value: r.id, label: r.label })),
    selectedValue$: derive(
        () => RANGES[get(selectedRange$)]?.id ?? RANGES[0].id,
    ),
    onSelect: (id) => {
        const i = RANGES.findIndex((r) => r.id === id);
        if (i >= 0 && i !== get(selectedRange$)) {
            set(selectedRange$, i);
            refresh();
        }
    },
    style: {
        primary: COLOR_PRIMARY,
        primarySoft: COLOR_PRIMARY_SOFT,
        surface: COLOR_CARD,
        text: COLOR_TEXT,
        textMuted: COLOR_TEXT_MUTED,
        divider: COLOR_DIVIDER,
        shadow: COLOR_SHADOW,
    },
});

// ---------------------------------------------------------------------------
// Header
// ---------------------------------------------------------------------------

function HeaderSummary() {
    return Column({
        crossAlignment: CrossAxisAlignment.Start,
        mainAxisSize: MainAxisSize.Min,
        children: [
            Text({
                text: "Play Counts",
                fontSize: 22,
                color: COLOR_TEXT,
            }),
            SizedBox({ height: 4 }),
            Text({
                text: derive(() => {
                    const range = RANGES[get(selectedRange$)];
                    const total = get(entries$).reduce(
                        (n, e) => n + e.count,
                        0,
                    );
                    return `${range.label} · ${total} play${total === 1 ? "" : "s"}`;
                }),
                fontSize: 13,
                color: COLOR_TEXT_MUTED,
            }),
        ],
    });
}

// Header row: title + subtitle on the left, range trigger pill on the
// right. The trigger is a CompositedTransform target; the menu (follower)
// anchors to its bottom-left from the page-level `Stack` in `rootView`.
function HeaderRow() {
    return Row({
        mainAlignment: MainAxisAlignment.SpaceBetween,
        crossAlignment: CrossAxisAlignment.Center,
        children: [HeaderSummary(), rangeSel.SelectorTrigger()],
    });
}

// ---------------------------------------------------------------------------
// Bar entry (one per music)
// ---------------------------------------------------------------------------

interface BarEntryProps {
    entry: PlayCountEntry;
    maxCount: number;
}

function BarEntry(props: BarEntryProps) {
    const fillFlex = props.entry.count;
    const trackFlex = Math.max(0, props.maxCount - props.entry.count);

    // The bar is a Row of Expanded segments sharing the card width in
    // proportion to count / maxCount. `crossAlignment: Stretch` is required
    // so the segments fill the 6px bar height (the default Center would
    // collapse childless fill Containers to 0 height).
    const barChildren: Element[] = [
        Expanded({
            flex: fillFlex,
            child: Container({
                color: COLOR_PRIMARY,
                borderRadius: BAR_HEIGHT / 2,
            }),
        }),
    ];
    if (trackFlex > 0) {
        barChildren.push(
            Expanded({
                flex: trackFlex,
                child: Container({
                    color: COLOR_BAR_TRACK,
                    borderRadius: BAR_HEIGHT / 2,
                }),
            }),
        );
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
                        mainAlignment: MainAxisAlignment.Start,
                        crossAlignment: CrossAxisAlignment.Center,
                        children: [
                            Text({
                                text: "♪",
                                fontSize: 14,
                                color: COLOR_PRIMARY,
                            }),
                            SizedBox({ width: 8 }),
                            Expanded({
                                child: Text({
                                    text: props.entry.title,
                                    fontSize: 14,
                                    color: COLOR_TEXT,
                                    maxLines: 1,
                                    overflow: "ellipsis",
                                }),
                            }),
                            SizedBox({ width: 10 }),
                            Container({
                                color: COLOR_PRIMARY_SOFT,
                                borderRadius: CHIP_RADIUS,
                                padding: 5,
                                children: [
                                    Text({
                                        text: String(props.entry.count),
                                        fontSize: 12,
                                        color: COLOR_PRIMARY,
                                    }),
                                ],
                            }),
                        ],
                    }),
                    SizedBox({ height: 10 }),
                    Container({
                        height: BAR_HEIGHT,
                        children: [
                            Row({
                                crossAlignment: CrossAxisAlignment.Stretch,
                                children: barChildren,
                            }),
                        ],
                    }),
                ],
            }),
        ],
    });
}

function ReadyBody() {
    // Virtualized list: only visible items (+ overscan) are mounted, so the
    // view stays cheap with hundreds of tracks. `itemCount` is reactive on
    // `entries$`, so a refresh re-layouts automatically.
    return LazyList({
        axis: Axis.Vertical,
        itemCount: derive(() => get(entries$).length),
        overscan: 6,
        builder: (index: number) => {
            const entries = get(entries$);
            const entry = entries[index];
            const maxCount = entries.reduce(
                (m, e) => Math.max(m, e.count),
                0,
            );
            const children: Element[] = [];
            if (index > 0) children.push(SizedBox({ height: 8 }));
            children.push(BarEntry({ entry, maxCount }));
            if (index === entries.length - 1) {
                children.push(SizedBox({ height: 24 }));
            }
            return Column({
                crossAlignment: CrossAxisAlignment.Stretch,
                mainAxisSize: MainAxisSize.Min,
                children,
            });
        },
    });
}

function LoadingBody() {
    return Container({
        padding: 48,
        children: [
            Text({
                text: "Loading…",
                fontSize: 14,
                color: COLOR_TEXT_MUTED,
            }),
        ],
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
                        color: COLOR_DIVIDER,
                    }),
                    SizedBox({ height: 12 }),
                    Text({
                        text: "No plays in this range.",
                        fontSize: 14,
                        color: COLOR_TEXT_MUTED,
                    }),
                ],
            }),
        ],
    });
}

// ---------------------------------------------------------------------------
// Root view
// ---------------------------------------------------------------------------

// Root: a page-level `Stack`.
//   z0 — base content (padded page bg + header row + the status body, the
//        list body wrapped in `Expanded` so the `LazyList` gets a bounded
//        height and can scroll);
//   z1 — selector scrim (page-wide backdrop, tap to close);
//   z2 — selector menu (a CompositedTransformFollower that anchors itself
//        to the trigger's bottom-left + padding — no absolute coordinates).
// The scrim + follower are `Condition`-gated fragments, so the `Stack` sees
// the nested `Positioned`/follower directly. The follower lives in this
// root `Stack` (the "root overlay slot") so it isn't clipped by ancestors.
export const rootView = view(() =>
    Stack({
        children: [
            Container({
                color: COLOR_PAGE_BG,
                padding: PAGE_PADDING,
                children: [
                    Column({
                        crossAlignment: CrossAxisAlignment.Stretch,
                        children: [
                            HeaderRow(),
                            SizedBox({ height: 14 }),
                            Expanded({
                                child: Switch({
                                    value: status$,
                                    cases: [
                                        { key: "loading", child: LoadingBody },
                                        { key: "empty", child: EmptyBody },
                                        { key: "ready", child: ReadyBody },
                                    ],
                                }),
                            }),
                        ],
                    }),
                ],
            }),
            rangeSel.SelectorScrim(),
            rangeSel.SelectorMenu(),
        ],
    }),
);

export { refresh };
