// WebDAV storage view — the tur-rendered form shown both in the add-storage
// page (create mode) and when editing an existing WebDAV storage (edit mode).
//
// Mode is derived from `ease.context.storageId$`: `null` = create (no storage
// row yet), a real `plugin_storage_id` = edit. The view branches once at
// module load (the value is static for the instance lifetime).
//
// - Create: alias / address / anonymous / username / password fields plus
//   Test + Save buttons. Save calls the plugin backend (`webdav:connect`) via
//   `ease.rpc.call`; the backend persists the config + secret and registers
//   the host storage row (`ease.context.createStorage`), whose upcall pops
//   this page — there is no host-side "save" step.
//
// - Edit: fields prefilled from the plugin's `ease.db` config
//   (`storage:<instance>`); a blank password means "keep the stored one".
//   Save rewrites the config via `webdav:connect` (+ `context.notifyChange`
//   so the dashboard shows the new alias). Removal is handled by the host's
//   top-bar trash icon.
//
// This module is bundled to `view.js` (see `rspack.config.cjs`) and loaded by
// the plugin-storage view host in `EditStorage.kt`. It runs in an isolated
// view instance, separate from the headless backend instance that owns the
// `webdav:*` RPC handlers.

// TextEncoder/TextDecoder polyfill FIRST — npm deps may rely on them.
import "../../infra/string-polyfill";
import "../../infra/text-polyfill";
import {
    Color, Condition, Column, Container, CrossAxisAlignment,
    HitTestBehavior, Input, MainAxisAlignment, MainAxisSize,
    PointerInteract, Row, SizedBox, Text, createTextEditingController,
    derive, mutate, mount, source, view, viewportSize$,
    type Mutation, type PointerInteractEvent, type Readable,
    type Store, type StoreCtx, type TextController, type Val,
} from "tur:std";
import { db, rpc, context, themes } from "ease";

const PROVIDER = "webdav";

// Inherit the host app's Material 3 theme so the form matches the
// surrounding UI. `themes.color(name)` throws on unknown names — views load
// long after the host pushes its theme, so a miss is always a bug (typo /
// outdated plugin) and failing fast beats silently rendering a fallback.
const COLOR_PRIMARY = Color.hex(themes.color("primary"));
const COLOR_CARD = Color.hex(themes.color("surface"));
const COLOR_TEXT = Color.hex(themes.color("onSurface"));
const COLOR_TEXT_MUTED = Color.hex(themes.color("onSurfaceVariant"));
const COLOR_DIVIDER = Color.hex(themes.color("outlineVariant"));
const COLOR_ERROR = Color.hex(themes.color("error"));
const COLOR_WHITE = Color.hex("#FFFFFF");

// --- mode + prefilled config ---------------------------------------------
//
// No module-level mutable state: everything reactive is a declaration
// (`source` / `derive`), materialized into the instance store that
// `start({ store })` receives. The `hydrate$` mutation (dispatched from
// `start` BEFORE mount) writes the prefilled controllers into the controller
// sources; `Input` resolves a controller source at build time, so the
// hydrated instances are what get attached — the seeds below are
// placeholders that never reach an Input.

const isEdit$ = derive((ctx) => ctx.get(context.storageId$) !== null);

// Local reactive state (declarations — materialized into the instance store
// that `start({ store })` received).
const anonymous = source(false);
const busy = source(false);
const statusText = source("");
const statusIsError = source(false);

const aliasController$ = source(createTextEditingController({ initialText: "" }));
const addrController$ = source(createTextEditingController({ initialText: "" }));
const usernameController$ = source(createTextEditingController({ initialText: "" }));
// Never replaced — a blank password means "keep the stored one".
const passwordController$ = source(createTextEditingController({ initialText: "" }));

const hydrate$ = mutate((ctx: StoreCtx): void => {
    const storageId = ctx.get(context.storageId$);
    const isEdit = storageId !== null;

    let initialAlias = "";
    let initialAddr = "";
    let initialUsername = "";
    let initialAnon = false;
    if (isEdit) {
        const raw = db.singleGet(`storage:${storageId}`);
        if (raw != null) {
            try {
                const cfg = JSON.parse(raw);
                initialAlias = cfg.alias ?? "";
                initialAddr = cfg.addr ?? "";
                initialUsername = cfg.username ?? "";
                initialAnon = !!cfg.isAnonymous;
            } catch {
                /* corrupt config — start blank */
            }
        }
    }
    ctx.set(aliasController$, createTextEditingController({ initialText: initialAlias }));
    ctx.set(addrController$, createTextEditingController({ initialText: initialAddr }));
    ctx.set(usernameController$, createTextEditingController({ initialText: initialUsername }));
    // The `anonymous` source declaration is seeded `false` at eval time;
    // write the hydrated value into the store (before mount, so the first
    // render sees it). Writes are equality-gated, so false→false is a no-op.
    ctx.set(anonymous, initialAnon);
});

const setStatus$ = mutate(
    (ctx: StoreCtx, text: string, isError: boolean): void => {
        ctx.set(statusText, text);
        ctx.set(statusIsError, isError);
    },
);

// --- widgets ---------------------------------------------------------------

function FieldLabel({ text }: { text: string }) {
    return Text({
        text,
        fontSize: 13,
        color: COLOR_TEXT_MUTED,
    });
}

function TextField(opts: {
    controller: Readable<TextController>;
    placeholder: Val<string>;
    obscure?: boolean;
}) {
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
                controller: opts.controller as TextController,
                placeholder: opts.placeholder,
                fontSize: 15,
                color: COLOR_TEXT,
                placeholderColor: COLOR_TEXT_MUTED,
                cursorColor: COLOR_PRIMARY,
                ...(opts.obscure ? { obscureText: true } : {}),
            }),
        ],
    });
}

/** Tappable "anonymous sign-in" toggle row (a bordered box fills when on).
 *  The box / label colors are reactive `Val`s so they repaint without a
 *  root-view rebuild. */
function AnonymousToggle() {
    return PointerInteract({
        behavior: HitTestBehavior.Opaque,
        onClick: mutate((ctx) => {
            ctx.set(anonymous, !ctx.get(anonymous));
        }),
        child: Row({
            mainAlignment: MainAxisAlignment.Start,
            crossAlignment: CrossAxisAlignment.Center,
            mainAxisSize: MainAxisSize.Min,
            children: [
                Container({
                    width: 18,
                    height: 18,
                    borderRadius: 5,
                    borderWidth: 1,
                    borderColor: checkBorder$,
                    color: checkFill$,
                }),
                SizedBox({ width: 8 }),
                Text({
                    text: "匿名登录",
                    fontSize: 14,
                    color: checkText$,
                }),
            ],
        }),
    });
}

function ActionButton(opts: {
    label: Val<string>;
    primary: boolean;
    onClick$: Mutation<[PointerInteractEvent], void>;
}) {
    return PointerInteract({
        behavior: HitTestBehavior.Opaque,
        onClick: opts.onClick$,
        child: Container(
            opts.primary
                ? {
                      color: COLOR_PRIMARY,
                      borderRadius: 24,
                      padding: 14,
                      children: [
                          Text({
                              text: opts.label,
                              fontSize: 15,
                              color: COLOR_WHITE,
                          }),
                      ],
                  }
                : {
                      color: COLOR_CARD,
                      borderColor: COLOR_PRIMARY,
                      borderWidth: 1,
                      borderRadius: 24,
                      padding: 14,
                      children: [
                          Text({
                              text: opts.label,
                              fontSize: 15,
                              color: COLOR_PRIMARY,
                          }),
                      ],
                  },
        ),
    });
}

// --- actions ---------------------------------------------------------------

interface ConnectArgs {
    storageId?: string;
    addr: string;
    alias?: string;
    username?: string;
    password?: string;
    isAnonymous?: boolean;
}

const collectArgs$ = mutate((ctx: StoreCtx): ConnectArgs => {
    const isAnon = ctx.get(anonymous);
    const isEdit = ctx.get(isEdit$);
    const storageId = ctx.get(context.storageId$);
    return {
        ...(isEdit && storageId != null ? { storageId } : {}),
        addr: (ctx.get(addrController$).text ?? "").trim(),
        alias: (ctx.get(aliasController$).text ?? "").trim(),
        username: isAnon ? "" : (ctx.get(usernameController$).text ?? "").trim(),
        password: isAnon ? "" : (ctx.get(passwordController$).text ?? ""),
        isAnonymous: isAnon,
    };
});

function validate(args: ConnectArgs, isEdit: boolean): string | null {
    if (args.addr === "") return "服务器地址不能为空";
    if (args.alias === "") return "服务器名称 (别名) 不能为空";
    if (!args.isAnonymous && args.username === "") return "用户名不能为空";
    if (!args.isAnonymous && args.password === "" && !isEdit) return "密码不能为空";
    return null;
}

const runTest$ = mutate((ctx: StoreCtx, _ev: PointerInteractEvent): void => {
    const args = ctx.set(collectArgs$);
    const invalid = validate(args, ctx.get(isEdit$));
    if (invalid != null) {
        ctx.set(setStatus$, invalid, true);
        return;
    }
    ctx.set(busy, true);
    ctx.set(setStatus$, "测试中...", false);
    // boa runs native async functions; since tur #212 async composition is
    // plain `await` (the `launch` generator driver is gone).
    void (async () => {
        try {
            const r = (await rpc.call("webdav:test", args)) as { result: string };
            if (r.result === "SUCCESS") {
                ctx.set(setStatus$, "测试成功", false);
            } else if (r.result === "UNAUTHORIZED") {
                ctx.set(setStatus$, "测试错误：认证错误", true);
            } else if (r.result === "TIMEOUT") {
                ctx.set(setStatus$, "测试错误：超时", true);
            } else {
                ctx.set(setStatus$, "测试错误：其他错误", true);
            }
        } catch (e: any) {
            ctx.set(setStatus$, `测试错误：${String(e?.message ?? e)}`, true);
        } finally {
            ctx.set(busy, false);
        }
    })();
});

const save$ = mutate((ctx: StoreCtx, _ev: PointerInteractEvent): void => {
    const args = ctx.set(collectArgs$);
    const invalid = validate(args, ctx.get(isEdit$));
    if (invalid != null) {
        ctx.set(setStatus$, invalid, true);
        return;
    }
    ctx.set(busy, true);
    ctx.set(setStatus$, "", false);
    void (async () => {
        try {
            await rpc.call("webdav:connect", args);
            if (ctx.get(isEdit$)) {
                // The backend already rewrote the kv + notified the host;
                // show confirmation (create mode pops via the host upcall).
                ctx.set(setStatus$, "已保存", false);
            }
        } catch (e: any) {
            ctx.set(setStatus$, String(e?.message ?? e), true);
        } finally {
            ctx.set(busy, false);
        }
    })();
});

// --- root ------------------------------------------------------------------

// NOTE: `view(fn)` builds ONCE — the thunk is not reactive. All dynamic UI
// therefore goes through reactive props: `Val<T>` positions accept Readables
// (`Source` / `Derived`) and re-render when they change; `Condition` swaps
// its child. Reads inside `derive` closures go through the store ctx.

const anonChecked$ = derive((ctx) => ctx.get(anonymous));
const showCreds$ = derive((ctx) => !ctx.get(anonymous));
const statusVisible$ = derive((ctx) => ctx.get(statusText) !== "");
const statusIsError$ = derive((ctx) => ctx.get(statusIsError));
const checkFill$ = derive((ctx) => (ctx.get(anonymous) ? COLOR_PRIMARY : COLOR_CARD));
const checkBorder$ = derive((ctx) => (ctx.get(anonymous) ? COLOR_PRIMARY : COLOR_DIVIDER));
const checkText$ = derive((ctx) => (ctx.get(anonymous) ? COLOR_TEXT : COLOR_TEXT_MUTED));
const statusColor$ = derive((ctx) => (ctx.get(statusIsError) ? COLOR_ERROR : COLOR_TEXT_MUTED));

const rootView = view(() => {
    // NOTE: `@tur-ng/std`'s d.ts references `Derived` without importing it, so
    // `viewportSize$` degrades to `any`/`unknown` at the call site; cast to the
    // documented `{ width, height }` shape. The reads are `derive` closures so
    // the page tracks viewport changes (the thunk itself runs once at mount).
    const vp$ = viewportSize$ as unknown as Readable<{ width: number; height: number }>;

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
                    TextField({ controller: aliasController$, placeholder: "WebDAV" }),
                    SizedBox({ height: 16 }),
                    FieldLabel({ text: "服务器地址" }),
                    SizedBox({ height: 6 }),
                    TextField({ controller: addrController$, placeholder: "https://example.com/dav" }),
                    SizedBox({ height: 16 }),
                    AnonymousToggle(),
                    SizedBox({ height: 16 }),
                    Condition({
                        condition: showCreds$,
                        child: () =>
                            Column({
                                crossAlignment: CrossAxisAlignment.Stretch,
                                mainAxisSize: MainAxisSize.Min,
                                children: [
                                    FieldLabel({ text: "用户名" }),
                                    SizedBox({ height: 6 }),
                                    TextField({ controller: usernameController$, placeholder: "" }),
                                    SizedBox({ height: 16 }),
                                    FieldLabel({ text: "密码" }),
                                    SizedBox({ height: 6 }),
                                    TextField({
                                        controller: passwordController$,
                                        placeholder: derive((ctx) =>
                                            ctx.get(isEdit$) ? "留空保持不变" : "",
                                        ),
                                        obscure: true,
                                    }),
                                    SizedBox({ height: 16 }),
                                ],
                            }),
                    }),
                    Row({
                        mainAlignment: MainAxisAlignment.Start,
                        crossAlignment: CrossAxisAlignment.Center,
                        mainAxisSize: MainAxisSize.Min,
                        children: [
                            ActionButton({ label: "测试", primary: false, onClick$: runTest$ }),
                            SizedBox({ width: 12 }),
                            ActionButton({
                                label: derive((ctx) => (ctx.get(isEdit$) ? "保存" : "连接")),
                                primary: true,
                                onClick$: save$,
                            }),
                        ],
                    }),
                    SizedBox({ height: 10 }),
                    Condition({
                        condition: statusVisible$,
                        child: () => Text({
                            text: statusText,
                            fontSize: 13,
                            color: statusColor$,
                        }),
                    }),
                ],
            }),
        ],
    });
});

// Module lifecycle contract: mount inside `start({ store })` (the engine
// runs the returned cleanup before the next load / at destroy; it hands us
// the instance-owned store — one per instance since tur #207, no
// `createStore`). Hydration is dispatched BEFORE mount: `Input` resolves
// its controller source at build time, so the prefilled controllers are
// what get attached. The root-tree lifecycle is engine-owned — `mount`
// replaces any existing root and module teardown clears it — so no cleanup
// is returned.
export function start({ store }: { store: Store }): void {
    store.set(hydrate$);
    mount(rootView);
}
