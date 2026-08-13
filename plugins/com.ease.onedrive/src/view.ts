// OneDrive setup view — the tur-rendered config form shown in the add-storage
// page when the user selects the "OneDrive" provider.
//
// Renders an alias input + a "Connect your account" button. Tapping the
// button calls `oauth.start("onedrive", alias)` from the unified `ease`
// host module (a fire-and-forget Rust→Kotlin upcall); the host fetches the
// authorize URL, stashes the alias, and opens the system browser. The
// `easem://oauth2redirect` callback in `MainActivity` completes the
// exchange and mints the storage row — there is no "save" step on the host
// side.
//
// This module is bundled to `view.js` (see `rspack.config.cjs`) and loaded
// by the plugin-storage view host in `EditStorage.kt`. It runs in an isolated
// view instance, separate from the headless backend instance that owns the
// `onedrive:*` RPC handlers.

import {
    Color, Column, Container, CrossAxisAlignment,
    HitTestBehavior, Input, MainAxisAlignment, MainAxisSize,
    PointerInteract, Row, SizedBox, Text, createTextEditingController,
    get, mutate, render, view, viewportSize$,
    type Readable,
} from "tur:std";
import { oauth, themes } from "ease";

const PROVIDER = "onedrive";

// Inherit the host app's Material 3 theme so the setup form matches the
// surrounding UI. `themes.color(name)` returns "#RRGGBBAA" (or "" if the
// host hasn't pushed a value yet — fall back to sensible defaults).
function themed(name: string, fallback: string): Color {
    const hex = themes.color(name);
    return Color.hex(hex.length > 0 ? hex : fallback);
}

const COLOR_PRIMARY = themed("primary", "#2E89B0");
const COLOR_CARD = themed("surface", "#FFFFFF");
const COLOR_TEXT = themed("onSurface", "#0F172A");
const COLOR_TEXT_MUTED = themed("onSurfaceVariant", "#64748B");
const COLOR_DIVIDER = themed("outlineVariant", "#E2E8F0");
const COLOR_WHITE = Color.hex("#FFFFFF");

// Single controller for the alias field. Read via `.text` at button-tap time.
const aliasController = createTextEditingController({});

function FieldLabel({ text }: { text: string }) {
    return Text({
        text,
        fontSize: 13,
        color: COLOR_TEXT_MUTED,
    });
}

function AliasField() {
    return Container({
        color: COLOR_CARD,
        borderColor: COLOR_DIVIDER,
        borderWidth: 1,
        borderRadius: 12,
        padding: 10,
        children: [
            Input({
                controller: aliasController,
                placeholder: "OneDrive",
                fontSize: 15,
                color: COLOR_TEXT,
                placeholderColor: COLOR_TEXT_MUTED,
                cursorColor: COLOR_PRIMARY,
            }),
        ],
    });
}

function ConnectButton() {
    return PointerInteract({
        behavior: HitTestBehavior.Opaque,
        onClick: mutate(() => {
            const alias: string = aliasController.text ?? "";
            oauth.start(PROVIDER, alias.length > 0 ? alias : null);
        }),
        child: Container({
            color: COLOR_PRIMARY,
            borderRadius: 24,
            padding: 14,
            children: [
                Text({
                    text: "连接你的账户",
                    fontSize: 15,
                    color: COLOR_WHITE,
                }),
            ],
        }),
    });
}

const rootView = view(() => {
    // NOTE: `@tur-ng/std`'s d.ts references `Derived` without importing it, so
    // `viewportSize$` degrades to `any`/`unknown` at the call site; cast to the
    // documented `{ width, height }` shape.
    const vp = get(viewportSize$ as unknown as Readable<{ width: number; height: number }>);
    return Container({
        color: COLOR_CARD,
        width: vp.width,
        height: vp.height,
        padding: 4,
        children: [
            Column({
                crossAlignment: CrossAxisAlignment.Stretch,
                mainAxisSize: MainAxisSize.Min,
                children: [
                    FieldLabel({ text: "服务器名称 (别名)" }),
                    SizedBox({ height: 6 }),
                    AliasField(),
                    SizedBox({ height: 16 }),
                    Row({
                        mainAlignment: MainAxisAlignment.Start,
                        crossAlignment: CrossAxisAlignment.Center,
                        mainAxisSize: MainAxisSize.Min,
                        children: [ConnectButton()],
                    }),
                ],
            }),
        ],
    });
});

render(rootView);
