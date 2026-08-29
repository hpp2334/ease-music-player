// OneDrive storage view — the tur-rendered form shown both in the add-storage
// page (create mode) and when editing an existing OneDrive storage (edit mode).
//
// Mode is derived from `ease.context.storageId$`: `null` = create (no storage
// row yet), a real `plugin_storage_id` = edit. The view branches once at
// module load (the value is static for the instance lifetime).
//
// - Create: an alias input + a "Connect your account" button. Tapping it
//   mints a flow id (`oauth.new()`), stashes the alias in the plugin's own
//   KV keyed by it (`oauth:<id>`), and fires `oauth.start(oauthId)` (a
//   fire-and-forget Rust→Kotlin upcall); the host fetches the authorize URL
//   from the backend, stashes `(pluginId, oauthId)`, and opens the system
//   browser. The `easem://oauth2redirect` callback in `MainActivity`
//   completes the exchange — the backend's `oauth:exchange` handler consumes
//   the pending slot — and mints the storage row; there is no "save" step on
//   the host side, and the alias never crosses the host.
//
// - Edit: the alias input is prefilled from the plugin's `ease.db` config
//   (`storage:<instance>` = `{ alias, secretId }`); a Save button writes it
//   back via `db.singleSet` + `context.notifyChange()` (host reloads the
//   dashboard so the new alias shows). Removal is handled by the host's
//   top-bar trash icon (which routes to the backend's
//   `storage:removeInstance` — kv + secret wipe + host row delete), so no
//   in-view disconnect button.
//
// This module is bundled to `view.js` (see `rspack.config.cjs`) and loaded by
// the plugin-storage view host in `EditStorage.kt`. It runs in an isolated
// view instance, separate from the headless backend instance that owns the
// `storage:*` / `oauth:*` RPC handlers.

// TextEncoder/TextDecoder polyfill FIRST — npm deps may rely on them.
import "../../infra/string-polyfill";
import "../../infra/text-polyfill";
import {
    Color, Column, Condition, Container, CrossAxisAlignment,
    HitTestBehavior, Input, MainAxisAlignment, MainAxisSize,
    PointerInteract, Row, SizedBox, Text, createTextEditingController,
    derive, mutate, mount, source, view, viewportSize$,
    type Mutation, type PointerInteractEvent, type Readable,
    type Store, type StoreCtx, type TextController,
} from "tur:std";
import { db, oauth, context, themes } from "ease";
import { pendingKey, type PendingFlow } from "./oauth-pending";

// Inherit the host app's Material 3 theme so the form matches the
// surrounding UI. `themes.color(name)` throws on unknown names — views load
// long after the host pushes its theme, so a miss is always a bug (typo /
// outdated plugin) and failing fast beats silently rendering a fallback.
const COLOR_PRIMARY = Color.hex(themes.color("primary"));
const COLOR_CARD = Color.hex(themes.color("surface"));
const COLOR_TEXT = Color.hex(themes.color("onSurface"));
const COLOR_TEXT_MUTED = Color.hex(themes.color("onSurfaceVariant"));
const COLOR_DIVIDER = Color.hex(themes.color("outlineVariant"));
const COLOR_WHITE = Color.hex("#FFFFFF");

// --- mode + prefilled config ---------------------------------------------
//
// No module-level mutable state: everything reactive is a declaration
// (`source` / `derive`), materialized into the instance store that
// `start({ store })` receives. The `hydrate$` mutation (dispatched from
// `start` BEFORE mount) writes the prefilled alias controller + secret id
// into the sources; `Input` resolves a controller source at build time, so
// the hydrated instance is what gets attached — the seed below is a
// placeholder that never reaches an Input.

const isEdit$ = derive((ctx) => ctx.get(context.storageId$) !== null);
const aliasController$ = source(createTextEditingController({ initialText: "" }));
const existingSecretId$ = source<number | null>(null);

const hydrate$ = mutate((ctx: StoreCtx): void => {
    const storageId = ctx.get(context.storageId$);
    const isEdit = storageId !== null;

    // In edit mode, hydrate the alias + secretId from the plugin's kv config
    // so the field is prefilled and Save preserves the existing secret
    // reference.
    let initialAlias = "";
    let secretId: number | null = null;
    if (isEdit) {
        const raw = db.singleGet(`storage:${storageId}`);
        if (raw != null) {
            try {
                const cfg = JSON.parse(raw);
                initialAlias = cfg.alias ?? "";
                secretId = cfg.secretId ?? null;
            } catch {
                /* corrupt config — start blank */
            }
        }
    }
    ctx.set(aliasController$, createTextEditingController({ initialText: initialAlias }));
    ctx.set(existingSecretId$, secretId);
});

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
                // The engine resolves a controller READABLE at build time
                // (`editable_text/element.rs` falls back to `controller_atom`
                // when no plain controller was given); the published typings
                // only declare the plain form — hence the cast.
                controller: aliasController$ as TextController,
                placeholder: "OneDrive",
                fontSize: 15,
                color: COLOR_TEXT,
                placeholderColor: COLOR_TEXT_MUTED,
                cursorColor: COLOR_PRIMARY,
            }),
        ],
    });
}

const connect$ = mutate((ctx: StoreCtx, _ev: PointerInteractEvent): void => {
    // Business data (the alias) stays plugin-owned: stash it in our
    // KV keyed by a freshly minted flow id, then fire the flow. The
    // `oauth:exchange` handler in backend.ts consumes the slot.
    const alias: string = ctx.get(aliasController$).text ?? "";
    const oauthId = oauth.new();
    db.singleSet(
        pendingKey(oauthId),
        JSON.stringify({ alias: alias.length > 0 ? alias : null } satisfies PendingFlow),
    );
    oauth.start(oauthId);
});

function ConnectButton() {
    return PointerInteract({
        behavior: HitTestBehavior.Opaque,
        onClick: connect$,
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

const save$ = mutate((ctx: StoreCtx, _ev: PointerInteractEvent): void => {
    const storageId = ctx.get(context.storageId$);
    if (storageId === null) return;
    const alias: string = ctx.get(aliasController$).text ?? "";
    // Preserve the existing secretId; only the alias is editable here.
    db.singleSet(
        `storage:${storageId}`,
        JSON.stringify({ alias, secretId: ctx.get(existingSecretId$) }),
    );
    // Reload the host list so the dashboard picks up the new alias.
    context.notifyChange();
});

function SaveButton() {
    return PointerInteract({
        behavior: HitTestBehavior.Opaque,
        onClick: save$,
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
    // documented `{ width, height }` shape. The reads are `derive` closures so
    // the page tracks viewport changes (the thunk itself runs once at mount).
    const vp$ = viewportSize$ as unknown as Readable<{ width: number; height: number }>;
    const actionRow = Row({
        mainAlignment: MainAxisAlignment.Start,
        crossAlignment: CrossAxisAlignment.Center,
        mainAxisSize: MainAxisSize.Min,
        children: [
            Condition({
                condition: isEdit$,
                child: () => SaveButton(),
                elseChild: () => ConnectButton(),
            }),
        ],
    });
    return Container({
        color: COLOR_CARD,
        width: derive((ctx) => ctx.get(vp$).width),
        height: derive((ctx) => ctx.get(vp$).height),
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

// Module lifecycle contract: mount inside `start({ store })` (the engine
// runs the returned cleanup before the next load / at destroy; it hands us
// the instance-owned store — one per instance since tur #207, no
// `createStore`). Hydration is dispatched BEFORE mount: `Input` resolves
// its controller source at build time, so the prefilled alias controller is
// what gets attached. The root-tree lifecycle is engine-owned — `mount`
// replaces any existing root and module teardown clears it — so no cleanup
// is returned.
export function start({ store }: { store: Store }): void {
    store.set(hydrate$);
    mount(rootView);
}
