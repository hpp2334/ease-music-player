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

import {
    Color, Condition, Column, Container, CrossAxisAlignment,
    HitTestBehavior, Input, MainAxisAlignment, MainAxisSize,
    PointerInteract, Row, SizedBox, Text, createTextEditingController,
    derive, launch, mutate, mount, source, view, viewportSize$,
    type Readable, type Store, type StoreCtx,
} from "tur:std";
import { db, rpc, context, themes } from "ease";

const PROVIDER = "webdav";

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
const COLOR_ERROR = themed("error", "#B3261E");
const COLOR_WHITE = Color.hex("#FFFFFF");

// --- mode + prefilled config ---------------------------------------------
//
// Hydration happens in `start({ store })` (before mount): reactive reads
// need a store since the multi-store model removed the free module-level
// `get`. These module-level `let`s are consumed only from inside the
// `view()` thunk and factories, which run at mount time — after hydration.

let storageId: string | null = null;
let isEdit = false;
let initialAnon = false;

// Placeholders replaced by `hydrate()` once the config is known; the
// placeholder controllers are never attached to any Input.
let aliasController = createTextEditingController({ initialText: "" });
let addrController = createTextEditingController({ initialText: "" });
let usernameController = createTextEditingController({ initialText: "" });
const passwordController = createTextEditingController({ initialText: "" });

function hydrate(store: Store): void {
    storageId = store.get(context.storageId$);
    isEdit = storageId !== null;

    let initialAlias = "";
    let initialAddr = "";
    let initialUsername = "";
    initialAnon = false;
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
    aliasController = createTextEditingController({ initialText: initialAlias });
    addrController = createTextEditingController({ initialText: initialAddr });
    usernameController = createTextEditingController({ initialText: initialUsername });
    // The `anonymous` source declaration is seeded `false` at eval time;
    // write the hydrated value into the store (before mount, so the first
    // render sees it). Writes are equality-gated, so false→false is a no-op.
    store.set(anonymous, initialAnon);
}

// Local reactive state (declarations — materialized into the instance store
// that `start({ store })` received).
const anonymous = source(false);
const busy = source(false);
const statusText = source("");
const statusIsError = source(false);

function setStatus(ctx: StoreCtx, text: string, isError: boolean): void {
    ctx.set(statusText, text);
    ctx.set(statusIsError, isError);
}

// --- widgets ---------------------------------------------------------------

function FieldLabel({ text }: { text: string }) {
    return Text({
        text,
        fontSize: 13,
        color: COLOR_TEXT_MUTED,
    });
}

function TextField(opts: {
    controller: ReturnType<typeof createTextEditingController>;
    placeholder: string;
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
                controller: opts.controller,
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

function ActionButton(opts: { label: string; primary: boolean; onClick: (ctx: StoreCtx) => void }) {
    return PointerInteract({
        behavior: HitTestBehavior.Opaque,
        onClick: mutate((ctx) => opts.onClick(ctx)),
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
    instance?: string;
    addr: string;
    alias?: string;
    username?: string;
    password?: string;
    isAnonymous?: boolean;
}

function collectArgs(ctx: StoreCtx): ConnectArgs {
    const isAnon = ctx.get(anonymous);
    return {
        ...(isEdit && storageId != null ? { instance: storageId } : {}),
        addr: (addrController.text ?? "").trim(),
        alias: (aliasController.text ?? "").trim(),
        username: isAnon ? "" : (usernameController.text ?? "").trim(),
        password: isAnon ? "" : (passwordController.text ?? ""),
        isAnonymous: isAnon,
    };
}

function validate(args: ConnectArgs): string | null {
    if (args.addr === "") return "服务器地址不能为空";
    if (args.alias === "") return "服务器名称 (别名) 不能为空";
    if (!args.isAnonymous && args.username === "") return "用户名不能为空";
    if (!args.isAnonymous && args.password === "" && !isEdit) return "密码不能为空";
    return null;
}

function runTest(ctx: StoreCtx): void {
    const args = collectArgs(ctx);
    const invalid = validate(args);
    if (invalid != null) {
        setStatus(ctx, invalid, true);
        return;
    }
    ctx.set(busy, true);
    setStatus(ctx, "测试中...", false);
    launch(function* () {
        try {
            const r = (yield rpc.call("webdav:test", args)) as { result: string };
            if (r.result === "SUCCESS") {
                setStatus(ctx, "测试成功", false);
            } else if (r.result === "UNAUTHORIZED") {
                setStatus(ctx, "测试错误：认证错误", true);
            } else if (r.result === "TIMEOUT") {
                setStatus(ctx, "测试错误：超时", true);
            } else {
                setStatus(ctx, "测试错误：其他错误", true);
            }
        } catch (e: any) {
            setStatus(ctx, `测试错误：${String(e?.message ?? e)}`, true);
        } finally {
            ctx.set(busy, false);
        }
    });
}

function save(ctx: StoreCtx): void {
    const args = collectArgs(ctx);
    const invalid = validate(args);
    if (invalid != null) {
        setStatus(ctx, invalid, true);
        return;
    }
    ctx.set(busy, true);
    setStatus(ctx, "", false);
    launch(function* () {
        try {
            yield rpc.call("webdav:connect", args);
            if (isEdit) {
                // The backend already rewrote the kv + notified the host;
                // show confirmation (create mode pops via the host upcall).
                setStatus(ctx, "已保存", false);
            }
        } catch (e: any) {
            setStatus(ctx, String(e?.message ?? e), true);
        } finally {
            ctx.set(busy, false);
        }
    });
}

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
                    TextField({ controller: aliasController, placeholder: "WebDAV" }),
                    SizedBox({ height: 16 }),
                    FieldLabel({ text: "服务器地址" }),
                    SizedBox({ height: 6 }),
                    TextField({ controller: addrController, placeholder: "https://example.com/dav" }),
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
                                    TextField({ controller: usernameController, placeholder: "" }),
                                    SizedBox({ height: 16 }),
                                    FieldLabel({ text: "密码" }),
                                    SizedBox({ height: 6 }),
                                    TextField({
                                        controller: passwordController,
                                        placeholder: isEdit ? "留空保持不变" : "",
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
                            ActionButton({ label: "测试", primary: false, onClick: (ctx) => runTest(ctx) }),
                            SizedBox({ width: 12 }),
                            ActionButton({ label: isEdit ? "保存" : "连接", primary: true, onClick: (ctx) => save(ctx) }),
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
// `createStore`). The root-tree lifecycle is engine-owned — `mount`
// replaces any existing root and module teardown clears it — so no cleanup
// is returned.
export function start({ store }: { store: Store }): void {
    hydrate(store);
    mount(rootView);
}
