// Reusable dropdown selector built on tur's CompositedTransform linking
// (Flutter-style anchor: the menu follower tracks the trigger target through
// layout/scroll/resize). The host owns a page-level `Stack` and places the
// `SelectorScrim` + `SelectorMenu` there so the follower paints above content
// and isn't clipped; `SelectorTrigger` goes anywhere in normal flow.

import {
    CompositedTransformTarget,
    CompositedTransformFollower,
    createLayerLink,
    Alignment,
    Column,
    Row,
    Container,
    SizedBox,
    Text,
    Condition,
    Positioned,
    PointerInteract,
    HitTestBehavior,
    Color,
    MainAxisAlignment,
    CrossAxisAlignment,
    MainAxisSize,
    derive,
    mutate,
    source,
} from "tur:std";
import type { Mutation, Source, Readable, Element } from "tur:core";

export interface SelectorOption<T> {
    value: T;
    label: string;
}

export interface SelectorStyle {
    primary: Color;
    primarySoft: Color;
    surface: Color;
    text: Color;
    textMuted: Color;
    divider: Color;
    shadow: Color;
}

export interface SelectorHandle<T> {
    open$: Source<boolean>;
    SelectorTrigger: () => Element;
    SelectorScrim: () => Element;
    SelectorMenu: () => Element;
}

export interface CreateSelectorOptions<T> {
    options: SelectorOption<T>[];
    selectedValue$: Readable<T>;
    /** Dispatched when the user picks an option — a mutation handle, so it
     *  writes through the click's store ctx (`ctx.set(onSelect$, value)`);
     *  there is no module-level store to write through. */
    onSelect$: Mutation<[T], void>;
    label$?: Readable<string>;
    gap?: number;
    menuWidth?: number;
    triggerHeight?: number;
    /** Full palette — required so every consumer passes its host-theme
     * colors; there is no built-in fallback palette. */
    style: SelectorStyle;
}

export function createSelector<T>(
    opts: CreateSelectorOptions<T>,
): SelectorHandle<T> {
    const style: SelectorStyle = opts.style;
    const gap = opts.gap ?? 6;
    const menuWidth = opts.menuWidth ?? 196;
    const triggerHeight = opts.triggerHeight ?? 36;
    const chipRadius = 999;
    const cardRadius = 14;

    const open$ = source(false);
    const link = createLayerLink();

    const label$: Readable<string> =
        opts.label$ ??
        derive((ctx) => {
            const v = ctx.get(opts.selectedValue$);
            const o = opts.options.find((x) => x.value === v);
            return o ? o.label : "";
        });

    function TriggerPill(): Element {
        return PointerInteract({
            behavior: HitTestBehavior.Opaque,
            onClick: mutate((ctx) => ctx.set(open$, !ctx.get(open$))),
            child: Container({
                height: triggerHeight,
                color: style.surface,
                borderColor: style.divider,
                borderWidth: 1,
                borderRadius: chipRadius,
                children: [
                    Row({
                        mainAlignment: MainAxisAlignment.Start,
                        crossAlignment: CrossAxisAlignment.Center,
                        mainAxisSize: MainAxisSize.Min,
                        children: [
                            SizedBox({ width: 14 }),
                            Text({
                                text: label$,
                                fontSize: 13,
                                color: style.text,
                                maxLines: 1,
                                overflow: "ellipsis",
                            }),
                            SizedBox({ width: 8 }),
                            Text({
                                text: derive((ctx) =>
                                    ctx.get(open$) ? "▲" : "▼",
                                ),
                                fontSize: 10,
                                color: style.primary,
                            }),
                            SizedBox({ width: 14 }),
                        ],
                    }),
                ],
            }),
        });
    }

    function OptionRow(option: SelectorOption<T>): Element {
        const selected$ = derive(
            (ctx) => ctx.get(opts.selectedValue$) === option.value,
        );
        return PointerInteract({
            behavior: HitTestBehavior.Opaque,
            onClick: mutate((ctx) => {
                ctx.set(open$, false);
                if (ctx.get(opts.selectedValue$) !== option.value) {
                    ctx.set(opts.onSelect$, option.value);
                }
            }),
            child: Container({
                color: derive((ctx) =>
                    ctx.get(selected$) ? style.primarySoft : style.surface,
                ),
                borderRadius: 10,
                padding: 10,
                children: [
                    Row({
                        mainAlignment: MainAxisAlignment.SpaceBetween,
                        crossAlignment: CrossAxisAlignment.Center,
                        children: [
                            Text({
                                text: option.label,
                                fontSize: 13,
                                color: derive((ctx) =>
                                    ctx.get(selected$)
                                        ? style.primary
                                        : style.text,
                                ),
                                maxLines: 1,
                                overflow: "ellipsis",
                            }),
                            Condition({
                                condition: selected$,
                                child: () =>
                                    Text({
                                        text: "✓",
                                        fontSize: 13,
                                        color: style.primary,
                                    }),
                            }),
                        ],
                    }),
                ],
            }),
        });
    }

    function MenuCard(): Element {
        const rows: Element[] = [];
        opts.options.forEach((o, i) => {
            if (i > 0) rows.push(SizedBox({ height: 4 }));
            rows.push(OptionRow(o));
        });
        return Container({
            width: menuWidth,
            color: style.surface,
            borderColor: style.divider,
            borderWidth: 1,
            borderRadius: cardRadius,
            shadowColor: style.shadow,
            shadowBlur: 16,
            shadowOffset: [0, 6],
            children: [
                Container({
                    padding: 6,
                    children: [
                        Column({
                            mainAlignment: MainAxisAlignment.Start,
                            crossAlignment: CrossAxisAlignment.Stretch,
                            mainAxisSize: MainAxisSize.Min,
                            children: rows,
                        }),
                    ],
                }),
            ],
        });
    }

    return {
        open$,
        SelectorTrigger: () =>
            CompositedTransformTarget({ link, child: TriggerPill() }),
        // Dismiss backdrop: Condition-gated (Condition OUTSIDE the Positioned)
        // so it only exists while open — an always-mounted fill `Positioned`
        // would steal taps from the trigger below it even with empty content.
        // It is fully transparent (no dim/mask over the page); it only captures
        // an outside tap to close. `right/bottom:0` fills the host page Stack.
        SelectorScrim: () =>
            Condition({
                condition: open$,
                child: () =>
                    Positioned({
                        left: 0,
                        top: 0,
                        right: 0,
                        bottom: 0,
                        child: PointerInteract({
                            behavior: HitTestBehavior.Opaque,
                            onClick: mutate((ctx) => ctx.set(open$, false)),
                            child: Container({ color: Color.rgba(0, 0, 0, 0) }),
                        }),
                    }),
            }),
        // Menu: a DIRECT Stack child (always mounted) so the
        // CompositedTransformSubsystem tracks and repositions it; the
        // `Condition` gating open/close lives INSIDE so the follower stays
        // linked. The trigger sits top-right, so the menu opens right-aligned:
        // the follower's top-right (`followerAnchor`) lands on the pill's
        // bottom-right (`targetAnchor`), so the card extends leftward and
        // stays on-screen.
        SelectorMenu: () =>
            CompositedTransformFollower({
                link,
                targetAnchor: Alignment.BottomRight,
                followerAnchor: Alignment.TopRight,
                targetOffset: { x: 0, y: gap },
                child: Condition({
                    condition: open$,
                    child: () => MenuCard(),
                }),
            }),
    };
}
