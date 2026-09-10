// Play Counts plugin — state, KV aggregation, and tur widget factories.
//
// Data model (KV multi-value, append-only):
//   key   = "plays:YYYY-MM-DD"
//   value = JSON `{ musicId, title, ts }`
//
// Each `music:play` event appends one row (the plugin's own backend module
// does the append, via `db.multiAppend`). This module reads the rows back via
// `db.multiGetAllMulti` from the unified `ease` module, aggregates
// per musicId in JS, and renders a sorted list with two combined filters: a
// time-range selector and a playlist selector (membership filter — rows count
// only when their musicId belongs to the selected playlist, per the host
// roster from `ease.library.playlists()`).
//
// Reactivity note: NO module-level mutable state — everything flows through
// reactive atoms (`source`/`derive`/`mutate` declarations that materialize
// into the instance store). The list is a reactive `Each` over `entries$` —
// item fragments mount/unmount on length change and SURVIVING fragments are
// kept mounted, so every data- AND rank-dependent prop inside a row is a
// `derive` over `entries$`/`maxCount$`/its index (mounted rows update IN
// PLACE — title, count, bar AND the tier reskin). The host playlist roster
// is a `source` atom seeded at module eval; filter visibility, selector
// options + labels, and the membership filter all derive from it. (A
// previous `LazyList` build relied on the same in-place contract but its
// itemCount tail-growth after a shrink→grow round-trip stopped mounting new
// rows, and its build-time static medals went stale on in-place reorder —
// both fixed here.)
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
    Each,
    Fragment,
    ScrollView,
    view,
    source,
    derive,
    mutate,
} from "tur:std";
import type { Source, Readable, Element, StoreCtx } from "tur:core";
import { db as Storage, themes, library } from "ease";
import type { PlaylistInfo } from "ease";
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
// Host playlists — read-only membership roster from `ease.library`, held as
// a reactive ATOM (no module-level data): the filter UI's visibility, the
// selector options + labels, and the membership filter in `refresh$` all
// derive from it. Seeded once at module eval (the same timing as the
// `themes` reads — views load long after the host is up); a rename/create/
// delete while the page stays open is picked up on the next open, since
// views re-eval per open.
// ---------------------------------------------------------------------------

const playlists$ = source<PlaylistInfo[]>(library.playlists());

/** Whether the playlist filter renders at all (empty roster → hidden). */
const hasPlaylistFilters$ = derive((ctx) => ctx.get(playlists$).length > 0);

/** Sentinel playlist-filter value: no membership filtering. */
const PLAYLIST_ALL = "all";

function playlistById(roster: PlaylistInfo[], id: string): PlaylistInfo | undefined {
    return roster.find((p) => p.id === id);
}

// ---------------------------------------------------------------------------
// Reactive state
// ---------------------------------------------------------------------------

const selectedRange$: Source<number> = source(0);
// Playlist filter value: `PLAYLIST_ALL` or a playlist id. Falling back to
// "all" is impossible mid-session (selection only takes values from the
// mounted selector), but `refresh$` still tolerates a vanished playlist
// (deleted between page opens) by ignoring the filter for unknown ids.
const selectedPlaylist$: Source<string> = source(PLAYLIST_ALL);
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

        // Membership filter: `null` = count every row; otherwise only rows
        // whose musicId belongs to the selected playlist. Play rows store
        // `String(musicId)` and `library.playlists()` returns the same
        // stringified id form, so plain string membership is exact.
        const roster = ctx.get(playlists$);
        const selPlaylist = ctx.get(selectedPlaylist$);
        const selInfo = selPlaylist === PLAYLIST_ALL ? undefined : playlistById(roster, selPlaylist);
        const allowed: Set<string> | null =
            selPlaylist !== PLAYLIST_ALL && selInfo
                ? new Set(selInfo.musicIds)
                : null;

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
                if (allowed !== null && !allowed.has(id)) continue;
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
// Tail discs: one container step ABOVE the card fill so they stand off it —
// the old `surfaceVariant` disc was near-invisible on dark cards.
const COLOR_MUTED_BADGE: Color = Color.hex(themes.color("surfaceContainerHighest"));
const COLOR_MUTED_NUM: Color = Color.hex(themes.color("onSurfaceVariant"));

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

const PAGE_PADDING = 20;
const CHIP_RADIUS = 999;

// ---------------------------------------------------------------------------
// Selectors — range + playlist, both built from the reusable `createSelector`
// (CompositedTransform anchored to the trigger; the menu floats over the
// list, no absolute coords). Mutual exclusion is free: each open menu's
// page-wide scrim eats any tap (including the other trigger), so only one
// can be open at a time.
// ---------------------------------------------------------------------------

// Shared pill palette for standalone (filled) triggers.
const SELECTOR_STYLE = {
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
};

// Bare trigger — NO fill, NO border (the two filters live INSIDE the shared
// segmented `FilterBar` container, which carries the tonal chrome for both;
// a pill-in-a-pill would double the outline).
const SELECTOR_STYLE_BARE = {
    ...SELECTOR_STYLE,
    triggerBg: Color.rgba(0, 0, 0, 0),
};

// Range change: update the selection + re-aggregate, composed as one
// mutation dispatched through the click ctx.
const selectRange$ = mutate((ctx: StoreCtx, id: string): void => {
    const i = RANGES.findIndex((r) => r.id === id);
    if (i >= 0 && i !== ctx.get(selectedRange$)) {
        ctx.set(selectedRange$, i);
        ctx.set(refresh$);
    }
});

const RANGE_OPTIONS = RANGES.map((r) => ({ value: r.id, label: r.label }));

const rangeSel = createSelector<string>({
    options$: derive(() => RANGE_OPTIONS),
    selectedValue$: derive(
        (ctx) => RANGES[ctx.get(selectedRange$)]?.id ?? RANGES[0].id,
    ),
    onSelect$: selectRange$,
    style: SELECTOR_STYLE_BARE,
});

// Playlist change: same composed mutation as the range change.
const selectPlaylist$ = mutate((ctx: StoreCtx, id: string): void => {
    if (id !== ctx.get(selectedPlaylist$)) {
        ctx.set(selectedPlaylist$, id);
        ctx.set(refresh$);
    }
});

const playlistSel = createSelector<string>({
    options$: derive((ctx) => [
        { value: PLAYLIST_ALL, label: "All Playlists" },
        ...ctx.get(playlists$).map((p) => ({ value: p.id, label: p.title })),
    ]),
    selectedValue$: selectedPlaylist$,
    onSelect$: selectPlaylist$,
    label$: derive((ctx) => {
        const v = ctx.get(selectedPlaylist$);
        if (v === PLAYLIST_ALL) return "All Playlists";
        return playlistById(ctx.get(playlists$), v)?.title ?? "";
    }),
    menuWidth: 240,
    // NOTE: both triggers live in the right-hand `FilterBar` now, so the
    // default leftward-opening anchor is correct for BOTH menus (the
    // leftward `"left"` mode was for the old left-side pill row).
    style: SELECTOR_STYLE_BARE,
});

// ---------------------------------------------------------------------------
// Header
// ---------------------------------------------------------------------------

// Header stat. The Compose top bar already carries the page title (the
// plugin's localized manifest name), so the in-view header is just the
// headline number + a muted unit — no duplicated H1, and the range context
// lives in the selector pill right next to it.
function HeaderSummary() {
    return Row()
        .crossAlignment(CrossAxisAlignment.Center)
        .mainAxisSize(MainAxisSize.Min)
        .children([
            Text({ text: derive((ctx) => String(ctx.get(total$))) })
                .fontSize(22)
                .color(COLOR_TEXT)
                .build(),
            SizedBox()
                .width(6)
                .build(),
            Text({ text: derive((ctx) => (ctx.get(total$) === 1 ? "play" : "plays")) })
                .fontSize(13)
                .color(COLOR_TEXT_MUTED)
                .build(),
        ])
        .build();
}

// Filter bar — ONE tonal segmented container holding both filter triggers,
// so the two controls read as a single group instead of loose chips. The
// container carries the chrome (fill/radius); the triggers are bare
// (see `SELECTOR_STYLE_BARE`). A hairline divider separates the segments.
// The playlist segment mounts only while the roster is non-empty — a
// `Condition` over the `playlists$` atom, not a build-time branch.
function FilterBar() {
    return Container()
        .color(
            IS_DARK
                ? withAlpha(themes.color("primary"), 58)
                : withAlpha(themes.color("secondary"), 205),
        )
        .borderRadius(CHIP_RADIUS)
        .padding(4)
        .children([
            Row()
                .crossAlignment(CrossAxisAlignment.Center)
                .mainAxisSize(MainAxisSize.Min)
                .children([
                    rangeSel.SelectorTrigger(),
                    Condition({ condition: hasPlaylistFilters$ })
                        .child(() =>
                            Fragment()
                                .children([
                                    Container()
                                        .width(1)
                                        .height(20)
                                        .color(withAlpha(themes.color("outlineVariant"), 140))
                                        .build(),
                                    playlistSel.SelectorTrigger(),
                                ])
                                .build())
                        .build(),
                ])
                .build(),
        ])
        .build();
}

// Header: the headline number on the left, the filter group on the right —
// one row. The Compose top bar already carries the page title, so the
// in-view header is just the stat + the filters.
function HeaderRow() {
    return Row()
        .mainAlignment(MainAxisAlignment.SpaceBetween)
        .crossAlignment(CrossAxisAlignment.Center)
        .children([
            Expanded()
                .child(HeaderSummary())
                .build(),
            SizedBox()
                .width(8)
                .build(),
            FilterBar(),
        ])
        .build();
}

// ---------------------------------------------------------------------------
// Ranked row
// ---------------------------------------------------------------------------

interface RankedRowProps {
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

// Top 3 get medal discs (gold/silver/bronze) sized to carry the page's
// color story; rank 4+ gets a small muted disc that still reads against the
// card. The medal is the ONLY tier-colored element on a card.
function medalSpec(rank: number): MedalSpec {
    if (rank === 1)
        return { diameter: 48, fill: COLOR_GOLD, ring: COLOR_GOLD_RING, num: COLOR_GOLD_NUM, fontSize: 18, bold: true };
    if (rank === 2)
        return { diameter: 44, fill: COLOR_SILVER, ring: COLOR_SILVER_RING, num: COLOR_SILVER_NUM, fontSize: 16, bold: true };
    if (rank === 3)
        return { diameter: 44, fill: COLOR_BRONZE, ring: COLOR_BRONZE_RING, num: COLOR_BRONZE_NUM, fontSize: 16, bold: true };
    return { diameter: 28, fill: COLOR_MUTED_BADGE, ring: COLOR_MUTED_BADGE, num: COLOR_MUTED_NUM, fontSize: 12, bold: false };
}

// Leading rank badge — a centered numeral on a colored disc. Rank arrives as
// a `Readable` (rows are kept mounted while the list reorders — see the
// file-top reactivity note), so every tier-dependent prop is a derive: a row
// sliding between podium and tail reskins its medal IN PLACE.
function RankBadge(props: { rank$: Readable<number> }): Element {
    const m$ = derive((ctx) => medalSpec(ctx.get(props.rank$)));
    return Container()
        .width(derive((ctx) => ctx.get(m$).diameter))
        .height(derive((ctx) => ctx.get(m$).diameter))
        .alignment(Alignment.Center)
        .color(derive((ctx) => ctx.get(m$).fill))
        .borderColor(derive((ctx) => ctx.get(m$).ring))
        .borderWidth(derive((ctx) => (ctx.get(m$).bold ? 2 : 1)))
        .borderRadius(derive((ctx) => ctx.get(m$).diameter / 2))
        .children([
            Text({ text: derive((ctx) => String(ctx.get(props.rank$))) })
                .fontSize(derive((ctx) => ctx.get(m$).fontSize))
                .color(derive((ctx) => ctx.get(m$).num))
                .build(),
        ])
        .build();
}

// One ranked row. Reads `entries$` REACTIVELY by `index` (see the file-top
// reactivity note): item fragments are kept mounted across data changes, so
// EVERY rank- or data-dependent value below is a `derive` — title, count,
// bar AND the medal/tier reskin — letting rows reorder in place without
// remounting.
//
// Visual system (cohesive): the medal is the single tier signal. Card fill
// and the brand-blue bar are identical for every rank, so the podium reads
// as one family of cards (distinguished by medal + slightly taller padding)
// rather than four colored echoes. Cards float on a tonal container step
// with a soft shadow — no keyline border.
function RankedRow(props: RankedRowProps): Element {
    const { index } = props;
    const barHeight = 6;

    const rank$ = derive((ctx) => index + 1);
    const tierA$ = derive((ctx) => ctx.get(rank$) <= 3);

    const entry$ = derive((ctx) => ctx.get(entries$)[index]);
    const title$ = derive((ctx) => ctx.get(entry$)?.title ?? "");
    const count$ = derive((ctx) => ctx.get(entry$)?.count ?? 0);
    const fillFlex$ = derive((ctx) => ctx.get(count$));
    const trackFlex$ = derive((ctx) => Math.max(0, ctx.get(maxCount$) - ctx.get(count$)));

    // Bar + count read as ONE glance: fill/track flexes keep their ratio and
    // the exact count sits right at the bar's end (the old right-edge count
    // chip duplicated the bar's information with no shared scale cue).
    const barChildren: Element[] = [
        Expanded()
            .flex(fillFlex$)
            .child(Container()
                .height(barHeight)
                .color(COLOR_PRIMARY)
                .borderRadius(barHeight / 2)
                .build())
            .build(),
        Expanded()
            .flex(trackFlex$)
            .child(Container()
                .height(barHeight)
                .color(COLOR_BAR_TRACK)
                .borderRadius(barHeight / 2)
                .build())
            .build(),
        SizedBox()
            .width(8)
            .build(),
        Text({ text: derive((ctx) => String(ctx.get(count$))) })
            .fontSize(12)
            .color(COLOR_TEXT_MUTED)
            .build(),
    ];

    // NOTE: tur `Text` has no `fontWeight` prop, and span `content` is parsed
    // to a static `String` at build time — so a *reactive* bold title (spans
    // over a reactive `text`) is impossible: the base text refreshes at
    // layout while the span byte-ranges stay frozen, desyncing. Hierarchy is
    // therefore expressed via size + color contrast (podium dark, tail muted)
    // plus the medal, not bold weight.
    const title: Element = Text({ text: title$ })
        .fontSize(derive((ctx) => (ctx.get(tierA$) ? 16 : 14)))
        .color(derive((ctx) => (ctx.get(tierA$) ? COLOR_TEXT : COLOR_TEXT_MUTED)))
        .maxLines(1)
        .overflow("ellipsis")
        .build();

    return Container()
        .color(COLOR_CARD)
        .borderRadius(12)
        .shadowColor(COLOR_SHADOW)
        .shadowBlur(6)
        .shadowOffset([0, 2])
        .children([
            Container()
                .padding(derive((ctx) => (ctx.get(tierA$) ? 16 : 12)))
                .children([
                    Column()
                        .crossAlignment(CrossAxisAlignment.Stretch)
                        .mainAxisSize(MainAxisSize.Min)
                        .children([
                            Row()
                                .mainAlignment(MainAxisAlignment.Start)
                                .crossAlignment(CrossAxisAlignment.Center)
                                .children([
                                    RankBadge({ rank$ }),
                                    SizedBox()
                                        .width(12)
                                        .build(),
                                    Expanded()
                                        .child(title)
                                        .build(),
                                ])
                                .build(),
                            SizedBox()
                                .height(8)
                                .build(),
                            Row()
                                .crossAlignment(CrossAxisAlignment.Center)
                                .children(barChildren)
                                .build(),
                            // Breathing room under the bar so it doesn't hug
                            // the card's bottom edge.
                            SizedBox()
                                .height(3)
                                .build(),
                        ])
                        .build(),
                ])
                .build(),
        ])
        .build();
}

function ReadyBody() {
    // Reactive list: `Each` diffs `entries$` (item fragments mount/unmount
    // on length change) and keeps surviving fragments mounted, where the
    // fully-derived rows update in place. Not virtualized — this page's row
    // counts are modest; the `ScrollView` supplies scrolling (its offset
    // clamps into the new extents when content shrinks — Flutter
    // `applyContentDimensions` parity, tur #226).
    return ScrollView()
        .axis(Axis.Vertical)
        .child(
            Column()
                .crossAlignment(CrossAxisAlignment.Stretch)
                .mainAxisSize(MainAxisSize.Min)
                .children([
                    // Section label over the podium (the top bar carries the
                    // page title, so this is the only heading in the view).
                    // Shown only when there IS a tail below the podium — with
                    // ≤3 cards the medals already tell the whole story and a
                    // "TOP 2" caption over two cards is noise.
                    Condition({ condition: derive((ctx) => ctx.get(entries$).length > 3) })
                        .child(() =>
                            Text({ text: derive((ctx) => {
                                const n = ctx.get(entries$).length;
                                return `TOP ${Math.min(3, n)}`;
                            }) })
                                .fontSize(11)
                                .color(COLOR_TEXT_MUTED)
                                .build())
                        .build(),
                    SizedBox()
                        .height(8)
                        .build(),
                    Each({ items: entries$ })
                        .itemBuilder((_entry: PlayCountEntry, index: number) => {
                            const children: Element[] = [];
                            if (index > 0) {
                                // Boundary between podium (1–3) and the tail
                                // (4+): the density change IS the separator —
                                // no hairline divider.
                                children.push(SizedBox()
                                    .height(index === 3 ? 24 : 12)
                                    .build());
                            }
                            children.push(RankedRow({ index }));
                            return Column()
                                .children(children)
                                .crossAlignment(CrossAxisAlignment.Stretch)
                                .mainAxisSize(MainAxisSize.Min)
                                .build();
                        })
                        .build(),
                    // Trailing bottom inset.
                    SizedBox()
                        .height(24)
                        .build(),
                ])
                .build(),
        )
        .build();
}

function LoadingBody() {
    return Container()
        .padding(48)
        .children([
            Text({ text: "Loading…" })
                .fontSize(14)
                .color(COLOR_TEXT_MUTED)
                .build(),
        ])
        .build();
}

function EmptyBody() {
    // Centered in the visible list area (the Switch child gets the list's
    // bounded height): a tonal disc carrying the note glyph, the message,
    // and a short hint — an intentional resting state, not a dead corner.
    return Column()
        .crossAlignment(CrossAxisAlignment.Center)
        .mainAlignment(MainAxisAlignment.Center)
        .children([
            Container()
                .width(88)
                .height(88)
                .alignment(Alignment.Center)
                .color(Color.hex(themes.color("surfaceContainerHigh")))
                .borderRadius(44)
                .children([
                    Text({ text: "♪" })
                        .fontSize(34)
                        .color(withAlpha(themes.color("primary"), 170))
                        .build(),
                ])
                .build(),
            SizedBox()
                .height(16)
                .build(),
            Text({ text: "No plays in this range." })
                .fontSize(14)
                .color(COLOR_TEXT)
                .build(),
            SizedBox()
                .height(4)
                .build(),
            Text({ text: "Try a wider range." })
                .fontSize(12)
                .color(COLOR_TEXT_MUTED)
                .build(),
        ])
        .build();
}

// ---------------------------------------------------------------------------
// Root view
// ---------------------------------------------------------------------------

// Root: a page-level `Stack`.
//   z0 — base content (padded page bg + header column + the status body, the
//        scroll view wrapped in `Expanded` so it gets a bounded height);
//   z1 — selector scrims (page-wide backdrop, tap to close);
//   z2 — selector menus (CompositedTransformFollowers that anchor themselves
//        to their trigger's bottom-right + padding — no absolute coordinates).
// The scrims + followers are `Condition`-gated fragments, so the `Stack`
// sees the nested `Positioned`/follower directly. The followers live in
// this root `Stack` (the "root overlay slot") so they aren't clipped by
// ancestors.
export const rootView = view(() => {
    const layers: Element[] = [
        Container()
            .color(COLOR_PAGE_BG)
            .padding(PAGE_PADDING)
            .children([
                Column()
                    .crossAlignment(CrossAxisAlignment.Stretch)
                    .children([
                        HeaderRow(),
                        SizedBox()
                            .height(14)
                            .build(),
                        Expanded()
                            .child(Switch({ value: status$ })
                                .cases([
                                    { key: "loading", child: LoadingBody },
                                    { key: "empty", child: EmptyBody },
                                    { key: "ready", child: ReadyBody },
                                ])
                                .build())
                            .build(),
                    ])
                    .build(),
            ])
            .build(),
        rangeSel.SelectorScrim(),
        rangeSel.SelectorMenu(),
        Condition({ condition: hasPlaylistFilters$ })
            .child(() =>
                Fragment()
                    .children([playlistSel.SelectorScrim(), playlistSel.SelectorMenu()])
                    .build())
            .build(),
    ];
    return Stack()
        .children(layers)
        .build();
});
