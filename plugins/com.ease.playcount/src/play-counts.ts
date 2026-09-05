// Play Counts plugin — state, KV aggregation, and tur widget factories.
//
// Data model (KV multi-value, append-only):
//   key   = "plays:YYYY-MM-DD"
//   value = JSON `{ musicId, title, ts }`
//
// Each `music:play` event appends one row (the plugin's own backend module
// does the append, via `db.multiAppend`). This module reads the rows back via
// `db.multiGetAllMulti` from the unified `ease` module, aggregates
// per musicId in JS, and renders a sorted list with a time-range selector.
//
// Reactivity note: a tur `LazyList` only invokes its `builder` when an item
// is freshly MOUNTED — it never rebuilds an already-mounted item when the
// underlying data changes (it reacts only to axis/itemExtent/itemCount, and
// even an itemCount change only destroys/grows the tail, leaving kept items
// stale). `refresh()` is synchronous, so the `loading → ready` interlude is
// batched away and the `Switch` does NOT remount the list. Therefore every
// data-dependent prop inside a row is a `derive` over `entries$`/`maxCount$`,
// so mounted rows update IN PLACE when the range switches. (Verified in
// `tur-engine/.../lazy_list/element.rs`: `build_item_spec` runs only at
// mount; `react_to_prop_changes`/`remount` never rebuild same-index items.)
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
    Alignment,
    CrossAxisAlignment,
    MainAxisAlignment,
    MainAxisSize,
    Switch,
    Expanded,
    Stack,
    Condition,
    LazyList,
    view,
    source,
    derive,
    mutate,
} from "tur:std";
import type { Source, Readable, Element, StoreCtx } from "tur:core";
import { db as Storage, themes } from "ease";
import { createSelector } from "./ui/selector";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface RangeDef {
    id: string;
    label: string;
    days: number;
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
    { id: "today",  label: "Today",       days: 1 },
    { id: "2days",  label: "Last 2 Days", days: 2 },
    { id: "3days",  label: "Last 3 Days", days: 3 },
    { id: "week",   label: "Last Week",   days: 7 },
    { id: "month",  label: "Last Month",  days: 30 },
    { id: "year",   label: "Last Year",   days: 365 },
];

// ---------------------------------------------------------------------------
// Reactive state
// ---------------------------------------------------------------------------

const selectedRange$: Source<number> = source(0);
const entries$: Source<PlayCountEntry[]> = source<PlayCountEntry[]>([]);
const loading$: Source<boolean> = source(false);

const status$: Readable<Status> = derive<Status>((ctx) => {
    if (ctx.get(loading$)) return "loading";
    return ctx.get(entries$).length === 0 ? "empty" : "ready";
});

// Max play count across the current range — drives every bar's fill ratio.
// Reactive so all mounted bars rescale together when `entries$` changes.
const maxCount$: Readable<number> = derive((ctx) => {
    let m = 0;
    for (const e of ctx.get(entries$)) if (e.count > m) m = e.count;
    return m;
});

// Total plays in the current range — the header stat.
const total$: Readable<number> = derive((ctx) =>
    ctx.get(entries$).reduce((n, e) => n + e.count, 0),
);

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
    // "Last N days" is inclusive of today: offsets 0 .. N-1.
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

// A mutation (dispatched from `start({ store })` / the range selector) so it
// can write through the instance store's ctx — there is no module-level store.
export const refresh$ = mutate((ctx: StoreCtx): void => {
    ctx.set(loading$, true);
    try {
        const range = RANGES[ctx.get(selectedRange$)];
        const keys = dateKeysForRange(range);
        const grouped = Storage.multiGetAllMulti(keys);

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
        ctx.set(entries$, sorted);
    } catch {
        ctx.set(entries$, []);
    } finally {
        ctx.set(loading$, false);
    }
});

// ---------------------------------------------------------------------------
// Palette — the HOST app's Material 3 theme, read via `ease:themes` (views
// load after the host pushes its theme, so module-eval-time reads are safe;
// `themes.color` throws on unknown names — a miss is a typo, not a race).
// The app's `primary` (#2E89B0) and `secondary` (#C9EBFA) ARE the brand
// pair this page was designed around; page/card/text/divider colors become
// real roles so the page follows the app's dark/light scheme instead of
// shipping its own light-only palette.
//
// Design rule: ONE tier accent per card, expressed ONLY by the medal.
// Gold / silver / bronze are semantic tier colors and stay fixed; the
// long-tail badge uses theme roles. Everything else (page, card, bar, text,
// count pill) is identical across ranks, so each card reads as one
// coherent object rather than a collection of colored parts. Cards rely on
// TONAL elevation (a container step above the page bg + a soft shadow) —
// no hard keyline border — and repeated accents (chips, the range trigger)
// are low-alpha tints with colored text so they never out-shout the medals.
// ---------------------------------------------------------------------------

const COLOR_PRIMARY: Color = Color.hex(themes.color("primary"));
// The app's `secondary` is the soft tint of `primary` (the former
// "primarySoft").
const COLOR_PRIMARY_SOFT: Color = Color.hex(themes.color("secondary"));
const COLOR_PAGE_BG: Color = Color.hex(themes.color("background"));
// One container step above the page background — cards read as raised
// surfaces without a border keyline.
const COLOR_CARD: Color = Color.hex(themes.color("surfaceContainerHigh"));
const COLOR_TEXT: Color = Color.hex(themes.color("onSurface"));
const COLOR_TEXT_MUTED: Color = Color.hex(themes.color("onSurfaceVariant"));
const COLOR_DIVIDER: Color = Color.hex(themes.color("outlineVariant"));
// Near the card tone — the track must recede; the fill + count carry the
// data (a full-width `outlineVariant` track was the loudest gray on screen).
const COLOR_BAR_TRACK: Color = Color.hex(
    themes.color("surfaceContainerHighest"),
);
// Translucent near-black in both schemes — barely visible on dark, which is
// the conventional treatment (dark UIs get their elevation from surfaces).
const COLOR_SHADOW: Color = Color.rgba(15, 23, 42, 28);

// Scheme flag + alpha helper. `themes.color` returns "#RRGGBBAA" hex; this
// derives translucent brand tints at runtime so accents follow the theme.
const IS_DARK: boolean = themes.isDark();

function withAlpha(hex: string, alpha: number): Color {
    const h = hex.replace("#", "");
    return Color.rgba(
        parseInt(h.slice(0, 2), 16),
        parseInt(h.slice(2, 4), 16),
        parseInt(h.slice(4, 6), 16),
        alpha,
    );
}

// Count chips — a quiet tint of the brand blue with colored text. Dark:
// bright `secondary` text on a faint fill; light: near-solid soft fill with
// the steel-blue text (the legacy look, slightly softened). Either way the
// repeated chip stops being the brightest element on every row.
const COLOR_CHIP_BG: Color = IS_DARK
    ? withAlpha(themes.color("secondary"), 38)
    : withAlpha(themes.color("secondary"), 235);
const COLOR_CHIP_TEXT: Color = IS_DARK ? COLOR_PRIMARY_SOFT : COLOR_PRIMARY;

// Medal fills — the SINGLE tier signal (gold / silver / bronze for the
// podium; a themed muted disc for the long tail). Numerals stay dark on
// gold/silver (legibility) and white on the darker bronze.
const COLOR_GOLD: Color = Color.hex("#F5C400");
const COLOR_GOLD_RING: Color = Color.hex("#E2B400");
const COLOR_GOLD_NUM: Color = Color.hex("#3D2E00");
const COLOR_SILVER: Color = Color.hex("#C6CDD6");
const COLOR_SILVER_RING: Color = Color.hex("#98A3B3");
const COLOR_SILVER_NUM: Color = Color.hex("#2F3640");
const COLOR_BRONZE: Color = Color.hex("#CD7F32");
const COLOR_BRONZE_RING: Color = Color.hex("#A05A1E");
const COLOR_BRONZE_NUM: Color = Color.hex("#FFFFFF");
const COLOR_MUTED_BADGE: Color = Color.hex(themes.color("surfaceVariant"));
const COLOR_MUTED_NUM: Color = Color.hex(themes.color("onSurfaceVariant"));

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

const PAGE_PADDING = 20;
const CHIP_RADIUS = 999;

// ---------------------------------------------------------------------------
// Range selector — built from the reusable `createSelector` (CompositedTransform
// anchored to the trigger; the menu floats over the list, no absolute coords).
// ---------------------------------------------------------------------------

// Range change: update the selection + re-aggregate, composed as one
// mutation dispatched through the click ctx.
const selectRange$ = mutate((ctx: StoreCtx, id: string): void => {
    const i = RANGES.findIndex((r) => r.id === id);
    if (i >= 0 && i !== ctx.get(selectedRange$)) {
        ctx.set(selectedRange$, i);
        ctx.set(refresh$);
    }
});

const rangeSel = createSelector<string>({
    options: RANGES.map((r) => ({ value: r.id, label: r.label })),
    selectedValue$: derive(
        (ctx) => RANGES[ctx.get(selectedRange$)]?.id ?? RANGES[0].id,
    ),
    onSelect$: selectRange$,
    style: {
        primary: COLOR_PRIMARY,
        primarySoft: COLOR_PRIMARY_SOFT,
        surface: COLOR_CARD,
        text: COLOR_TEXT,
        textMuted: COLOR_TEXT_MUTED,
        divider: COLOR_DIVIDER,
        shadow: COLOR_SHADOW,
        // Tonal trigger: brand-tinted pill, no keyline, accent-colored label
        // (ties the control to the bars instead of white-on-gray).
        triggerBg: IS_DARK
            ? withAlpha(themes.color("primary"), 46)
            : withAlpha(themes.color("secondary"), 200),
        triggerText: IS_DARK ? COLOR_PRIMARY_SOFT : COLOR_PRIMARY,
    },
});

// ---------------------------------------------------------------------------
// Header
// ---------------------------------------------------------------------------

// Header stat. The Compose top bar already carries the page title (the
// plugin's localized manifest name), so the in-view header is just the
// headline number + a muted unit — no duplicated H1, and the range context
// lives in the selector pill right next to it.
function HeaderSummary() {
    return Row({
        crossAlignment: CrossAxisAlignment.Center,
        mainAxisSize: MainAxisSize.Min,
        children: [
            Text({
                text: derive((ctx) => String(ctx.get(total$))),
                fontSize: 22,
                color: COLOR_TEXT,
            }),
            SizedBox({ width: 6 }),
            Text({
                text: derive((ctx) =>
                    ctx.get(total$) === 1 ? "play" : "plays",
                ),
                fontSize: 13,
                color: COLOR_TEXT_MUTED,
            }),
        ],
    });
}

// Header row: stat on the left, range trigger pill on the right. The
// trigger is a CompositedTransform target; the menu (follower) anchors to
// its bottom-left from the page-level `Stack` in `rootView`.
function HeaderRow() {
    return Row({
        mainAlignment: MainAxisAlignment.SpaceBetween,
        crossAlignment: CrossAxisAlignment.Center,
        children: [HeaderSummary(), rangeSel.SelectorTrigger()],
    });
}

// ---------------------------------------------------------------------------
// Ranked row
// ---------------------------------------------------------------------------

interface RankedRowProps {
    rank: number;
    index: number;
}

interface MedalSpec {
    diameter: number;
    fill: Color;
    ring: Color;
    num: Color;
    fontSize: number;
    bold: boolean;
}

// Top 3 get medal discs (gold/silver/bronze); rank 4+ gets a small muted
// disc. The medal is the ONLY tier-colored element on a card.
function medalSpec(rank: number): MedalSpec {
    if (rank === 1)
        return { diameter: 36, fill: COLOR_GOLD, ring: COLOR_GOLD_RING, num: COLOR_GOLD_NUM, fontSize: 16, bold: true };
    if (rank === 2)
        return { diameter: 32, fill: COLOR_SILVER, ring: COLOR_SILVER_RING, num: COLOR_SILVER_NUM, fontSize: 15, bold: true };
    if (rank === 3)
        return { diameter: 32, fill: COLOR_BRONZE, ring: COLOR_BRONZE_RING, num: COLOR_BRONZE_NUM, fontSize: 15, bold: true };
    return { diameter: 26, fill: COLOR_MUTED_BADGE, ring: COLOR_MUTED_BADGE, num: COLOR_MUTED_NUM, fontSize: 13, bold: false };
}

// Leading rank badge — a centered numeral on a colored disc.
function RankBadge(props: { rank: number }): Element {
    const m = medalSpec(props.rank);
    return Container({
        width: m.diameter,
        height: m.diameter,
        alignment: Alignment.Center,
        color: m.fill,
        borderColor: m.ring,
        borderWidth: m.bold ? 1.5 : 1,
        borderRadius: m.diameter / 2,
        children: [
            Text({
                text: String(props.rank),
                spans: [{ content: String(props.rank), bold: m.bold, color: m.num }],
                fontSize: m.fontSize,
            }),
        ],
    });
}

// One ranked row. Reads `entries$` REACTIVELY by `index` (see the file-top
// reactivity note): because a `LazyList` builder only runs at mount, every
// data-dependent value below is a `derive`, so switching range updates the
// title / count / bar in place without remounting.
//
// Visual system (cohesive): the medal is the single tier signal. Card fill
// and the brand-blue bar are identical for every rank, so the podium reads
// as one family of cards (distinguished by medal + slightly taller padding)
// rather than four colored echoes. Cards float on a tonal container step
// with a soft shadow — no keyline border.
function RankedRow(props: RankedRowProps): Element {
    const { rank, index } = props;
    const tierA = rank <= 3;
    const m = medalSpec(rank);
    const barHeight = 6;
    const pad = tierA ? 16 : 12;

    const entry$ = derive((ctx) => ctx.get(entries$)[index]);
    const title$ = derive((ctx) => ctx.get(entry$)?.title ?? "");
    const count$ = derive((ctx) => ctx.get(entry$)?.count ?? 0);
    const fillFlex$ = derive((ctx) => ctx.get(count$));
    const trackFlex$ = derive((ctx) => Math.max(0, ctx.get(maxCount$) - ctx.get(count$)));

    const barChildren: Element[] = [
        Expanded({
            flex: fillFlex$,
            child: Container({ color: COLOR_PRIMARY, borderRadius: barHeight / 2 }),
        }),
        Expanded({
            flex: trackFlex$,
            child: Container({ color: COLOR_BAR_TRACK, borderRadius: barHeight / 2 }),
        }),
    ];

    // NOTE: tur `Text` has no `fontWeight` prop, and span `content` is parsed
    // to a static `String` at build time — so a *reactive* bold title (spans
    // over a reactive `text`) is impossible: the base text refreshes at
    // layout while the span byte-ranges stay frozen, desyncing. Hierarchy is
    // therefore expressed via size + color contrast (podium dark, tail muted)
    // plus the medal, not bold weight.
    const title: Element = Text({
        text: title$,
        fontSize: tierA ? 16 : 14,
        color: tierA ? COLOR_TEXT : COLOR_TEXT_MUTED,
        maxLines: 1,
        overflow: "ellipsis",
    });

    const countBadge = Container({
        color: COLOR_CHIP_BG,
        borderRadius: CHIP_RADIUS,
        padding: tierA ? 6 : 5,
        children: [
            Text({
                text: derive((ctx) => String(ctx.get(count$))),
                fontSize: tierA ? 13 : 12,
                color: COLOR_CHIP_TEXT,
            }),
        ],
    });

    return Container({
        color: COLOR_CARD,
        borderRadius: 12,
        shadowColor: COLOR_SHADOW,
        shadowBlur: 6,
        shadowOffset: [0, 2],
        children: [
            Container({
                padding: pad,
                children: [
                    Column({
                        crossAlignment: CrossAxisAlignment.Stretch,
                        mainAxisSize: MainAxisSize.Min,
                        children: [
                            Row({
                                mainAlignment: MainAxisAlignment.Start,
                                crossAlignment: CrossAxisAlignment.Center,
                                children: [
                                    RankBadge({ rank }),
                                    SizedBox({ width: 12 }),
                                    Expanded({ child: title }),
                                    SizedBox({ width: 10 }),
                                    countBadge,
                                ],
                            }),
                            SizedBox({ height: 8 }),
                            Container({
                                height: barHeight,
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
            }),
        ],
    });
}

function ReadyBody() {
    // Virtualized list: only visible items (+ overscan) are mounted. The
    // builder captures `index`; each row reads `entries$` reactively, so a
    // range switch updates mounted rows in place (no remount needed).
    return LazyList({
        axis: Axis.Vertical,
        itemCount: derive((ctx) => ctx.get(entries$).length),
        overscan: 6,
        builder: (index: number) => {
            const rank = index + 1;
            const children: Element[] = [];
            if (index === 0) {
                // Section label over the podium (the top bar carries the
                // page title, so this is the only heading in the view).
                // Gated so a single-entry range shows just the card — an
                // empty `Text` would still reserve its line box.
                children.push(
                    Condition({
                        condition: derive(
                            (ctx) => ctx.get(entries$).length > 1,
                        ),
                        child: () =>
                            Text({
                                text: derive((ctx) => {
                                    const n = ctx.get(entries$).length;
                                    return `TOP ${Math.min(3, n)}`;
                                }),
                                fontSize: 11,
                                color: COLOR_TEXT_MUTED,
                            }),
                    }),
                );
                children.push(SizedBox({ height: 8 }));
            } else if (index <= 2) {
                // within the top-3 podium cluster
                children.push(SizedBox({ height: 12 }));
            } else if (index === 3) {
                // boundary between podium (1–3) and the tail (4+): the
                // density change IS the separator — no hairline divider
                children.push(SizedBox({ height: 24 }));
            } else {
                children.push(SizedBox({ height: 12 }));
            }
            children.push(RankedRow({ rank, index }));
            // Trailing bottom inset on the last item. The builder runs only
            // at mount, so the last-ness is a `derive` — a height-0 SizedBox
            // is invisible, and this also tracks list-length changes.
            children.push(
                SizedBox({
                    height: derive(
                        (ctx) =>
                            ctx.get(entries$).length - 1 === index ? 24 : 0,
                    ),
                }),
            );
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
                        color: withAlpha(themes.color("primary"), 150),
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
