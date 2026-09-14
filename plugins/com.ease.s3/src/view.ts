// S3 storage view — the tur-rendered form shown both in the add-storage page
// (create mode) and when editing an existing S3 storage (edit mode).
//
// Mode is derived from `ease.context.storageId$`: `null` = create (no storage
// row yet), a real `plugin_storage_id` = edit. The view branches once at
// module load (the value is static for the instance lifetime).
//
// - Create: alias / endpoint / region / bucket / access key / secret key
//   fields plus Test + Save buttons. Save calls the plugin backend
//   (`s3:connect`) via `ease.rpc.call`; the backend persists the config +
//   secret and registers the host storage row (`ease.context.createStorage`),
//   whose upcall pops this page — there is no host-side "save" step.
//
// - Edit: fields prefilled from the plugin's `ease.db` config
//   (`storage:<instance>`); a blank secret key means "keep the stored one".
//   Save rewrites the config via `s3:connect` (+ `context.notifyChange` so
//   the dashboard shows the new alias). Removal is handled by the host's
//   top-bar trash icon.
//
// This module is bundled to `view.js` (see `rspack.config.cjs`) and loaded by
// the plugin-storage view host in `EditStorage.kt`. It runs in an isolated
// view instance, separate from the headless backend instance that owns the
// `s3:*` RPC handlers.

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
import { S3TestSig, S3ConnectSig } from "./rpc";
import type { S3ConnectArgs } from "./rpc";

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
const busy = source(false);
const statusText = source("");
const statusIsError = source(false);

const aliasController$ = source(createTextEditingController({ initialText: "" }));
const endpointController$ = source(createTextEditingController({ initialText: "" }));
const regionController$ = source(createTextEditingController({ initialText: "" }));
const bucketController$ = source(createTextEditingController({ initialText: "" }));
const accessKeyController$ = source(createTextEditingController({ initialText: "" }));
// Never replaced — a blank secret means "keep the stored one".
const secretKeyController$ = source(createTextEditingController({ initialText: "" }));

const hydrate$ = mutate((ctx: StoreCtx): void => {
    const storageId = ctx.get(context.storageId$);
    const isEdit = storageId !== null;

    let initialAlias = "";
    let initialEndpoint = "";
    let initialRegion = "us-east-1";
    let initialBucket = "";
    let initialAccessKey = "";
    if (isEdit) {
        const raw = db.singleGet(`storage:${storageId}`);
        if (raw != null) {
            try {
                const cfg = JSON.parse(raw);
                initialAlias = cfg.alias ?? "";
                initialEndpoint = cfg.endpoint ?? "";
                initialRegion = cfg.region ?? "us-east-1";
                initialBucket = cfg.bucket ?? "";
                initialAccessKey = cfg.accessKeyId ?? "";
            } catch {
                /* corrupt config — start blank */
            }
        }
    }
    ctx.set(aliasController$, createTextEditingController({ initialText: initialAlias }));
    ctx.set(endpointController$, createTextEditingController({ initialText: initialEndpoint }));
    ctx.set(regionController$, createTextEditingController({ initialText: initialRegion }));
    ctx.set(bucketController$, createTextEditingController({ initialText: initialBucket }));
    ctx.set(accessKeyController$, createTextEditingController({ initialText: initialAccessKey }));
    ctx.set(secretKeyController$, createTextEditingController({ initialText: "" }));
});

const setStatus$ = mutate(
    (ctx: StoreCtx, text: string, isError: boolean): void => {
        ctx.set(statusText, text);
        ctx.set(statusIsError, isError);
    },
);

// --- widgets ---------------------------------------------------------------

function FieldLabel({ text }: { text: string }) {
    return Text({ text })
        .fontSize(13)
        .color(COLOR_TEXT_MUTED)
        .build();
}

function TextField(opts: {
    controller: Readable<TextController>;
    placeholder: Val<string>;
    obscure?: boolean;
}) {
    const input = Input()
        // The builder typing accepts the controller as a plain
        // `TextController` or as its readable (`controller_atom`) — the
        // engine resolves a controller READABLE at build time
        // (`editable_text/element.rs`).
        .controller(opts.controller)
        .placeholder(opts.placeholder)
        .fontSize(15)
        .color(COLOR_TEXT)
        .placeholderColor(COLOR_TEXT_MUTED)
        .cursorColor(COLOR_PRIMARY);
    if (opts.obscure) input.obscureText(true);
    return Container()
        .color(COLOR_CARD)
        .borderColor(COLOR_DIVIDER)
        .borderWidth(1)
        .borderRadius(12)
        .padding(7)
        .children([input.build()])
        .build();
}

function ActionButton(opts: {
    label: Val<string>;
    primary: boolean;
    onClick$: Mutation<[PointerInteractEvent], void>;
}) {
    const button = Container()
        .borderRadius(24)
        .padding(11)
        .children([
            Text({ text: opts.label })
                .fontSize(15)
                .color(opts.primary ? COLOR_WHITE : COLOR_PRIMARY)
                .build(),
        ]);
    if (opts.primary) {
        button.color(COLOR_PRIMARY);
    } else {
        button.color(COLOR_CARD).borderColor(COLOR_PRIMARY).borderWidth(1);
    }
    return PointerInteract()
        .behavior(HitTestBehavior.Opaque)
        .onClick(opts.onClick$)
        .child(button.build())
        .build();
}

// --- actions ---------------------------------------------------------------

const collectArgs$ = mutate((ctx: StoreCtx): S3ConnectArgs => {
    const isEdit = ctx.get(isEdit$);
    const storageId = ctx.get(context.storageId$);
    return {
        ...(isEdit && storageId != null ? { storageId } : {}),
        endpoint: (ctx.get(endpointController$).text ?? "").trim(),
        region: (ctx.get(regionController$).text ?? "").trim() || "us-east-1",
        bucket: (ctx.get(bucketController$).text ?? "").trim(),
        alias: (ctx.get(aliasController$).text ?? "").trim(),
        accessKeyId: (ctx.get(accessKeyController$).text ?? "").trim(),
        secretAccessKey: ctx.get(secretKeyController$).text ?? "",
    };
});

function validate(args: S3ConnectArgs, isEdit: boolean): string | null {
    if (args.endpoint === "") return "端点地址 (Endpoint) 不能为空";
    if (args.alias === "") return "名称 (别名) 不能为空";
    if (args.bucket === "") return "存储桶 (Bucket) 不能为空";
    if (args.accessKeyId === "") return "Access Key ID 不能为空";
    if ((args.secretAccessKey ?? "") === "" && !isEdit) return "Secret Access Key 不能为空";
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
            const r = await rpc.call(S3TestSig, {
                storageId: args.storageId,
                endpoint: args.endpoint,
                region: args.region,
                bucket: args.bucket,
                accessKeyId: args.accessKeyId,
                secretAccessKey: args.secretAccessKey,
            });
            if (r.result === "SUCCESS") {
                ctx.set(setStatus$, "测试成功", false);
            } else if (r.result === "UNAUTHORIZED") {
                ctx.set(setStatus$, "测试错误：认证或签名错误", true);
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
            await rpc.call(S3ConnectSig, args);
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

const statusVisible$ = derive((ctx) => ctx.get(statusText) !== "");
const statusIsError$ = derive((ctx) => ctx.get(statusIsError));
const statusColor$ = derive((ctx) => (ctx.get(statusIsError) ? COLOR_ERROR : COLOR_TEXT_MUTED));

function Field(opts: { label: string; controller: Readable<TextController>; placeholder: Val<string>; obscure?: boolean }) {
    const children = [
        FieldLabel({ text: opts.label }),
        SizedBox().height(2).build(),
        TextField({ controller: opts.controller, placeholder: opts.placeholder, obscure: opts.obscure }),
    ];
    return Column()
        .crossAlignment(CrossAxisAlignment.Stretch)
        .mainAxisSize(MainAxisSize.Min)
        .children(children)
        .build();
}

/** Vertical gap between form rows. Kept small: the host embeds this view in
 *  a fixed-height (480 dp) TurView (`EditStorage.kt`) with no scrolling —
 *  six fields + buttons must fit, so compactness is a hard requirement. */
function FieldGap() {
    return SizedBox().height(7).build();
}

const rootView = view(() => {
    // NOTE: `@tur-ng/std`'s d.ts references `Derived` without importing it, so
    // `viewportSize$` degrades to `any`/`unknown` at the call site; cast to the
    // documented `{ width, height }` shape. The reads are `derive` closures so
    // the page tracks viewport changes (the thunk itself runs once at mount).
    const vp$ = viewportSize$ as unknown as Readable<{ width: number; height: number }>;

    return Container()
        .color(COLOR_CARD)
        .width(derive((ctx) => ctx.get(vp$).width))
        .height(derive((ctx) => ctx.get(vp$).height))
        .padding(2)
        .children([
            Column()
                .crossAlignment(CrossAxisAlignment.Stretch)
                .mainAxisSize(MainAxisSize.Min)
                .children([
                    Field({ label: "名称 (别名)", controller: aliasController$, placeholder: "S3" }),
                    FieldGap(),
                    Field({
                        label: "端点地址 (Endpoint)",
                        controller: endpointController$,
                        placeholder: "https://s3.amazonaws.com",
                    }),
                    FieldGap(),
                    Field({
                        label: "区域 (Region)",
                        controller: regionController$,
                        placeholder: "us-east-1",
                    }),
                    FieldGap(),
                    Field({
                        label: "存储桶 (Bucket)",
                        controller: bucketController$,
                        placeholder: "music",
                    }),
                    FieldGap(),
                    Field({
                        label: "Access Key ID",
                        controller: accessKeyController$,
                        placeholder: "",
                    }),
                    FieldGap(),
                    Field({
                        label: "Secret Access Key",
                        controller: secretKeyController$,
                        placeholder: derive((ctx) =>
                            ctx.get(isEdit$) ? "留空保持不变" : "",
                        ),
                        obscure: true,
                    }),
                    FieldGap(),
                    Row()
                        .mainAlignment(MainAxisAlignment.Start)
                        .crossAlignment(CrossAxisAlignment.Center)
                        .mainAxisSize(MainAxisSize.Min)
                        .children([
                            ActionButton({ label: "测试", primary: false, onClick$: runTest$ }),
                            SizedBox().width(12).build(),
                            ActionButton({
                                label: derive((ctx) => (ctx.get(isEdit$) ? "保存" : "连接")),
                                primary: true,
                                onClick$: save$,
                            }),
                        ])
                        .build(),
                    SizedBox().height(8).build(),
                    Condition({ condition: statusVisible$ })
                        .child(() => Text({ text: statusText })
                            .fontSize(13)
                            .color(statusColor$)
                            .build())
                        .build(),
                ])
                .build(),
        ])
        .build();
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
