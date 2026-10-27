# Plugin porting guide — the rut epoch (apiVersion 2)

Plugins are **rut sources** since apiVersion 2. There is no JS, no npm, no
rspack — the engine compiles the sources; `pnpm test` (the
`ease-plugins-check` suite) compile+boot-checks every plugin host-side.

## Manifest (v2)

```json
{
  "id": "com.ease.webdav",
  "name": { "en-US": "WebDAV", "zh-CN": "WebDAV" },
  "author": "ease",
  "version": "0.1.0",
  "apiVersion": 2,
  "backend": "backend.rut",
  "events": ["music:play"],
  "contributions": {
    "storages":  [{ "id": "webdav", "title": "…", "desc": "…", "view": "view.rut" }],
    "dashboard": [{ "id": "main", "title": "…", "view": "view.rut" }],
    "lyricParsers": [{ "id": "lrc", "title": "…", "extensions": ["lrc"] }]
  }
}
```

Entry files are `backend.rut` / `view.rut` at the zip root (packaging zips
the manifest + every root-level `.rut` + referenced icons).

## The RPC contract (backends)

Each contract op is its own **entry fn** answering the reply envelope
string. Register the mapping at `start` through the bus rows; the host
stashes the request and calls the entry with the context token:

```rut
use tur_host::ease::rpc::{ register, subscribe_event, take_request };

struct Ctx { /* module state */ }

entry fn start() -> opaque {
    register("storage:list", "op_storage_list");
    register("storage:get", "op_storage_get");
    register("storage:get-chunk", "op_storage_get_chunk");
    register("storage:release", "op_storage_release");
    register("lyric:parse", "op_lyric_parse");
    register("oauth:url", "op_oauth_url");
    register("oauth:exchange", "op_oauth_exchange");
    register("storage:removeInstance", "op_storage_remove_instance");
    register("webdav:test", "op_webdav_test");   // view ops registered too
    subscribe_event("music:play", "op_on_music_play");
    return opaque(Ctx { ... });
}

entry fn op_storage_list(cx: opaque) -> str {
    let c = opaque::downcast<Ctx>(cx);
    if (c == nil) { panic("no ctx"); }
    let req = take_request();          // {"op":…,"args":{pluginId, storageId, dir}}
    ...                                // serve it
    return f"{{\"ok\":true,\"result\":{result_json}}}";
}
```

- The envelope is exactly `{"ok":true,"result":…}` / `{"ok":false,"error":"…"}`.
  Error messages beginning `UNAUTHORIZED` / `TIMEOUT` map to typed host UI
  states — prefix your auth/timeouts with those.
- `take_request()` drains the pending payload ("" = none). Events use the
  same stash: read `args.payload` from it.
- The context token: `start() -> opaque` boxes the module record
  (`opaque(Ctx{...})`); entries receive `cx: opaque` and
  `opaque::downcast<Ctx>(cx)` (nil-check + `panic` on mismatch).
- Contract literals (the host calls these exact strings):
  - storage backends: `storage:list`, `storage:get` (opener →
    `{sid, meta:{totalLength?, name?, contentType?, dataOffset?}}`),
    `storage:get-chunk` (`{sid, offset, len}` → `{data: <base64>,
    eof: bool}` — the final chunk carries data AND eof), `storage:release`
    (`{sid}`), plus `oauth:url` / `oauth:exchange` /
    `storage:removeInstance` for OAuth providers.
  - lyric parsers: `lyric:parse` (`{pluginId, parserId, fileName, size,
    contentBase64}` → `lines: [{timeMs, durationMs?, text}], metadata?` —
    answer `nil`-ish result (result: null) when the format isn't yours).
  - view ops (plugin-private names, e.g. `webdav:test`) are registered on
    the same bus; views reach them via `ease.rpc.call`.
- Keep per-session state (the `storage:get` session map) in the module's
  state record. `storage:release` must drop the session.

## The host surface (`tur_host::ease::*` rows)

All rows are typed; composites ride JSON strings; binary is `bytes`.

```rut
use tur_host::ease::db::{ single_get, single_set, single_delete, multi_append, multi_get_all, multi_count, list_keys };
use tur_host::ease::secret::{ get as secret_get, put as secret_put, remove as secret_remove };
use tur_host::ease::oauth::{ begin, new };   // `begin` fires the browser flow (the engine's `start` leaf is unspellable from a module exporting `entry fn start`)
use tur_host::ease::themes::{ color, is_dark };
use tur_host::ease::context::{ notify_change, create_storage, remove_storage, storage_id };
use tur_host::ease::library::{ playlists, covers };
use tur_host::ease::rpc::call;                       // view → its own backend
use tur_host::ease::crypto::{ random_uuid, md5_hex, md5_base64, base64_encode, base64_decode, sha256_hex, hmac_sha256_hex };
use tur_host::ease::xml::to_json;                    // bytes -> ?str (JSON tree)
use tur_host::ease::net::{ request, finish, status, status_text, headers, body, error };
use tur_host::ease::time::{ now_unix_ms, format_http_date, format_amz_date, parse_http_date };
```

- `db`: `single_get(key) -> ?str`; `single_set(key, value)`; composites
  take/return JSON (`single_get_multi(keysJson) -> [{key,value}]`,
  `multi_get_all(key) -> [value]`, `multi_count(key) -> u64`,
  `list_keys(prefix) -> [{key,kind}]`).
- `secret`: `get(id: i64) -> ?str`, `put(secret: str) -> i64`, `remove(id)`.
- `oauth`: `new() -> flowId`; `start(flowId)` fires the browser flow; the
  redirect lands in `oauth:exchange` on the backend.
- `net` (full HTTP — upstream's net rows carry no headers/bodies, ours do):
  ```rut
  let opts = f"{{\"url\":\"{url}\",\"method\":\"PROPFIND\",\"headers\":{{\"Depth\":\"1\",\"Authorization\":\"Basic {auth}\"}},\"body\":{{\"text\":{body_json}}}}}";
  let t = request(opts);
  let data = await finish(t);
  let code = status(t);            // u64, 0 = no response
  let hdrs = headers(t);           // JSON {name: value} (lowercased names)
  let err = error(t);              // "" on success
  ```
  Response body is raw `bytes`; decode with `decode_utf8(data)` from
  `tur_host::clipboard` (or base64-encode via `ease.crypto.base64_encode`).
  Request body: `{"text": …}` or `{"base64": …}`.
- `xml.to_json(xmlBytes) -> ?str`: JSON tree
  `{attrs…, "text": "…", "children": {tag: el | [el…]}}` — the
  PROPFIND/multistatus decoder. Walk it with std json.
- `time`: `now_unix_ms()`, `format_http_date(ms)` (RFC 1123),
  `format_amz_date(ms)` (`20060102T150405Z`), `parse_http_date(s) -> u64`.
- `context.storage_id() -> ?str`: nil = create mode, Some = edit (read
  once at `start` — it never changes for the instance).
- `library.playlists() -> [{id,title,musicIds}]` (ids are strings),
  `library.covers(musicIdsJson) -> [{musicId, id}]` (id = image resource
  or null; render via the kit's `Image(id)`).

## rut survival kit

Rust-family syntax. The corpus (`tur-upstream rut/cases/`, esp.
`github-viewer`, `countdown`, `todolist`) is the reference.

- Entries: `entry fn start() -> opaque` (+ optional `entry fn stop()`);
  op entries as above.
- Structs: `struct S { a: str, b: u64 }` — literal `S { a: "x", b: 0 }`;
  fields always public; `mut` bindings/params for field writes
  (`fn bump(mut s: S) { s.b = s.b + 1.0; }`).
- Locals: `let x: ?str = nil;` optionals; `when v { "a" -> {...}, else -> {...} }`;
  `for (let row of rows)`; `while (cond)`; `f"…{x}…"` with `{{`/`}}`
  escapes for literal braces; `x as u64` casts; unannotated float locals
  are `f32` — annotate `: f64` when handing them to rows.
- str: `len()`, `slice(a, b)`, `code_at(i)` (u32 codepoint),
  `starts_with(at, prefix)`, comparisons `==`. bytes: `.len()`, decode via
  `decode_utf8`.
- JSON (std pkg, offered): `use json::{ decodeJson, JsonReader,
  JsonDeserialize, DecodeErrorKind, DecodeJsonError };` — records spell
  `impl S { pub fn decode(mut r: JsonReader) -> (?Self, ?DecodeJsonError)
  { ... } }` (see github-viewer/schema.rut for the full reader idiom:
  `r.begin_object() / while (r.more()) { when (r.next_key()) { "k" ->
  {…}, else -> { r.skip_value(); } } / r.end_object() / r.failed() /
  r.read_str() / r.read_i64() / r.read_f64() / r.read_bool() /
  r.try_null() }`). Arrays ride a local generic shim
  (`fn decode_rows<T requires JsonDeserialize>(...) -> (?Vec<T>,
  ?DecodeJsonError)` with `use pouch::{ Vec };`).
- Async: `use futures::{ launch_future, sleep };` — `async fn f() -> str`,
  `await f(x)` / `await sleep(ms)`, launched from mutations/entries via
  `launch_future(f(...))`.
- Per-file use binding: EVERY file writes every import it calls; deep
  paths, one grouped use per module, names ASCII-sorted.
- No DOM/Node: no Date (use ease.time), no setTimeout (sleep), no btoa
  (ease.crypto).

## Views (tur_kit — the fluent law)

`view.rut` builds UI in `start` and mounts it. Construction is the class
call form, chained; `.build()` is the only terminal; handlers are sealed
mutations; reactive props take one method over `Readable<T>`; statics wrap
`sv(v)`.

```rut
use tur_kit::flags::{ Align, CrossAlign, MainAlign, MainAxisSize, BorderPosition };
use tur_kit::gesture::pointer::{ MouseRegion, PointerInteract };
use tur_kit::handles::{ View, mount, text_ctrl, undo_ctrl };
use tur_kit::layout::box::{ Container, SizedBox };
use tur_kit::layout::flex::{ Column, Expanded, Row };
use tur_kit::reactive::{ DeriveCtx, Mutation, MutationCtx, Readable, Source, derive, mutate, source, sv };
use tur_kit::scroll::{ ScrollView };
use tur_kit::text::core::{ Text };
use tur_kit::text::input::{ Input };
```

Shapes (from the corpus): `Column().main_alignment(MainAlign::Center)
.cross_alignment(CrossAlign::Stretch).main_axis_size(MainAxisSize::Min)
.child(v).children([...]).build()`; `Container().width_height(w, h)
.width(sv(w)) .padding(9.0).radius(sv(8.0f64)).color(sv(0xFFF8FAFCFFu64))
.border(color, 1.0, BorderPosition::Inside).alignment(Align::Center)
.child(...).build()`; `Text().text(sv("hi") | readable)
.font_size(13.0).color(0xFF1E293Bu64).query_key("key").build()`;
`SizedBox(w, h).build()`; `Expanded().flex(sv(1.0f64)).child(v).build()`;
`ScrollView().axis(Axis::Vertical).child(...).build()`; `Input()
.controller(ctrl).undo(undo).placeholder(sv("…")).font_size(14.0)
.width_height(0.0, 22.0).query_key("…").build()` with
`let ctrl = text_ctrl();` + `ctrl.text()` read / `ctrl.set_text("…")`;
taps: `PointerInteract().on_click(b).child(...).query_key("…").build()`
with `let b = mutate(fn (ctx: MutationCtx, _e: nil) { ... });`;
`MouseRegion().cursor(Cursor::Pointer).child(...).build()`;
`Condition(cond).then(v).else_branch(v).build()`;
`Each(list).item_builder(fn (i: u64, item: str) -> View {...}).build()`;
`mount(root)` — the last statement of `start`; `start` returns an atom id
(`-> u64`) or nothing.

View state: kit sources `let draft: Source<str> = source<str>("");` read
`ctx.get<str>(draft)` write `ctx.set<str>(draft, v)` inside mutations;
derive labels `derive<str>(fn (ctx: DeriveCtx) -> str { ... })`. Field
edits write the source; the connect button's mutation reads it, calls the
backend (`ease.rpc.call` — blocking control call), reports via a status
source. Long journeys: `launch_future(...)` + `await ease.rpc.call`-free
pattern (rpc.call is blocking — call it inside mutations, not tasks).

## House rules

- apiVersion **2**, versions bumped per plugin.
- Delete: `src/`, `package.json`, `pnpm-lock.yaml`, `rspack.config.cjs`,
  `tsconfig.json`, `scripts/`, `node_modules/` (never committed anyway).
- Validate: `cd rust-libs && cargo nextest run -p ease-plugins-check`
  (compile+boot for every entry file; contract-op registration checks).
- Boot law: `start` must not call Android-only rows (`themes`, `oauth`)
  or touch the network — register ops, seed state, return the ctx.
- The plugin dir ships ONLY: `manifest.json`, `*.rut`, icons.
