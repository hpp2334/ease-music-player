# Plugin porting guide — the emp epoch (apiVersion 3)

Plugins are **rut pkgs** (since apiVersion 2, the pkg-tree shape since
phase 6: `rut.jsonc` + the `src/` shell) served through the **`emp` SDK**
(apiVersion 3), packed into **dist artifacts** (`ease-plugin-packer` →
`dist/backend.zip`, `dist/view.zip` — source-mode rut zips) and shipped
inside the registry zips. There is no JS, no npm, no rspack — the engine
compiles the sources; `pnpm test` (the `ease-plugins-check` suite)
compile+boot-checks every entrypoint **from its packed dist artifact**
when present (the shipped bytes, end to end), falling back to the source
tree in dev. apiVersion 2 (single-file rut sources, `tur_host::ease.*`
rows, the stash-bus RPC) is RETIRED — the engine refuses installs outside
its `PLUGIN_API_MIN..MAX` range (currently `3..=3`), and the bundled
bootstrap re-installs the shipped ids from their apiVersion-3 zips.

## Layout (rut pkgs + dist artifacts)

```text
plugins/com.ease.webdav/
  manifest.json
  icon.png                 # optional plugin-level icon (≤128 KiB raster)
  backend/                 # the headless service ("backend": "backend.zip")
    rut.jsonc              # the pkg manifest — deps = the PATH deps only
    src/
      mod.rut              # the pkg ROOT module — declares `mod NAME;` children
      auth.rut  dav.rut  ops.rut  …
  view/                    # per-contribution views ("view": "view.zip")
    rut.jsonc
    src/
      mod.rut
      form.rut  actions.rut
  dist/                    # BUILD OUTPUT (gitignored) — the packer's zips
    backend.zip  view.zip
```

The pkg grammar is rut's own (`plugins/3rd/*` is the reference shape):
the root module is `src/mod.rut`, `mod NAME;` in it mounts
`src/NAME/mod.rut` (dir form) or `src/NAME.rut` (single-file form) beside
the declaring file — recursively, both spellings, both present = the loud
ambiguity. A `.rut` outside `src/` refuses at the pack door.

### Manifest entries: dist pointers, source fallback (D8)

The manifest PREFERs **dist pointers**: `"backend": "backend.zip"`,
contribution `"view": "view.zip"` — the packed artifact under `dist/`
loads through the engine's pkg-zip rail (unpacked + compiled per
instance load, the embedded `deps/` closure riding along). The
source-tree spelling (`"backend": "backend"`) stays fully supported: a
dist pointer whose artifact is MISSING falls back to the pkg tree of the
same stem (`backend.zip` → `backend/`) — dev checkouts without `dist/`
and legacy single-file/pkg spellings keep loading.

### Deps in `rut.jsonc`: path deps only; the session supplies the rest

`rut.jsonc`'s `deps` table declares ONLY the **path deps** — the 3rd
libs the entrypoint uses, each embedded into the packed zip under
`deps/` by the packer:

```jsonc
{
  "name": "backend",
  "deps": {
    "crypto": { "path": "../../3rd/crypto" },
    "net":    { "path": "../../3rd/net" }
  }
}
```

The **session deps** — `emp`, `json`, `pouch`, `strbuild`, `tur_kit`,
`tur_host` (and every `*_host` decl pkg) — are deliberately NOT
declared: they resolve against the engine's offered set at device
compile through the pkg-zip lane's ambient-nameability law (the same law
every module load runs). rut's grammar has no "satisfied by the session"
descriptor — `rut pack` embeds every row it walks — so declaring them
would embed the SDK into every plugin zip; the grammar-shaped answer is
declare-what-embeds, let-the-session-supply-the-rest.

## Manifest (v3)

```json
{
  "id": "com.ease.webdav",
  "name": { "en-US": "WebDAV", "zh-CN": "WebDAV" },
  "author": "ease",
  "version": "0.4.1",
  "apiVersion": 3,
  "backend": "backend.zip",
  "events": ["music:play"],
  "contributions": {
    "storages":  [{ "id": "webdav", "title": "…", "desc": "…", "view": "view.zip" }],
    "dashboard": [{ "id": "main", "title": "…", "view": "view.zip" }],
    "lyricParsers": [{ "id": "lrc", "title": "…", "extensions": ["lrc"] }]
  }
}
```

`apiVersion` is REQUIRED — an integer, and the packaging epoch gate
(`scripts/package-plugin.ts`) fails any build whose level ≠ 3. Entry
values name **dist artifacts** (`.zip`) or — as the fallback — source
spellings.

## The emp SDK (constructor-injected handle)

```rut
use emp::{ Db, Secret, OAuth, Theme, Context, Library, Rpc };
```

`entry fn start(cx: opaque)` receives the **plugin handle** — the
host-minted identity (`cx`'s u64 is the whole crossing; a rut-side
`opaque(<u64>)` forgery can only name the forger's own id, a foreign id
or any other box shape traps `emp:{ns}:`). Every service class is
constructed FROM the handle and its members are one forwarded `emp_host`
row each:

- `Db(cx)`: `get(key) -> ?str`, `set(key, value)`, `delete(key)`,
  `append(key, value)`, `get_all(key) -> [v]` (JSON), `count(key) -> u64`,
  `list_keys(prefix) -> [{key,kind}]`; the `*_multi` twins take/answer
  JSON strings.
- `Secret(cx)`: `get(id) -> ?str`, `put(secret) -> i64`, `remove(id)`.
- `OAuth(cx)`: `mint() -> flowId`, `begin(flowId)` (the browser flow;
  the redirect lands in your backend's `oauth:exchange` op).
- `Theme` (no field): `color(name) -> "#RRGGBBAA"`, `is_dark() -> bool`
  (Android-only rows — never at boot).
- `Context(cx)`: `notify_change()`, `create_storage(id)`,
  `remove_storage(id)`, `storage_id() -> ?str` (nil = create mode,
  Some = edit — read once at `start`).
- `Library(cx)`: `playlists() -> [{id,title,musicIds}]` (ids are
  strings), `covers(musicIdsJson) -> [{musicId, id}]` (`id` = the
  registered engine image resource or null; render via the kit's
  `Image(id)`).

Boot law: constructors only STORE the handle — constructing services at
`start` touches no host row, no network, nothing Android-only.

## The RPC contract (bind table — the only contract)

A backend serves ALL ops through **one dispatch entry per lane**,
declared once in the root module (`src/mod.rut`), plus `rpc.bind` per op at `start`.
Handlers are plain fns closing over the service handles; **args are RAW
— the handler decodes its own args** (`json` lib for str ops):

```rut
use emp::{ Db, Rpc };

entry fn start(cx: opaque) {
    const db = Db(cx);
    const rpc = Rpc(cx);
    rpc.bind("storage:list", fn (args: str) -> ?str { return storage_list(db, args); });
    rpc.bind("storage:get", fn (args: str) -> ?str { return storage_get(db, args); });
    rpc.bind("storage:get-chunk", fn (args: str) -> ?str { return storage_get_chunk(db, args); });
    rpc.bind("storage:release", fn (args: str) -> ?str { return storage_release(db, args); });
    rpc.bind("webdav:test", fn (args: str) -> ?str { return webdav_test(db, args); });  // view op
    rpc.subscribe("music:play", fn (payload: str) -> ?str { return on_play(db, payload); });
    // no answer — the identity rode the cx lane, not a return
}

entry fn rpc(cx: opaque, op: str, args: str) -> ?str {
    const rpc = Rpc(cx);
    return rpc.dispatch(op, args);
}

// only backends with binary ops need the twin lane
entry fn rpc_bin(cx: opaque, op: str, args: bytes) -> ?bytes {
    const rpc = Rpc(cx);
    return rpc.dispatch_bin(op, args);
}
```

- The handler answers the RAW payload; **`nil` = "not mine"** (the lyric
  dispatch's fall-through arm). An error is a rut `panic(...)` — the trap
  message rides the host's error arm, and **messages beginning
  `UNAUTHORIZED` / `TIMEOUT` map to typed host UI states** — prefix
  your auth/timeouts with those.
- The `{"ok":true,"result":…}` / `{"ok":false,"error":"…"}` envelope is
  built HOST-side from the answer/trap — no envelope code exists in rut.
- Views call the backend with `Rpc(cx).call(op, argsJson) -> str` (the
  envelope passes through; blocking — call it inside mutations, not
  tasks). Events are dispatched through the str entry under the
  `event:<type>` key; `rpc.subscribe(type, f)` files the handler.
- Contract literals (the host calls these exact strings):
  - storage backends: `storage:list`, `storage:get` (opener →
    `{sid, meta:{totalLength?, name?, contentType?, dataOffset?}}`),
    `storage:get-chunk` (`{sid, offset, len}` → `{data: <base64>,
    eof: bool}` — the final chunk carries data AND eof), `storage:release`
    (`{sid}`), plus `oauth:url` / `oauth:exchange` /
    `storage:removeInstance` for OAuth providers.
  - lyric parsers: `lyric:parse` on the **binary lane** (see below).
  - view ops (plugin-private names, e.g. `webdav:test`) bind on the same
    table; views reach them via `Rpc.call`.
- Keep per-session state (the `storage:get` session map) in the module
  (`let` bindings at file scope or behind `Db`). `storage:release` must
  drop the session.

### The binary lane + the lyric framing

`rpc.bind_bin(op, fn (args: bytes) -> ?bytes)` — raw bytes in, raw bytes
out, **no base64 anywhere**. Only ops that want raw content need the
twin `entry fn rpc_bin`; the per-op lane is declared by which entry the
host addresses (`rpc` = str args, `rpc_bin` = bytes args).

Binary ops need a way to carry a few control fields beside the bytes.
The convention (used by `lyric:parse`, keep it for your own binary ops):

```text
<compact JSON header>\n<raw content bytes>
```

The header (`{"parserId":"lrc","fileName":"song.lrc"}`) never contains a
literal newline (JSON string escaping), so the FIRST `\n` byte ends it.
Decode: read the whole payload lossily as UTF-8
(`tur_host::clipboard::decode_utf8`), split at the first `\n` (str ops —
no bytes slicing anywhere), parse the header with the `json` lib, and
answer `tur_host::clipboard::encode_utf8(payload)`. The host owns the
size cap (≤2 MiB for lyrics) and the timeout (10 s).

## The 3rd-party libs (utilities)

The identity-free utilities left the SDK — they are standard rut pkgs
imported by name and declared as PATH deps in the entrypoint's
`rut.jsonc` (the packer embeds the closure into the dist zip):

```jsonc
// backend/rut.jsonc — only the libs this entrypoint uses
"deps": {
  "crypto": { "path": "../../3rd/crypto" },
  "net":    { "path": "../../3rd/net" },
  "time":   { "path": "../../3rd/time" },
  "xml":    { "path": "../../3rd/xml" }
}
```

```rut
use crypto::{ random_uuid, md5_hex, md5_base64, base64_encode, base64_decode, sha256_hex, hmac_sha256_hex, random_bytes };
use xml::to_json;                    // bytes -> ?str (JSON tree)
use net::{ request, finish, status, status_text, headers, body, error, request_sync };
use time::{ now_unix_ms, format_http_date, format_amz_date, parse_http_date, unix_ms };
```

- `net` (full HTTP — upstream's rut net rows carry no headers/bodies,
  ours do):
  ```rut
  let opts = f"{{\"url\":\"{url}\",\"method\":\"PROPFIND\",\"headers\":{{\"Depth\":\"1\",\"Authorization\":\"Basic {auth}\"}},\"body\":{{\"text\":{body_json}}}}}";
  let t = request(opts);
  let data = await finish(t);
  let code = status(t);            // u64, 0 = no response
  let hdrs = headers(t);           // JSON {name: value} (lowercased names)
  let err = error(t);              // "" on success
  ```
  Response body is raw `bytes`; decode with `decode_utf8(data)` from
  `tur_host::clipboard` (or base64-encode via `crypto.base64_encode`).
  Request body: `{"text": …}` or `{"base64": …}`. `request_sync(opts)
  -> str` is the blocking one-shot (entry-fn-safe — the async `request`/
  `finish` pair must not be awaited from a sync entry).
- `xml.to_json(xmlBytes) -> ?str`: JSON tree
  `{attrs…, "text": "…", "children": {tag: el | [el…]}}` — the
  PROPFIND/multistatus decoder. Walk it with std json.
- `time`: `now_unix_ms()`, `format_http_date(ms)` (RFC 1123),
  `format_amz_date(ms)` (`20060102T150405Z`), `parse_http_date(s) -> u64`,
  `unix_ms(secs) -> u64` (the JS `Date.now()/1000` round-trip).

`tur_host::clipboard::{ decode_utf8, encode_utf8 }` are the engine's own
rows (UTF-8 both ways) — the one `tur_host` import a plugin may still
spell.

## rut survival kit

Rust-family syntax. The corpus (`tur-upstream rut/cases/`, esp.
`github-viewer`, `countdown`, `todolist`) is the reference.

- Entries: `entry fn start(cx: opaque)` (+ optional `entry fn stop()`);
  one dispatch entry per lane as above.
- Structs: `struct S { a: str, b: u64 }` — literal `S { a: "x", b: 0 }`;
  fields always public; bindings/params are mutable by default (the
  drop-mut law — `mut` is gone from the grammar; write fields freely:
  `fn bump(s: S) { s.b = s.b + 1.0; }`, `const` for the shallow opt-out).
- Locals: `let x: ?str = nil;` optionals; `when v { "a" -> {...}, else -> {...} }`;
  `for (let row of rows)`; `while (cond)`; `f"…{x}…"` with `{{`/`}}`
  escapes for literal braces; `x as u64` casts; unannotated float locals
  are `f32` — annotate `: f64` when handing them to rows. A tail
  `panic(...)` statement is a missing-return parse repro at this pin —
  wrap the panic in an `if`.
- str: `len()`, `slice(a, b)`, `code_at(i)` (u32 codepoint),
  `starts_with(at, prefix)`, comparisons `==` — all three use the SAME
  index units, so scan+slice compose. bytes: `.len()`, `.clone()`;
  decode/encode via `tur_host::clipboard`.
- JSON (std pkg, offered): `use json::{ decodeJson, JsonReader,
  JsonWriter, JsonDeserialize, DecodeErrorKind, DecodeJsonError };` —
  records spell `impl S { pub fn decode(r: JsonReader) -> (?Self,
  ?DecodeJsonError) { ... } }` (see github-viewer/schema.rut for the
  full reader idiom: `r.begin_object() / while (r.more()) { when
  (r.next_key()) { "k" -> {…}, else -> { r.skip_value(); } } /
  r.end_object() / r.failed() / r.read_str() / r.read_i64() /
  r.read_f64() / r.read_bool() / r.try_null() }`). Arrays ride a local
  generic shim (`fn decode_rows<T requires JsonDeserialize>(...) ->
  (?Vec<T>, ?DecodeJsonError)` with `use pouch::{ Vec };`).
- Async: `use futures::{ launch_future, sleep };` — `async fn f() -> str`,
  `await f(x)` / `await sleep(ms)`, launched from mutations/entries via
  `launch_future(f(...))`.
- No DOM/Node: no Date (use time), no setTimeout (sleep), no btoa
  (crypto).

## Views (tur_kit — the fluent law)

`view/src/mod.rut` builds UI in `start` and mounts it. Construction is the
class call form, chained; `.build()` is the only terminal; handlers are
sealed mutations; reactive props take one method over `Readable<T>`;
statics wrap `sv(v)`.

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
backend (`Rpc(cx).call(op, args)` — a blocking control call), reports via
a status source. Long journeys: `launch_future(...)` + `await` — but
`call` is blocking: call it inside mutations, not tasks.

## House rules

- apiVersion **3**, versions bumped per plugin (an installed copy heals
  to a newer bundled zip via the bootstrap's outdated check — bump
  whenever the packed bytes change).
- Validate: `cd rust-libs && cargo nextest run -p ease-plugins-check`
  (compile+boot for every entrypoint — from its PACKED dist artifact
  when present, else the source tree; bind-table greps against the
  source trees; the bytes-lane E2E).
- Package: `pnpm build:plugins` — runs the packer
  (`rust-libs/ease-plugin-packer`, the `rut pack --mode source` lane →
  `dist/<entry>.zip`) then `scripts/package-plugin.ts` (stamps the
  registry zips + index — fails any build whose manifest apiVersion ≠ 3
  or whose dist artifact is missing).
- Boot law: `start` must not call Android-only rows (`Theme`,
  `OAuth.begin`) or touch the network — construct services, bind the
  ops, return nothing.
- The plugin dir ships: `manifest.json`, the rut pkg trees
  (`rut.jsonc` + `src/`), icons. `dist/` is build output (gitignored) —
  the registry zips + `plugins.json` are the committed artifacts.
