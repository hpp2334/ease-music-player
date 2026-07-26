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

import {
    Column,
    Container,
    Row,
    ScrollView,
    SizedBox,
    Text,
    Color,
    Axis,
    CrossAxisAlignment,
    MainAxisAlignment,
    MainAxisSize,
    PointerInteract,
    Switch,
    Each,
    view,
    source,
    derive,
    set,
    get,
    mutate,
} from "tur:std";
import type { Atom, Readable } from "tur:core";
import * as Storage from "ease:storage";

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

const selectedRange$: Atom<number> = source(0);
const entries$: Atom<PlayCountEntry[]> = source<PlayCountEntry[]>([]);
const loading$: Atom<boolean> = source(false);

const status$: Readable<Status> = derive<Status>(() => {
    if (get(loading$)) return "loading";
    return get(entries$).length === 0 ? "empty" : "ready";
});

// ---------------------------------------------------------------------------
// Date helpers (UTC, so the day-key matches what the Kotlin appender writes)
// ---------------------------------------------------------------------------

function pad2(n: number): string {
    return n < 10 ? "0" + n : String(n);
}

function isoDate(d: Date): string {
    return (
        d.getUTCFullYear() +
        "-" +
        pad2(d.getUTCMonth() + 1) +
        "-" +
        pad2(d.getUTCDate())
    );
}

function dateKey(offsetDays: number): string {
    const d = new Date();
    d.setUTCDate(d.getUTCDate() - offsetDays);
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

function selectRange(i: number): void {
    if (i === get(selectedRange$)) return;
    set(selectedRange$, i);
    refresh();
}

// ---------------------------------------------------------------------------
// Palette
// ---------------------------------------------------------------------------

const COLOR_PRIMARY: Color = Color.rgb(0x12, 0x59, 0x66);
const COLOR_PRIMARY_LIGHT: Color = Color.rgb(0xee, 0xee, 0xee);
const COLOR_TEXT: Color = Color.rgb(0x33, 0x33, 0x33);
const COLOR_TEXT_MUTED: Color = Color.rgb(0x99, 0x99, 0x99);
const COLOR_WHITE: Color = Color.rgb(0xff, 0xff, 0xff);

// ---------------------------------------------------------------------------
// Widgets
// ---------------------------------------------------------------------------

interface RangeChipProps {
    index: number;
    label: string;
}

function RangeChip(props: RangeChipProps) {
    const selected = props.index === get(selectedRange$);
    return PointerInteract({
        onPointerDown: mutate(() => selectRange(props.index)),
        child: Container({
            color: selected ? COLOR_PRIMARY : COLOR_PRIMARY_LIGHT,
            padding: 8,
            borderRadius: 16,
            children: [
                Text({
                    text: props.label,
                    fontSize: 13,
                    color: selected ? COLOR_WHITE : COLOR_TEXT_MUTED,
                }),
            ],
        }),
    });
}

function RangeRow() {
    // Plain Row — 6 chips × ~70px ≈ 460px, well within the 1440px viewport.
    // (Previously wrapped in a horizontal `ScrollView`, which leaked an
    // unbounded cross-axis height up to the parent `Column` and starved
    // every sibling below it for vertical space.)
    const children = RANGES.flatMap((r, i) => [
        RangeChip({ index: i, label: r.label }),
        SizedBox({ width: 8 }),
    ]);
    // Drop the trailing spacer.
    if (children.length > 0) children.pop();
    return Row({
        mainAlignment: MainAxisAlignment.Start,
        crossAlignment: CrossAxisAlignment.Center,
        mainAxisSize: MainAxisSize.Min,
        children,
    });
}

function EntryItem(entry: PlayCountEntry) {
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
                                color: COLOR_PRIMARY,
                            }),
                            SizedBox({ width: 8 }),
                            Text({
                                text: entry.title,
                                fontSize: 15,
                                color: COLOR_TEXT,
                            }),
                        ],
                    }),
                    Text({
                        text: String(entry.count),
                        fontSize: 16,
                        color: COLOR_PRIMARY,
                    }),
                ],
            }),
        ],
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
            Text({
                text: "No plays in this range.",
                fontSize: 14,
                color: COLOR_TEXT_MUTED,
            }),
        ],
    });
}

function ReadyBody() {
    // `Each` is itself a flex; lay it out as a stretch column so each
    // `EntryItem` spans the full width.
    return ScrollView({
        axis: Axis.Vertical,
        child: Each<PlayCountEntry>({
            items: entries$,
            build: (entry) => EntryItem(entry),
            crossAlignment: CrossAxisAlignment.Stretch,
        }),
    });
}

// ---------------------------------------------------------------------------
// Root view
// ---------------------------------------------------------------------------

export const rootView = view(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Stretch,
        children: [
            SizedBox({ height: 16 }),
            Container({
                padding: 16,
                children: [RangeRow()],
            }),
            Switch({
                value: status$,
                cases: [
                    { key: "loading", child: LoadingBody },
                    { key: "empty",   child: EmptyBody },
                    { key: "ready",   child: ReadyBody },
                ],
            }),
        ],
    }),
);

export { refresh };
