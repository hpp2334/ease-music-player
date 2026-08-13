// Tur Test plugin — `followerAnchor` repro.
//
// Two independent target/follower pairs (each on its own LayerLink):
//
//   A — CONTROL: always-visible follower.
//        targetAnchor = CenterRight, followerAnchor = CenterRight.
//        If followerAnchor is honored, the RED box extends LEFT of target A.
//
//   B — CONDITION-GATED: the follower is always mounted, but its child is a
//        `Condition(openB)` (the exact shape used by the Play Counts dropdown).
//        targetAnchor = CenterRight, followerAnchor = CenterRight.
//        Tap target B to toggle openB. When open, the BLUE box should extend
//        LEFT of target B. If the bug is present, the BLUE box extends RIGHT
//        (followerAnchor treated as TopLeft) exactly like the Play Counts menu.
//
// Comparing A and B isolates whether the issue is `followerAnchor` itself or
// the Condition-gated content path.

import {
    render,
    view,
    source,
    get,
    set,
    mutate,
    derive,
    Stack,
    Container,
    Column,
    Row,
    SizedBox,
    Text,
    Color,
    Alignment,
    MainAxisSize,
    CrossAxisAlignment,
    MainAxisAlignment,
    Condition,
    PointerInteract,
    HitTestBehavior,
    CompositedTransformTarget,
    CompositedTransformFollower,
    createLayerLink,
} from "tur:std";

const BG = Color.hex("#F8FAFC");
const INK = Color.hex("#0F172A");
const MUTED = Color.hex("#64748B");
const GREEN = Color.hex("#16A34A");
const GREEN_EDGE = Color.hex("#15803D");
const RED = Color.hex("#EF4444");
const BLUE = Color.hex("#2563EB");
const WHITE = Color.hex("#FFFFFF");

const TARGET = 120;
const FW = 170;
const FH = 64;

const linkA = createLayerLink();
const linkB = createLayerLink();
const openB$ = source(true);

function TargetBox(label: string): ReturnType<typeof Row> {
    return Container({
        width: TARGET,
        height: TARGET,
        color: GREEN,
        borderColor: GREEN_EDGE,
        borderWidth: 2,
        borderRadius: 14,
        children: [
            Row({
                mainAlignment: MainAxisAlignment.Center,
                crossAlignment: CrossAxisAlignment.Center,
                mainAxisSize: MainAxisSize.Max,
                children: [
                    Text({ text: label, fontSize: 15, color: WHITE }),
                ],
            }),
        ],
    });
}

function FollowerBox(color: Color, label: string): ReturnType<typeof Row> {
    return Container({
        width: FW,
        height: FH,
        color: color,
        borderRadius: 10,
        children: [
            Row({
                mainAlignment: MainAxisAlignment.Center,
                crossAlignment: CrossAxisAlignment.Center,
                mainAxisSize: MainAxisSize.Max,
                children: [
                    Text({ text: label, fontSize: 13, color: WHITE }),
                ],
            }),
        ],
    });
}

function Legend() {
    return Column({
        crossAlignment: CrossAxisAlignment.Start,
        mainAxisSize: MainAxisSize.Min,
        children: [
            Text({ text: "followerAnchor repro", fontSize: 20, color: INK }),
            SizedBox({ height: 12 }),
            Text({
                text: "A = always-visible follower (control)",
                fontSize: 12,
                color: MUTED,
            }),
            Text({
                text: "B = Condition-gated follower (tap B to toggle)",
                fontSize: 12,
                color: MUTED,
            }),
            SizedBox({ height: 8 }),
            Text({
                text: "both: targetAnchor=CenterRight, followerAnchor=CenterRight",
                fontSize: 12,
                color: MUTED,
            }),
            SizedBox({ height: 12 }),
            Text({
                text: "Expected: red AND blue extend LEFT of their target.",
                fontSize: 13,
                color: INK,
            }),
            Text({
                text: "Bug: a box extends RIGHT (followerAnchor ignored).",
                fontSize: 13,
                color: RED,
            }),
        ],
    });
}

export const rootView = view(() =>
    Stack({
        children: [
            // Base content: legend + two right-aligned targets so the followers
            // (which extend left when followerAnchor is honored) stay on-screen.
            Container({
                color: BG,
                padding: 22,
                children: [
                    Column({
                        crossAlignment: CrossAxisAlignment.Stretch,
                        mainAxisSize: MainAxisSize.Min,
                        children: [
                            Legend(),
                            SizedBox({ height: 70 }),
                            Row({
                                mainAlignment: MainAxisAlignment.End,
                                crossAlignment: CrossAxisAlignment.Center,
                                children: [
                                    CompositedTransformTarget({
                                        link: linkA,
                                        child: TargetBox("A"),
                                    }),
                                ],
                            }),
                            SizedBox({ height: 150 }),
                            Row({
                                mainAlignment: MainAxisAlignment.End,
                                crossAlignment: CrossAxisAlignment.Center,
                                children: [
                                    CompositedTransformTarget({
                                        link: linkB,
                                        child: PointerInteract({
                                            behavior: HitTestBehavior.Opaque,
                                            onClick: mutate((ctx) =>
                                                ctx.set(openB$, !ctx.get(openB$)),
                                            ),
                                            child: TargetBox("B"),
                                        }),
                                    }),
                                ],
                            }),
                        ],
                    }),
                ],
            }),
            // Follower A — always visible. Should extend LEFT of target A.
            CompositedTransformFollower({
                link: linkA,
                targetAnchor: Alignment.CenterRight,
                followerAnchor: Alignment.CenterRight,
                child: FollowerBox(RED, "A: CenterRight"),
            }),
            // Follower B — Condition-gated (mirrors the Play Counts dropdown).
            // Always mounted; the Condition lives INSIDE the follower so it
            // stays linked. Should extend LEFT of target B when open.
            CompositedTransformFollower({
                link: linkB,
                targetAnchor: Alignment.CenterRight,
                followerAnchor: Alignment.CenterRight,
                child: Condition({
                    condition: openB$,
                    child: () => FollowerBox(BLUE, "B: CenterRight"),
                }),
            }),
        ],
    }),
);

render(rootView);
