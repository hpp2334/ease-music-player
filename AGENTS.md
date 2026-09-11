# AGENTS.md

Guide for coding agents working in this repository. Read this first.

## Project overview

Ease Music Player is a lightweight **Android** music player written in **Kotlin / Jetpack Compose** (UI) and **Rust** (backend). It targets Android `arm64-v8a` only.

Features: WebDAV and OneDrive cloud storage (both JS plugin providers), playlist-based playback, music cover art, lyrics.

> **History note (0.3 → 0.4):** version 0.4 briefly migrated the UI to Kotlin Multiplatform / Compose Multiplatform with a Desktop JVM target (JavaFX `MediaPlayer` + Skiko). The desktop build was dropped for 0.4.0-beta.0 — memory overhead (~half a GB at idle, mostly from loading two rendering stacks) and lack of user-facing benefit made the single-target Android app the better shape. The Rust-side improvements from that era are kept (`ease-client-schema` / `ease-client-migration` crate split, UniFFI tokio routing). See [`docs/motivation.md`](./docs/motivation.md).

## Architecture at a glance

```
┌──────────────────────────────┐    JSON bridge + thin JNI    ┌──────────────────────────────┐
│  Kotlin / Jetpack Compose    │ ───────────────────────────▶ │  Rust workspace (rust-libs/) │
│  android/app/  (Gradle :app) │                              │  ease-client-android (cdylib: │
│                              │ ◀─────────────────────────── │   libease_client_android.so) │
│  Hilt DI, MediaSessionCompat │  StateFlows + fire-and-forget│  → ease-client-backend (rlib) │
│  EaseBackend facade          │  signals (EaseSignalHost)    │  + Sea-ORM / SQLite          │
│                              │  via repositories            │  + cantode audio engine      │
└──────────────────────────────┘                              └──────────────────────────────┘
```

- **Rust side** ([`rust-libs/`](./rust-libs/)) is split along the platform seam: [`ease-client-android`](./rust-libs/ease-client-android) is the cdylib the app loads — it hosts **every** JNI symbol (the JSON-bridge `EaseBridge.call`, the tur engine's `TurNative.*`, `EasePluginBridge.*` runtime binding, `nativeInitAndroidContext`) plus the `ease:*` JS host module; [`ease-client-backend`](./rust-libs/ease-client-backend) is the platform-agnostic business half (SQLite via Sea-ORM, controllers/services/repositories) that stays host-testable. The two meet through the `PluginEngineHost` trait (backend defines, android implements over the tur engine). Audio decode + output live in the separate [`cantode/`](./cantode/) repo-root engine (symphonia + cpal/AAudio; Rust crate in `cantode/rust/`, Kotlin facade in `cantode/kotlin/`), linked into the same `.so`.
- **Kotlin side** ([`android/app/`](./android/app/)) talks to the backend through [`singleton/Bridge.kt`](./android/app/src/main/java/com/kutedev/easemusicplayer/singleton/Bridge.kt) (the JSON+buffer bridge) and owns the Android half of the process graph in the [`EaseBackend`](./android/app/src/main/java/com/kutedev/easemusicplayer/singleton/EaseBackend.kt) facade: the shared tur runtime lifecycle (pools → createRuntime → bind) plus the typed fan-out of Rust signals (`EaseSignalHost.onSignal` → `BackendSignal`). Heavy logic — which plugin backends run, when they reload — is Rust-side (`plugin_manager::reload_backends`); JNI is a thin layer.
- **Playback**: [`cantode`](./cantode/) decodes (symphonia: mp3/flac/vorbis/ogg/wav/aac/isomp4) and renders via cpal's AAudio backend. Its Kotlin half (`cantode/kotlin/`, package `com.kutedev.cantode`) is the `Cantode` facade — a 10 Hz poller + transport commands over cantode's **own JNI bridge** (`Java_com_kutedev_cantode_CantodeNative_*`, feature `ffi`, compiled into the same `.so`; players are addressed by the same bridge handle id `player.new` registers). Business logic talks to the engine only through `cantode.play(...)`-style calls and its `state`/`loading`/`ended` flows; the load itself (`player.loadMusic` with `autoplay`) stays on the backend bridge because source construction (storage plugins) and the metadata→DB writeback are business logic. [`PlaybackService`](./android/app/src/main/java/com/kutedev/easemusicplayer/core/MusicPlayer.kt) is a plain `android.app.Service` (no longer `MediaSessionService`) that owns a `MediaSessionCompat` from `androidx.media:media` for notification / lock-screen / Bluetooth / Auto integration. No media3 / ExoPlayer dependency remains.

## Repository layout

| Path | Purpose |
|---|---|
| [`android/`](./android/) | **Gradle root** of the Android project: `settings.gradle.kts`, `build.gradle.kts`, `gradle/`, `gradlew*`, `gradle.properties`, `gradle/libs.versions.toml`. |
| [`android/app/`](./android/app/) | The `:app` Gradle module — the Android application (Kotlin + Compose + Hilt + MediaSessionCompat). |
| [`rust-libs/`](./rust-libs/) | Cargo workspace of Rust crates (backend, schema, migration, FFI builder, etc.). |
| [`scripts/`](./scripts/) | TypeScript build/test orchestration (run via `pnpm`/`tsx`). |
| [`docs/`](./docs/) | `motivation.md` + screenshots. |
| [`.github/workflows/`](./.github/workflows/) | CI (PRs + `main` pushes + release tags): build JNI, run Rust tests, build + upload the signed APK as an artifact; attach it to the GitHub release on `v*` tags. |
| [`.opencode/`](./.opencode/) | OpenCode agent config (subagents for git/PR finalization, multimodal image reading + device operation). |

Root Java package: `com.kutedev.easemusicplayer`. Namespace / applicationId: `com.kutedev.easemusicplayer`.

## Kotlin source layout (`android/app/src/main/java/com/kutedev/easemusicplayer/`)

- `MainActivity.kt` — `@AndroidEntryPoint` `ComponentActivity`; also declares the top-level `@HiltAndroidApp class EaseMusicPlayerApplication`. Hosts `setContent { Root() }`, requests permissions (notably `POST_NOTIFICATIONS` for the playback foreground service), and runs the startup reload sequence.
- `Root.kt` — main `@Composable` (`NavHost`, routes, theme).
- `core/`
  - `MusicPlayer.kt` — `PlaybackService` (plain `android.app.Service` owning a `MediaSessionCompat` for system integration). `@AndroidEntryPoint`.
  - `KeepBackendService.kt` — foreground service that keeps the Rust backend process alive: it is the **single lifecycle driver** of the Android process graph via the [`EaseBackend`](../singleton/EaseBackend.kt) facade (`start(context)` → pools → `createRuntime(ctx, pools, backendHandle)` → `bindPluginRuntime(backendHandle, runtimeHandle, poolsHandle, assets)`, every step logged to logcat + the in-app backend log; `stop(reason)` → `unbindPluginRuntime` (CAS — a stale stop never clobbers a newer binding — and it tears down the Rust-owned headless backends) → `TurRuntime.close`; `runtimeOrNull()` / `runtime: StateFlow<TurRuntime?>` for consumers, so UI pages self-heal when the service restarts in-process). The headless backend instances themselves are **Rust-owned**: `bindPluginRuntime` attaches a `TurEngineHost` (the `PluginEngineHost` impl) and triggers `reload_backends` (scan → spawn headless instance → load backend.js → wire the `RpcClient`), and every set-changing mutation re-runs it — no plugin lifecycle logic, instance list, or revision collector on the Kotlin side. `@AndroidEntryPoint`.
  - `CoroutineScopeModule.kt` — Hilt `@Module` providing the app-wide `CoroutineScope` (`SupervisorJob + Dispatchers.Default`).
- `singleton/` — `Bridge` + repositories (see [Key patterns](#key-patterns)).
- `viewmodels/` — `@HiltViewModel` ViewModels (`PlayerVM`, `PlaylistsVM`, `PlaylistVM`, `AssetVM`, `CreatePlaylistVM`, `EditPlaylistVM`, `EditStorageVM`, `ImportVM`, `StoragesVM`, `SleepModeVM`, `LogVM`, `DebugMoreVM`, `ToastVM`).
- `widgets/` — Compose screens organized by feature (`appbar/`, `dashboard/`, `devices/`, `home/`, `musics/`, `playlists/`, `settings/`, plus `ToastWidget.kt`).
- `components/` — reusable composables (Checkbox, ConfirmDialog, Form, MusicCover, ...).
- `ui/theme/` — `Color.kt`, `Theme.kt`, `Type.kt`.
- `utils/` — `Duration.kt`, etc.

### Android resources & manifest
[`android/app/src/main/`](./android/app/src/main/)
- `AndroidManifest.xml` — declares `EaseMusicPlayerApplication`, `MainActivity`, `PlaybackService` (plain service owning a `MediaSessionCompat`), `KeepBackendService`, OAuth2 redirect (`easem://oauth2redirect`), `FileProvider`.
- `res/` — Android resources (mipmaps, `values/strings.xml`, `values-zh-rCN/strings.xml`, `xml/backup_rules.xml`, `xml/data_extraction_rules.xml`, `xml/file_paths.xml`).
- `assets/` — Compose resources (`composeResources/drawable/`, `composeResources/font/noto_sans.ttf`).
- `jniLibs/arm64-v8a/` — gitignored, generated by `pnpm build:jni`.

## Rust crates (`rust-libs/`)

Workspace root: [`rust-libs/Cargo.toml`](./rust-libs/Cargo.toml) (resolver = `"2"`, centralized `[workspace.dependencies]`). `clippy.toml` sets `large-error-threshold = 256`.

| Crate | Purpose |
|---|---|
| `ease-client-android` | **The cdylib the app loads** (`libease_client_android.so`), GPL-3.0. The Android embedder half: every JNI symbol (`EaseBridge.call` JSON bridge in `bridge_jni.rs`, `TurNative.*` + `EasePluginBridge.*` in `plugin_runtime/plugin_jni.rs`, `nativeInitAndroidContext`), the `ease:*` JS host module (`plugin_runtime/`), CJK fonts, and `engine.rs` — the `TurEngineHost` impl of backend's `PluginEngineHost` trait (module-source registration, Rust-driven headless spawns with JVM attach + `FrameLoop` wiring, `RpcClient` extraction, signal emission). |
| `ease-client-backend` | Platform-agnostic business half; `crate-type = ["rlib"]`, GPL-3.0. Controllers/services/repositories, `ctx.rs`, the JSON bridge dispatch + handle table, and the `PluginEngineHost` seam. Host-testable (`cargo nextest` runs on macOS; two documented Android-dep exceptions: `cantode`+`ffi` so the audio JNI rides the `.so`, and the `tracing-android` logcat layer that must compose into the set-global subscriber). Source: `controllers/`, `services/`, `repositories/`, `objects/`, `bridge/`. |
| `ease-client-schema` | Sea-ORM entities, models, domain types. |
| `ease-client-migration` | DB migration from legacy `redb` format to SQLite. Versioned upgraders in `src/legacy/` (`redb_v2`, `redb_v3`, `schema_v2`, `schema_v3`, `upgrader_v1_v2`, `upgrader_v2_v3`); the SQLite (v4) schema is created by a **single collapsed init migration** (`src/migrations/`) — only the v3 (redb) → v4 (SQLite) import is considered; the four old migration names remain as **no-op tombstones** so already-migrated dev databases still resolve their `seaql_migrations` rows. Integration tests in `tests/`. |
| `ease-client-tokio` | Shared tokio multi-thread runtime accessor (`tokio_runtime()`). |
| `ease-order-key` | Standalone orderable-key utility. **Dual MIT OR Apache-2.0 license** (different from the rest). |
| `ease-remote-storage` (path dep, not a workspace member) | Storage backend trait + `LocalBackend` (the native WebDAV client was removed when WebDAV became a JS plugin). GPL-3.0. |
| [`cantode/`](./cantode/) (repo root, **not** in `rust-libs/` workspace) | Standalone cross-platform audio engine owning both halves: `cantode/rust/` (symphonia decode + cpal/AAudio output behind a trait-based API; `ffi` feature adds the JNI surface) and `cantode/kotlin/` (pure-JVM Gradle module `:cantode-engine`, package `com.kutedev.cantode` — the Kotlin facade; no biz logic). Linked into the same `.so` as `ease-client-backend`. Edition 2024, **dual MIT OR Apache-2.0 license** (matches `ease-order-key`, different from the GPL-3.0 main app). |

Notable Rust constraints: SQLite is force-bundled (`libsqlite3-sys` `bundled`) for cross-compilation; Sea-ORM 1.1 with sqlx-sqlite + runtime-tokio-rustls; the Kotlin↔Rust seam is a single hand-written JNI JSON bridge (no UniFFI anymore).

## Key patterns

### Dependency injection (Hilt)
- [`core/CoroutineScopeModule.kt`](./android/app/src/main/java/com/kutedev/easemusicplayer/core/CoroutineScopeModule.kt) — Hilt `@Module` (`SingletonComponent`) providing the app `CoroutineScope` (`SupervisorJob + Dispatchers.Default`).
- Repositories & `Bridge` are `@Inject constructor`-annotated `class`es; Hilt constructs them automatically. ViewModels are `@HiltViewModel` with `@Inject constructor`.
- `EaseMusicPlayerApplication` (top-level class in `MainActivity.kt`) is annotated `@HiltAndroidApp`. Activities/services use `@AndroidEntryPoint` + `@Inject lateinit var`.

### Bridge / FFI
[`singleton/Bridge.kt`](./android/app/src/main/java/com/kutedev/easemusicplayer/singleton/Bridge.kt) is a thin singleton over the single JNI JSON-bridge entrypoint (`EaseBridge.call(payloadJson, buffers) -> NativeBridgeResult`, hand-written in `rust-libs/ease-client-android/src/bridge_jni.rs`):
- `initialize()` / `destroy()` — create / dispose the backend.
- `run { backend -> ... }` — suspend, **swallows** exceptions and returns `null`.
- `runRaw { ... }` — suspend, **propagates** exceptions.
- `runSync { ... }` / `runSyncRaw { ... }` — non-suspend variants.

Repositories call backend functions through `bridge.run { }`. Method names: business ops `ct*` (controller) / `cts*` (controller service) plus namespaced ops (`plugin.*`, `player.*`, `backend.*`). Argument structs are prefixed `Arg*` (e.g. `ArgUpsertStorage`, `ArgCreatePlaylist`, `ArgRemoveMusicFromPlaylist`) and mirror in Kotlin under `singleton/types/`. Binary payloads (cover art) ride the `Array<ByteArray>` buffer side-channel.

The Rust backend spawns work on the shared tokio runtime via `ease_client_tokio::tokio_runtime()`; the JNI bridge dispatcher blocks the calling thread on it (`tokio_runtime().block_on(dispatch(...))`).

### Startup sequence
`MainActivity.onStart()` launches a `lifecycleScope` coroutine that calls `reload()` on `playerRepository`, `storageRepository`, `playlistRepository` (in that order). `PlaybackService` is started lazily on first play via `PlayerControllerRepository` (it owns the `MediaSessionCompat`); `MainActivity` no longer wires a `MediaController`. `Bridge.initialize()` runs at process start in `EaseMusicApplication.onCreate()` (idempotent — `MainActivity` still calls it as a no-op) so the backend-owned in-app language preference can be loaded asynchronously before the first activity applies its locale (`LanguageSetting`).

### Repository pattern
All in `singleton/`, constructed with `Bridge` + `CoroutineScope`, expose `StateFlow` / `SharedFlow`:
- `PlayerRepository` — current music/playlist, play mode, derived `previousMusic` / `nextMusic` / `onCompleteMusic` flows, pause requests.
- `PlaylistRepository` — playlist list (debounced reload), reorder via `ease-order-key`, reacts to storage-removal events.
- `StorageRepository` — cloud storage list, OAuth refresh token, remove events.
- `AssetRepository` — in-memory cache for cover art bytes + decoded bitmaps.
- `PluginRepository` — Kotlin registry of the Rust-side plugin scan: `scanPlugins()` calls the `plugin.list` bridge method and publishes `installedPlugins` / `enabledPlugins` / `dashboardItems` / `storageProviders` / `lyricParsers` (+ derived `lyricExtensions` feeding the import/browse lyric filters) / `lyricParserSelection` (manifest data classes carry **module-source handles**, not JS text); `bindPlayerEvents()` forwards player events to JS backends (see [Plugin system](#plugin-system-js-plugins)).
- `PlayerControllerRepository`, `PermissionRepository`, `ImportRepository`, `ToastRepository`, `PluginManager` (thin bridge facade + SAF copy + revision), `PluginRegistryRepository` (thin bridge facade). The install/state/registry logic itself is Rust-side (see [Plugin install model](#plugin-install-model-rust-manager)).

### Plugin system (JS plugins)
Plugins live under [`plugins/`](./plugins/) (TS sources, rspack bundles into `plugins/<id>/dist/`) and are distributed as registry zips ([`plugins/registry/`](./plugins/registry/), committed); `com.ease.webdav` (storage) and `com.ease.lyricformats` (the lyric parsers — see `lyric:parse` below) are additionally bundled into `assets/plugin-bundles/` and ensure-installed by the bootstrap (see [Plugin install model](#plugin-install-model-rust-manager)). At runtime plugins are installed under `filesDir/plugins/<id>/` — the Rust-side scan (`plugin.list`) walks those folders (enabled plugins only contribute storages/dashboard items/lyric parsers). Manifest schema:

```json
{
  "id": "com.ease.onedrive",
  "name": { "en-US": "OneDrive", "zh-CN": "OneDrive" },   // string | { "<tag>": string } (localized text)
  "author": "ease",
  "backend": "backend.js",                    // optional; long-lived module (headless tur instance)
  "icon": "icon.png",                         // optional plugin-level icon (raster ≤128 KiB) — installed + registry lists
  "events": ["music:play"],
  "contributions": {
    "storages":  [{ "id": "onedrive", "title": "…", "desc": "…", "view": "view.js" }], // per-storage config view (short-lived)
    "dashboard": [{ "id": "main", "title": "…", "desc": "…", "icon": "icon.png", "view": "view.js" }],  // dashboard entry cards → standalone view page
    "lyricParsers": [{ "id": "subtitle", "title": "…", "extensions": ["srt", "vtt"] }]  // headless lyric-format parsers (no view)
  }
}
```

- **Localized text**: `name` / `description` and contribution `title` / `desc` accept a plain string (the base/default text) or a tag→string map (`{"en-US": …, "zh-CN": …}`). Rust normalizes both into `{base, locales}` over the wire; **Kotlin resolves at render time** (`LocalizedText.resolve()`: exact tag → language prefix → base) against the activity locale, so the language-switch recreate flow just works and `SYSTEM` mode needs no Rust knowledge. Missing contribution `title` falls back to the plugin `name`.
- **Contribution icons**: `"icon": "icon.png"` names a raster file (`.png`/`.webp`/`.jpg`, ≤128 KiB) in the plugin root; the Rust scan base64s it into the `plugin.list` payload (`iconData`) and Kotlin decodes with `BitmapFactory` (built-in glyph fallback). `package-plugin.ts` copies manifest-referenced icons into the zip (missing icon = packaging error). A **plugin-level** root `"icon"` field follows the same validation/scan path and feeds the installed-plugins list + registry list (`components/PluginIcon.kt` renders bytes-or-glyph); `package-plugin.ts` additionally publishes it as `plugins/registry/icons/<id>.png` with an `"icon"` path in `plugins.json`, which Rust resolves against the source base URL, fetches during `registryFetch` (≤128 KiB, non-fatal on failure), and caches as raw bytes in `plugin-registry-cache/<md5>.icon` for offline `registryCached`.

- **`backend`** — one per plugin; all contributions share it. Loaded once by `KeepBackendService` into a headless tur instance stamped with `PluginId`; registers `tur:rpc` handlers, **split by caller**: `hostRpc` for ops the Rust host invokes — **contract literals, identical names for every provider**, with identity riding the payload (`storage:list` / `storage:get` `{ pluginId, storageId, … }`, `storage:removeInstance`, `oauth:url` / `oauth:exchange` `{ pluginId, oauthId, … }`; `registerStream` openers resolve `{ meta, body, release?, mapError? }`, no stream ids in plugin code; plus `onEvent` for host-fired event subscriptions like `music:play` in `com.ease.playcount`'s backend) — and `viewRpc` (`registerHandler` only) for ops the plugin's own view reaches via `ease.rpc.call` (e.g. `webdav:test` / `webdav:connect`, plugin-private names). The dispatcher routes strictly by the request envelope's `scope` — a mis-registered op is a `no host/view handler` error at the first call. The host composes no op names and derives no provider prefixes: `pluginId` comes from the storage row or the instance data slot, `storageId` is the `plugin_storage_id`, and `oauthId` is a host-minted flow token (`ease.oauth.new()`). Backend modules load by **module-source handle** (see below), never as JS strings across JNI.
- **`view`** — per contribution; loaded into a `TurView` when the page opens (add-storage form, plugin page) and destroyed on leave.
- **`lyricParsers`** — headless-only contributions (no `view`); each declares `extensions` (lowercase, dot-free, `^[a-z0-9]{1,12}$`, ≤16 per parser). **There is no built-in lyric parser — LRC included**: `services/lyrics/` only dispatches, holding zero format knowledge. On `music.loadLyric` the Rust seam resolves candidate files (explicit `music.lyric`, else one sibling `<audio>.<ext>` per registered extension, ≤8 probes), fetches bytes over the storage seam and calls each matching plugin's `lyric:parse` hostRpc op (`{ pluginId, parserId, fileName, size, contentBase64 }` → `{ lines: [{ timeMs, durationMs?, text }], metadata? }` | `null` = "not mine" → next parser; 10 s timeout, ≤2 MiB content) — see [`services/lyrics/mod.rs`](./rust-libs/ease-client-backend/src/services/lyrics/mod.rs) and [`com.ease.lyricformats`](./plugins/com.ease.lyricformats/) (LRC/SRT/VTT via a hand-written, regex-free lexer/parser in `src/parsers.ts` — no parsing deps; `pnpm test` in the plugin runs its Node selftest; bundled into the APK). Settings → 通用 → 歌词解析 (`LyricParserPage`) shows one group per extension with an Auto default and a per-extension user pick persisted Rust-side in `plugin-state.json` (`lyricParserSelection`, ext → `<pluginId>:<parserId>`; `plugin.setLyricParserSelection` bridge — never bumps the generation, dispatch reads it at call time; uninstall drops a plugin's entries).
- **Worker pools** (`plugin_runtime/plugin_jni.rs`): all plugin instances share two capped tur worker pools — `ease-plugin-backend` (2 lane threads, all headless backends) and `ease-plugin-view` (2 lane threads, all TurView instances) — instead of the engine default of one dedicated thread per instance. Apps within a pool share lanes cooperatively; pools stay isolated from each other. The pools live behind an opaque `jlong` handle (`EasePluginBridge.createPluginWorkerPools()` → boxed `PluginWorkerPools` in Rust) held by `TurRuntime.poolsHandle` and passed back into `createRuntime` (registers them) and `TurNative.createInstance` / `createHeadlessInstance` (assign `view` / `backend` via `TurAppBuilder::worker_pool`); freed by `TurRuntime.close()` after `destroyRuntime`.
- **Host event pipeline** (no per-plugin Kotlin logic): `PlayerControllerRepository.pluginEvents` → `PluginRepository.bindPlayerEvents` filters by the manifest's `events` list → `bridge.call(BridgeMethods.Plugin.EVENT, ArgPluginEvent(pluginId, type, payload))` → Rust `plugin.event` dispatch → `BackendContext.dispatch_plugin_event` → that plugin's `RpcClient.emit_event` (fire-and-forget on the plugin-event bus channel, `ease_tur_rpc::EVENT_CHANNEL_ID = 1`; RPC control + streams ride channel 0) → JS `hostRpc.onEvent(type, …)`.
- **RPC map**: `BackendContext.service_rpcs: RwLock<HashMap<String /* pluginId */, RpcClient>>` — one entry per live plugin backend, installed by Rust's own `reload_backends` (`spawn_headless` → `load_headless_module` → `extract_rpc` through the `PluginEngineHost` trait; the old `wireServiceRpc`/`unwireServiceRpc` JNI calls are gone). Storage dispatch (`services/storage/mod.rs`) passes the row's `plugin_id` + `plugin_storage_id` straight into `JsStorageBackend` (host scope: `storage:list` / `storage:get`); `bridge/dispatch.rs`'s `oauth.url` / `oauth.exchange` arms resolve the client by the `pluginId` that rode the Kotlin call (slot identity from the `ease.oauth.start` upcall), and `storage_plugin.remove_instance` by the row's `plugin_id`.
- **`ease` host module** (`plugin_runtime/plugin.rs`) exports 7 namespaces: `db` (KV, formerly `ease.storage`), `secret`, `oauth`, `themes`, `rpc`, `context`, `library` (read-only host playlist roster: `library.playlists()` → `[{ id, title, musicIds }]` in playlist order, ids stringified to match `music:play` payloads; not plugin-scoped). All ctx-bound; identity comes from the per-instance `PluginId` data slot — never from JS args. `context.storageId$` is a per-instance `Readable<string|null>` (null = create-mode view, a `plugin_storage_id` = edit view) minted via `PluginContext::reactive()` (tur #189). `context.notifyChange()` triggers a host storage-list reload; `context.removeStorage(id)` deletes the host row (called by a backend to complete its own disconnect). `rpc.call(op, args)` lets a view invoke a handler on its own backend (`service_rpc_for(pluginId)` → `RpcClient::call_view`, reused — no cross-bus relay); it resolves `viewRpc.registerHandler` registrations only. The OAuth flow carries identity only: `oauth.new()` mints an opaque flow id, `oauth.start(oauthId)` fires the flow, and business data (the connect-form alias) stays in the plugin's own KV keyed by the flow id — the host stashes just `(pluginId, oauthId)` across the browser round-trip (`PluginOAuthState`). Type declarations: [`plugins/infra/ease.d.ts`](./plugins/infra/ease.d.ts) + [`plugins/infra/tur-rpc.d.ts`](./plugins/infra/tur-rpc.d.ts).
- **Plugin TS layout**: `src/backend.ts` + `src/view.ts`, rspack entries `{ backend, view }` (one entry per declared contribution view file); `pnpm build` emits into `plugins/<id>/dist/`. **Element construction uses tur's builder pattern** (tur #225, `@tur-ng/std` ≥ 0.0.16): every element ctor takes its *required* props only (or nothing) and returns a chainable builder — `Container().padding(16).children([Text({ text: "hi" }).fontSize(14).build()]).build()`; each prop is a camelCase method, `.children([...])` appends, `.child(el)` sets the single child, `.build()` materializes (renames: `Each.build` → `.itemBuilder`, Table `build`/`buildHeader` → `.rowBuilder`/`.headerBuilder`). The legacy props-object convention is **gone** — migrating old code by hand? tur ships a codemod (`scripts/migrate-builder-codemod.mjs` in the tur repo). Note: `ReadableSubscribe` is **gone** (tur #226 — use `watch(atom, cb)` for non-element subscriptions), and an ellipsizing `Text` label inside a `Row` needs a `Flexible` wrapper (tur #227 — non-flex Row children get an unbounded main axis, as in Flutter). `@tur-ng/net`'s published 0.0.5 pins `@tur-ng/std@0.0.9` (stale pre-builder d.ts that poisons typecheck via its `/// <reference types>`), so the webdav/onedrive `package.json`s carry a `pnpm.overrides` forcing std 0.0.16, and all plugin tsconfigs load tur types via the `types` array (NOT `paths` — paths-mapped ambient modules made resolution order-dependent). **npm deps are allowed** — each plugin is a standalone pnpm project; rspack bundles everything except the `tur:*` / `ease` externals, so prefer popular libraries over hand-rolled code. The runtime is **boa** (no DOM / Node APIs — no `Buffer`, `process`, `btoa`), but the host polyfills the common Web Platform globals: `crypto.getRandomValues` / `crypto.randomUUID` are installed Rust-side from OS entropy ([`plugin_runtime/webapi.rs`](./rust-libs/ease-client-android/src/plugin_runtime/webapi.rs), per instance), `TextEncoder` / `TextDecoder` come from the shared [`plugins/infra/text-polyfill.ts`](./plugins/infra/text-polyfill.ts) (a WHATWG-shaped wrapper over `tur:std`'s `encodeUtf8` / `decodeUtf8`), and [`plugins/infra/string-polyfill.ts`](./plugins/infra/string-polyfill.ts) patches the Annex-B `String.prototype.substr` boa lacks (fast-xml-parser needs it). Every bundle entry imports both polyfill modules first. So packages like `uuid` (and the WebDAV plugin's `fast-xml-parser` / `js-md5` / `js-base64`) work unmodified. Built zips + the committed registry live in [`plugins/registry/`](./plugins/registry/) (`zips/<id>-<version>.zip` + `plugins.json` with sha256/size), produced by `scripts/package-plugin.ts` (`pnpm run build:plugins` packages all plugins and copies the WebDAV + Lyric Formats zips into `assets/plugin-bundles/`).

### Plugin install model (Rust manager)
Plugins are **installable at runtime**, not baked into assets. Installed plugins live as folders under `filesDir/plugins/<id>/` (manifest.json + JS). The install layer lives in **Rust** — [`services/plugin_manager.rs`](./rust-libs/ease-client-backend/src/services/plugin_manager.rs) — reached from Kotlin through `plugin.*` JSON-bridge methods (see `bridge/dispatch.rs`): `list` (scan + module-source registration), `installZipPath` (Kotlin stream-copies a SAF `content://` pick to a cache temp file; only the **path** crosses JNI), `installFromRegistry` (Rust downloads + sha256-verifies + installs via reqwest), `setEnable` / `uninstall` / `bootstrap`, `setLyricParserSelection` (pure preference — returns the updated selection map, never bumps the generation), `registryFetch` / `registryCached` (entries arrive pre-stamped with `installedVersion`/`updateAvailable` — Kotlin never compares versions), and `sourcesList`/`sourceRemember`/`sourceAddCustom`/`sourceRemoveCustom` (a stale persisted `lastSourceUrl` is dropped to null by Rust so the page self-heals to the default preset). Every set-changing mutation triggers `reload_backends` **inside Rust** (teardown → rescan → reload **enabled** backends only) and ends with a `PLUGINS_CHANGED` signal (fire-and-forget upcall through `EaseSignalHost`, fan-out typed as `BackendSignal` in the Kotlin `EaseBackend` facade; `PluginRepository` collects it into a debounced `plugin.list` rescan — the old Kotlin revision collector is gone). Persisted state (`filesDir/plugin-state.json`, legacy-compatible schema) and the per-source registry cache (`filesDir/plugin-registry-cache/<md5>.json`) are Rust-owned. Install validation: manifest at zip root, id `^[A-Za-z0-9._-]+$`, no `..`/absolute/backslash entries, ≤200 entries, ≤20MB; extraction to a staging dir → atomic swap (overwrite = upgrade).
- **First run / ensure-installed** (`plugin.bootstrap`, every service start): Rust installs each bundled zip (`com.ease.webdav`, `com.ease.lyricformats`) read **natively** from `assets/plugin-bundles/` via the NDK `AAssetManager` (the raw pointer is stashed by the `bindPluginRuntime` JNI trampoline — zip bytes never cross JNI) — recorded per-install in `plugin-state.json` (`bundledInstalled`), so adding a new bundled id reaches existing installs on upgrade while a later user uninstall is never re-forced; then, guarded by `firstRunDone`, any plugin referenced by existing storage rows (bundled, else best-effort from the default registry source). Uninstall wipes **all** plugin data — its storage rows (removed via the normal storage-removal cascade, taking their musics/playlist entries/cover blobs with them), its `plugin_kv` store, its scoped `plugin:<id>` secrets and its lyric-parser selection entries — so the "storage removed" rendering only covers legacy/orphaned rows from older versions.
- **Module sources (tur #198 + #199)**: the vendored `turintegration/` Kotlin loads modules **by opaque handle** — `TurInstance.loadModule(sourceHandle: Long)`, `TurView(sourceHandle = …)`, `TurNative.registerModuleSource/releaseModuleSource`. During `plugin.list`, Rust reads each enabled plugin's backend/view JS and registers it on the runtime's shared `ModuleSourceRegistry` (`tur_engine::ModuleSourceRegistry`, engine-owned) — through the `PluginEngineHost::register_source` trait call into the android crate's `TurEngineHost` (`tur_android::ops::with_runtime`). The runtime ↔ backend binding is **instance-attached** and **explicitly managed**: `EaseBackend.start` passes the backend handle into `createRuntime`, which resolves the `BackendContext` and stamps it into every engine instance's plugin state (`PluginBackendCx`, read by the `ease:*` bridge fns — the old `BACKEND_CONTEXT` OnceLock is gone), and `bindPluginRuntime(backendHandle, runtimeHandle, poolsHandle, assets)` attaches a `TurEngineHost` to the backend's plugin-manager state (compare-and-set detached by `unbindPluginRuntime`). A scan with no host attached logs an error and reports it in the `plugin.list` `warnings` — zero handles are never silent. Plugin JS bytes never cross the Kotlin↔Rust boundary. Stale handles (older generations) are safe misses — ids are monotonic.
- **Rust-driven headless backends**: `plugin_manager::reload_backends` (backend crate) is the single lifecycle path — teardown all tracked instances + service RPC entries → scan → for each enabled plugin with a `backend.js`: `spawn_headless` (android crate: JVM thread attach via ndk-context, construct the Kotlin `FrameLoop` from the cached class ref, `tur_android::ops::create_instance` on the `ease-plugin-backend` pool, wire the loop's pump callbacks through a zeroable `AtomicLong` cell — exact `TurInstance` semantics so a late wake after close reads 0 and no-ops) → `load_headless_module` → `extract_rpc` (`with_app` round-trip; FIFO settles behind spawn + load) → `cx.set_service_rpc`. Triggers: the engine bind + every set-changing mutation; serialized by a reload lock; `unbindPluginRuntime` CAS-detaches then tears down everything before `destroyRuntime`.
- **Distribution**: [`plugins/registry/plugins.json`](./plugins/registry/plugins.json) (committed; served from the repo via jsDelivr/GitHub Raw) is fetched Rust-side — source presets (jsDelivr / fastly / gcore / GitHub Raw, constants in `plugin_manager.rs`, pinned to the `main` ref) + user-added custom sources (verified by fetching `plugins.json`, then saved). Registry lists are cached per source.
- **UI**: Settings → 通用 → 插件管理 (`PluginManagementPage`): installed rows (enable `Switch`, uninstall w/ storage-count warning) + 从 ZIP 安装 (SAF). Its top-right button pushes `AvailablePluginsPage` (`获取插件`): source picker dialog (presets + saved customs + 自定义源 verify dialog), registry rows with 安装/更新 (Rust-stamped), offline cache fallback. Dashboard shows one **entry card** per `contributions.dashboard` item; tapping pushes `PluginViewPage` (full-screen standalone TurView, loaded by source handle).

### ViewModels
`@HiltViewModel` extending `androidx.lifecycle.ViewModel`, using `viewModelScope`. UI state classes are co-located (e.g. `PlaylistsState`, `SleepModeState`, `PlaylistsMode` enum). Example: `PlayerVM` polls playback position every 1 s.

## Build & run commands

### Prerequisites
- **JDK 21** (CI uses Zulu 21).
- **Rust stable** + `cargo-ndk@3.5.4` + `rustup target add aarch64-linux-android` + `cargo-nextest`.
- **Android SDK** (compileSdk 35 / minSdk 29 / targetSdk 34) + **NDK r27c** with `ANDROID_NDK_HOME` set.
- **pnpm** for running the TypeScript scripts.
- The JNI build is a pure `cargo ndk build -p ease-client-android` (no bindgen/host-dylib step). The cdylib name `libease_client_android.so` must stay in sync with `System.loadLibrary("ease_client_android")` (declared once in `EaseMusicPlayerApplication`).

### Commands (run via `pnpm` from repo root)
| Command | What it does |
|---|---|
| `pnpm build:jni` | `cargo ndk` cross-compile `ease-client-android` (`arm64-v8a`, release) into `android/app/src/main/jniLibs/` (single cdylib `libease_client_android.so`; strays filtered out). |
| `pnpm build:apk` | `EBUILD=1 build:jni` + `:app:assembleRelease` + copy APK to `artifacts/apk/`. Requires `ANDROID_SIGN_JKS` (brotli + base64) and `ANDROID_SIGN_PASSWORD` secrets. |
| `pnpm build:plugins` | rspack-build every plugin into `plugins/<id>/dist/` → package registry zips + `plugins/registry/plugins.json` (sha256/size) → copy the WebDAV + Lyric Formats zips into `assets/plugin-bundles/`. |
| `pnpm test` | `cd rust-libs && cargo nextest run` (Rust tests). |

### Gradle tasks (run from `android/`)
- `cd android && ./gradlew :app:assembleDebug` — debug APK.
- `cd android && ./gradlew :app:assembleRelease` — release APK (after JNI libs are present).

### Generated / gitignored artifacts
These are **not checked in** and must be regenerated before a clean checkout will compile:
- `android/app/src/main/jniLibs/arm64-v8a/` — Android native lib (via `pnpm build:jni`).
- `assets/plugin-bundles/com.ease.webdav.zip` + `assets/plugin-bundles/com.ease.lyricformats.zip` + `plugins/<id>/dist/` — plugin build outputs (via `pnpm build:plugins`; the registry zips + `plugins.json` under `plugins/registry/` **are** committed).

## Conventions

- **Kotlin code style**: `official` (per `android/gradle.properties`). No license headers. No ktlint / detekt.
- **Class naming**:
  - `*Repository` — data layer (under `singleton/`).
  - `*Controller` — platform / interaction (under `singleton/`).
  - `*VM` — `@HiltViewModel` ViewModels (under `viewmodels/`).
- **UI layout**: Compose screens under `widgets/<feature>/`; reusable composables under `components/`. Route helpers are top-level functions in `core/Routes.kt` (under `singleton/`).
- **JSON bridge naming**: backend functions use `ct*` (controller) and `cts*` (controller service) prefixes; argument structs are prefixed `Arg*`.
- **Resources**: Android resources under `android/app/src/main/res/`; Compose resources under `android/app/src/main/assets/composeResources/`.
- **Branch naming**: `feat/v<version>` (e.g. `feat/v0.4`). PRs target `main`.
- **Commit messages**: semantic — `feat:` / `fix:` / `refactor:` / `chore:` / `test:` / `docs:`.
- **Release tags**: `vX.Y.Z` and `pre-vX.Y.Z-beta.N` (trigger the APK build/release CI).
- **ProGuard** (`android/app/proguard-rules.pro`): `-dontwarn` for AWT classes (legacy from the KMP experiment; harmless on Android); `-keepattributes LineNumberTable,SourceFile`. (The `uniffi.**`/`com.sun.jna.**` keeps are vestiges of the dropped UniFFI seam — harmless.)

## On-device verification

- **Device + adb**: follow the `android-dev` skill. The wireless adb link is flaky — reconnect before each call (the reliable pattern is a small `SH()` wrapper that `disconnect`→`connect`→sleep→runs the command). The device's wireless-debugging port changes when its adbd restarts — if the proxy target port stops accepting, check the port on the device (or use port `5555` if `adb tcpip` is enabled) and restart the proxy. A **dozing device** drops the link mid-transfer and produces empty screencaps — `input keyevent KEYCODE_WAKEUP; wm dismiss-keyguard` first.
- **Installing**: MIUI rejects silent installs — use `adb push` + `pm install -r` (run `pm install` detached via `nohup … > /data/local/tmp/install.log 2>&1 &` and poll `lastUpdateTime` in `dumpsys package` — the install outlives the stable link window). Large pushes (>40 MB) reliably die mid-transfer; **split the APK into 2–5 MB chunks** (`split -b 5m`), push each with a reconnect, `cat` on device, then install. Screencaps are **physical pixels** (1440×3200 on the test device, Mi 11); `uiautomator dump` bounds are exact — prefer them over screenshot-based estimates for Compose UI. tur-rendered plugin views don't expose text to `uiautomator dump` (only the Compose top bar does), so tap targets inside them must be found from a screenshot, not a UI dump.
- **Analyzing screenshots / driving the device: ALWAYS delegate to the `operator` subagent** — it is multimodal (reads images directly) and can run adb itself. Do not hand-parse pixels with PIL/Python scripts. Give it full context in the prompt: screen size + dpr, what the screen should show, the colors/labels of the elements of interest, and the precise question (e.g. "is box X to the left or right of target Y, and is it clipped?"). For exact geometry, ask it for bounding boxes of distinctly-colored solid-fill elements (unique colors are easiest to measure); treat text-only estimates as ±tens of px.

## Gotchas

- **Gradle root is at `android/`, not the repo root.** All `gradlew` invocations must `cd android/` first (the `build-apk.ts` script does this automatically).
- **Android `arm64-v8a` only** — no x86 / armeabi targets.
- **`local.properties`** (repo root) currently contains `sdk.dir=/usr/local/share/android-commandlinetools`, a developer-specific path. Adjust to point at your local Android SDK before building, or place a `local.properties` under `android/`.
- **SQLite is force-bundled** (`libsqlite3-sys` `bundled` feature) for cross-compilation.
- **`.opencode/agents/git-end.md`** references `scripts/local_ci.cjs`, which does **not** currently exist in the tree.
- **License split**: the majority of this project is GPL-3.0; [`rust-libs/ease-order-key`](./rust-libs/ease-order-key) is dual-licensed MIT OR Apache-2.0. `rust-libs/ease-remote-storage` and `android/` each ship their own `LICENSE-GPL`.
- **Version**: `android/app/build.gradle.kts` is the single source of truth for `versionName` (`0.4.0-beta.0` at the time of writing). The pre-0.4 `platformAppVersion()` desktop gotcha is gone.
