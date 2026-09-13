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
    Container,
    Row,
    SizedBox,
    Text,
    Condition,
    Each,
    Flexible,
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
    /** Optional tonal trigger restyle: when `triggerBg` is set, the pill
     * drops its keyline border and uses this fill (typically a low-alpha
     * brand tint) with `triggerText` for BOTH the label and the caret —
     * a colored-on-tint control instead of text-on-surface. */
    triggerBg?: Color;
    triggerText?: Color;
}

export interface SelectorHandle<T> {
    open$: Source<boolean>;
    SelectorTrigger: () => Element;
    SelectorScrim: () => Element;
    SelectorMenu: () => Element;
}

export interface CreateSelectorOptions<T> {
    /** Reactive option list — the menu rows re-render when it changes
     * (`Each` rebuilds items), so the host data can flow in through an
     * atom instead of a module-level snapshot. */
    options$: Readable<SelectorOption<T>[]>;
    selectedValue$: Readable<T>;
    /** Dispatched when the user picks an option — a mutation handle, so it
     *  writes through the click's store ctx (`ctx.set(onSelect$, value)`);
     *  there is no module-level store to write through. */
    onSelect$: Mutation<[T], void>;
    label$?: Readable<string>;
    gap?: number;
    menuWidth?: number;
    triggerHeight?: number;
    /** Which way the menu opens from the trigger: `"right"` (default)
     * anchors the follower's top-RIGHT to the trigger's bottom-right, so
     * the card extends leftward (for right-aligned triggers); `"left"`
     * anchors top-LEFT to bottom-left, extending rightward (for
     * left-aligned triggers, whose leftward card would clip off-screen). */
    menuAlign?: "left" | "right";
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
    const menuAlign = opts.menuAlign ?? "right";
    const chipRadius = 999;
    const cardRadius = 14;

    const open$ = source(false);
    const link = createLayerLink();

    const label$: Readable<string> =
        opts.label$ ??
        derive((ctx) => {
            const v = ctx.get(opts.selectedValue$);
            const o = ctx.get(opts.options$).find((x) => x.value === v);
            return o ? o.label : "";
        });

    function TriggerPill(): Element {
        return PointerInteract()
            .behavior(HitTestBehavior.Opaque)
            .onClick(mutate((ctx) => ctx.set(open$, !ctx.get(open$))))
            .child(Container()
                .height(triggerHeight)
                .color(style.triggerBg ?? style.surface)
                .borderColor(style.divider)
                .borderWidth(style.triggerBg ? 0 : 1)
                .borderRadius(chipRadius)
                .children([
                    Row()
                        .mainAlignment(MainAxisAlignment.Start)
                        .crossAlignment(CrossAxisAlignment.Center)
                        .mainAxisSize(MainAxisSize.Min)
                        .children([
                            SizedBox()
                                .width(14)
                                .build(),
                            // `Flexible` (loose fit): the label shrink-wraps
                            // when it fits the pill and ellipsizes at the
                            // pill's true budget when it doesn't — the
                            // Flutter-idiomatic shape (tur #227).
                            Flexible()
                                .child(
                                    Text({ text: label$ })
                                        .fontSize(13)
                                        .color(style.triggerText ?? style.text)
                                        .maxLines(1)
                                        .overflow("ellipsis")
                                        .build(),
                                )
                                .build(),
                            SizedBox()
                                .width(8)
                                .build(),
                            Text({ text: derive((ctx) => (ctx.get(open$) ? "▲" : "▼")) })
                                .fontSize(10)
                                .color(style.triggerText ?? style.primary)
                                .build(),
                            SizedBox()
                                .width(14)
                                .build(),
                        ])
                        .build(),
                ])
                .build())
            .build();
    }

    function OptionRow(option: SelectorOption<T>): Element {
        const selected$ = derive(
            (ctx) => ctx.get(opts.selectedValue$) === option.value,
        );
        return PointerInteract()
            .behavior(HitTestBehavior.Opaque)
            .onClick(mutate((ctx) => {
                ctx.set(open$, false);
                if (ctx.get(opts.selectedValue$) !== option.value) {
                    ctx.set(opts.onSelect$, option.value);
                }
            }))
            .child(Container()
                .color(derive((ctx) =>
                    ctx.get(selected$) ? style.primarySoft : style.surface,
                ))
                .borderRadius(10)
                .padding(12)
                .children([
                    Row()
                        .mainAlignment(MainAxisAlignment.SpaceBetween)
                        .crossAlignment(CrossAxisAlignment.Center)
                        .children([
                            // `Flexible` (FlexFit.loose, tur #227) gives the
                            // label a finite width budget: non-flex Row
                            // children get an UNBOUNDED main axis (RenderFlex
                            // parity — in Flutter too), where an ellipsizing
                            // `Text` has no true budget at all.
                            Flexible()
                                .child(
                                    Text({ text: option.label })
                                        .fontSize(13)
                                        .color(derive((ctx) =>
                                            ctx.get(selected$)
                                                ? style.primary
                                                : style.text,
                                        ))
                                        .maxLines(1)
                                        .overflow("ellipsis")
                                        .build(),
                                )
                                .build(),
                            Condition({ condition: selected$ })
                                .child(() =>
                                    Text({ text: "✓" })
                                        .fontSize(13)
                                        .color(style.primary)
                                        .build())
                                .build(),
                        ])
                        .build(),
                ])
                .build())
            .build();
    }

    function MenuCard(): Element {
        return Container()
            .width(menuWidth)
            .color(style.surface)
            .borderColor(style.divider)
            .borderWidth(1)
            .borderRadius(cardRadius)
            .shadowColor(style.shadow)
            .shadowBlur(16)
            .shadowOffset([0, 6])
            .children([
                Container()
                    .padding(6)
                    .children([
                        Column()
                            .mainAlignment(MainAxisAlignment.Start)
                            .crossAlignment(CrossAxisAlignment.Stretch)
                            .mainAxisSize(MainAxisSize.Min)
                            .children([
                                // Reactive rows: `Each` rebuilds an item when
                                // the options atom changes (length or not),
                                // so the menu follows host data that flows
                                // in through an atom. A height-0 SizedBox
                                // fronts every row after the first.
                                Each({ items: opts.options$ })
                                    .itemBuilder((o: SelectorOption<T>, i: number) =>
                                        Column()
                                            .crossAlignment(CrossAxisAlignment.Stretch)
                                            .mainAxisSize(MainAxisSize.Min)
                                            .children([
                                                SizedBox()
                                                    .height(i === 0 ? 0 : 4)
                                                    .build(),
                                                OptionRow(o),
                                            ])
                                            .build())
                                    .build(),
                            ])
                            .build(),
                    ])
                    .build(),
            ])
            .build();
    }

    return {
        open$,
        SelectorTrigger: () =>
            CompositedTransformTarget({ link })
                .child(TriggerPill())
                .build(),
        // Dismiss backdrop: Condition-gated (Condition OUTSIDE the Positioned)
        // so it only exists while open — an always-mounted fill `Positioned`
        // would steal taps from the trigger below it even with empty content.
        // It is fully transparent (no dim/mask over the page); it only captures
        // an outside tap to close. `right/bottom:0` fills the host page Stack.
        SelectorScrim: () =>
            Condition({ condition: open$ })
                .child(() =>
                    Positioned()
                        .left(0)
                        .top(0)
                        .right(0)
                        .bottom(0)
                        .child(PointerInteract()
                            .behavior(HitTestBehavior.Opaque)
                            .onClick(mutate((ctx) => ctx.set(open$, false)))
                            .child(Container()
                                .color(Color.rgba(0, 0, 0, 0))
                                .build())
                            .build())
                        .build())
                .build(),
        // Menu: a DIRECT Stack child (always mounted) so the
        // CompositedTransformSubsystem tracks and repositions it; the
        // `Condition` gating open/close lives INSIDE so the follower stays
        // linked. Which corner pair anchors depends on `menuAlign`: the
        // default extends the card LEFTWARD from a right-aligned trigger
        // (stays on-screen); `"left"` extends it rightward from a
        // left-aligned trigger's left edge.
        SelectorMenu: () => {
            const follower = CompositedTransformFollower({ link })
                .targetAnchor(
                    menuAlign === "left" ? Alignment.BottomLeft : Alignment.BottomRight,
                )
                .followerAnchor(
                    menuAlign === "left" ? Alignment.TopLeft : Alignment.TopRight,
                )
                .targetOffset({ x: 0, y: gap })
                .child(Condition({ condition: open$ })
                    .child(() => MenuCard())
                    .build())
                .build();
            return follower;
        },
    };
}
