// OneDrive storage view — the tur-rendered form shown both in the add-storage
// page (create mode) and when editing an existing OneDrive storage (edit mode).
//
// Mode is derived from `ease.context.storageId$`: `null` = create (no storage
// row yet), a real `plugin_storage_id` = edit. The view branches once at
// module load (the value is static for the instance lifetime).
//
// - Create: an alias input + a "Connect your account" button. Tapping it
//   calls `oauth.start("onedrive", alias)` (a fire-and-forget Rust→Kotlin
//   upcall); the host fetches the authorize URL, stashes the alias, and opens
//   the system browser. The `easem://oauth2redirect` callback in `MainActivity`
//   completes the exchange and mints the storage row — there is no "save"
//   step on the host side.
//
// - Edit: the alias input is prefilled from the plugin's `ease.db` config
//   (`storage:<instance>` = `{ alias, secretId }`); a Save button writes it
//   back via `db.singleSet` + `context.notifyChange()` (host reloads the
//   dashboard so the new alias shows). Removal is handled by the host's
//   top-bar trash icon (which routes to the backend's `onedrive:removeInstance`
//   — kv + secret wipe + host row delete), so no in-view disconnect button.
//
// This module is bundled to `view.js` (see `rspack.config.cjs`) and loaded by
// the plugin-storage view host in `EditStorage.kt`. It runs in an isolated
// view instance, separate from the headless backend instance that owns the
// `onedrive:*` RPC handlers.

import {
    Color, Column, Container, CrossAxisAlignment,
    HitTestBehavior, Input, MainAxisAlignment, MainAxisSize,
    PointerInteract, Row, SizedBox, Text, createTextEditingController,
    get, mutate, mount, view, viewportSize$,
    type Readable,
} from "tur:std";
import { db, oauth, context, themes } from "ease";

const PROVIDER = "onedrive";

// Inherit the host app's Material 3 theme so the form matches the surrounding
// UI. `themes.color(name)` returns "#RRGGBBAA" (or "" if the host hasn't
// pushed a value yet — fall back to sensible defaults).
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

// --- mode + prefilled config ---------------------------------------------

const storageId = get(context.storageId$);
const isEdit = storageId !== null;

// In edit mode, hydrate the alias + secretId from the plugin's kv config so
// the field is prefilled and Save preserves the existing secret reference.
let initialAlias = "";
let existingSecretId: number | null = null;
if (isEdit) {
    const raw = db.singleGet(`storage:${storageId}`);
    if (raw != null) {
        try {
            const cfg = JSON.parse(raw);
            initialAlias = cfg.alias ?? "";
            existingSecretId = cfg.secretId ?? null;
        } catch {
            /* corrupt config — start blank */
        }
    }
}

// Single controller for the alias field. Read via `.text` at button-tap time.
const aliasController = createTextEditingController({ initialText: initialAlias });

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

function SaveButton() {
    return PointerInteract({
        behavior: HitTestBehavior.Opaque,
        onClick: mutate(() => {
            if (!isEdit || storageId === null) return;
            const alias: string = aliasController.text ?? "";
            // Preserve the existing secretId; only the alias is editable here.
            db.singleSet(
                `storage:${storageId}`,
                JSON.stringify({ alias, secretId: existingSecretId }),
            );
            // Reload the host list so the dashboard picks up the new alias.
            context.notifyChange();
        }),
        child: Container({
            color: COLOR_PRIMARY,
            borderRadius: 24,
            padding: 14,
            children: [
                Text({
                    text: "保存",
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
    const actionRow = Row({
        mainAlignment: MainAxisAlignment.Start,
        crossAlignment: CrossAxisAlignment.Center,
        mainAxisSize: MainAxisSize.Min,
        children: [isEdit ? SaveButton() : ConnectButton()],
    });
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
                    actionRow,
                ],
            }),
        ],
    });
});

// Module lifecycle contract: mount inside `start()` (the engine runs the
// returned cleanup before the next load / at destroy). The root-tree
// lifecycle is engine-owned — `mount` replaces any existing root and module
// teardown clears it — so no cleanup is returned.
export function start(): void {
    mount(rootView);
}
