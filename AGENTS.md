# AGENTS.md

Guide for coding agents working in this repository. Read this first.

## Project overview

Ease Music Player is a lightweight **Android** music player written in **Kotlin / Jetpack Compose** (UI) and **Rust** (backend). It targets Android `arm64-v8a` only.

Features: WebDAV and OneDrive cloud storage, playlist-based playback, music cover art, lyrics.

> **History note (0.3 → 0.4):** version 0.4 briefly migrated the UI to Kotlin Multiplatform / Compose Multiplatform with a Desktop JVM target (JavaFX `MediaPlayer` + Skiko). The desktop build was dropped for 0.4.0-beta.0 — memory overhead (~half a GB at idle, mostly from loading two rendering stacks) and lack of user-facing benefit made the single-target Android app the better shape. The Rust-side improvements from that era are kept (`ease-client-schema` / `ease-client-migration` crate split, UniFFI tokio routing). See [`docs/motivation.md`](./docs/motivation.md).

## Architecture at a glance

```
┌──────────────────────────────┐        UniFFI (JNA)         ┌──────────────────────────────┐
│  Kotlin / Jetpack Compose    │ ──────────────────────────▶ │  Rust workspace (rust-libs/) │
│  android/app/  (Gradle :app) │                            │  ease-client-backend         │
│                              │ ◀────────────────────────── │  (cdylib: libease_client_*)  │
│  Hilt DI, MediaSessionCompat │   StateFlow / SharedFlow    │  + Sea-ORM / SQLite          │
│                              │   via repositories          │  + cantode audio engine      │
└──────────────────────────────┘                            └──────────────────────────────┘
```

- **Rust side** ([`rust-libs/`](./rust-libs/)) exposes a UniFFI `Backend` object as a `cdylib`. It owns the database (SQLite via Sea-ORM), business logic, and controllers/services/repositories. Audio decode + output live in the separate [`cantode/`](./cantode/) repo-root crate (symphonia + cpal/AAudio), linked into the same `.so`.
- **Kotlin side** ([`android/app/`](./android/app/)) talks to the backend through [`singleton/Bridge.kt`](./android/app/src/main/java/com/kutedev/easemusicplayer/singleton/Bridge.kt), which wraps the generated UniFFI bindings and exposes suspend + sync helpers.
- **Playback**: [`cantode`](./cantode/) (Rust audio engine) decodes (symphonia: mp3/flac/vorbis/ogg/wav/aac/isomp4) and renders via cpal's AAudio backend, exposing a `PlayerHandle` over UniFFI. [`CantodeEngine`](./android/app/src/main/java/com/kutedev/easemusicplayer/core/CantodeEngine.kt) wraps the handle and polls state at ~10 Hz. [`PlaybackService`](./android/app/src/main/java/com/kutedev/easemusicplayer/core/MusicPlayer.kt) is a plain `android.app.Service` (no longer `MediaSessionService`) that owns a `MediaSessionCompat` from `androidx.media:media` for notification / lock-screen / Bluetooth / Auto integration. No media3 / ExoPlayer dependency remains.

## Repository layout

| Path | Purpose |
|---|---|
| [`android/`](./android/) | **Gradle root** of the Android project: `settings.gradle.kts`, `build.gradle.kts`, `gradle/`, `gradlew*`, `gradle.properties`, `gradle/libs.versions.toml`. |
| [`android/app/`](./android/app/) | The `:app` Gradle module — the Android application (Kotlin + Compose + Hilt + MediaSessionCompat). |
| [`rust-libs/`](./rust-libs/) | Cargo workspace of Rust crates (backend, schema, migration, FFI builder, etc.). |
| [`scripts/`](./scripts/) | TypeScript build/test orchestration (run via `pnpm`/`tsx`). |
| [`docs/`](./docs/) | `motivation.md` + screenshots. |
| [`.github/workflows/`](./.github/workflows/) | CI: build JNI, run Rust tests, build APK on release tags. |
| [`.opencode/`](./.opencode/) | OpenCode agent config (subagents for git/PR finalization, image reading). |

Root Java package: `com.kutedev.easemusicplayer`. Namespace / applicationId: `com.kutedev.easemusicplayer`.

## Kotlin source layout (`android/app/src/main/java/com/kutedev/easemusicplayer/`)

- `MainActivity.kt` — `@AndroidEntryPoint` `ComponentActivity`; also declares the top-level `@HiltAndroidApp class EaseMusicPlayerApplication`. Hosts `setContent { Root() }`, requests permissions (notably `POST_NOTIFICATIONS` for the playback foreground service), and runs the startup reload sequence.
- `Root.kt` — main `@Composable` (`NavHost`, routes, theme).
- `core/`
  - `MusicPlayer.kt` — `PlaybackService` (plain `android.app.Service` owning a `MediaSessionCompat` for system integration). `@AndroidEntryPoint`.
  - `KeepBackendService.kt` — foreground service that keeps the Rust backend process alive AND hosts the plugin backends: one headless tur instance per plugin manifest declaring a `backend` field (`createHeadlessInstance(pluginId)` + `loadModule(backend.js)` + `wireServiceRpc(handle, pluginId)`). `@AndroidEntryPoint`.
  - `CantodeEngine.kt` — Kotlin wrapper around a cantode `PlayerHandle` (Rust audio engine over UniFFI). Owns the 10 Hz state-poll loop, surfaces state via `@Volatile` fields + an `endedEvent` `SharedFlow`.
  - `CoroutineScopeModule.kt` — Hilt `@Module` providing the app-wide `CoroutineScope` (`SupervisorJob + Dispatchers.Default`).
- `singleton/` — `Bridge` + repositories (see [Key patterns](#key-patterns)).
- `viewmodels/` — `@HiltViewModel` ViewModels (`PlayerVM`, `PlaylistsVM`, `PlaylistVM`, `AssetVM`, `CreatePlaylistVM`, `EditPlaylistVM`, `EditStorageVM`, `ImportVM`, `StoragesVM`, `SleepModeVM`, `LogVM`, `DebugMoreVM`, `ToastVM`).
- `widgets/` — Compose screens organized by feature (`appbar/`, `dashboard/`, `devices/`, `home/`, `musics/`, `playlists/`, `settings/`, plus `ToastWidget.kt`).
- `components/` — reusable composables (Checkbox, ConfirmDialog, Form, MusicCover, ...).
- `ui/theme/` — `Color.kt`, `Theme.kt`, `Type.kt`.
- `utils/` — `Duration.kt`, etc.
- `uniffi/ease_client_backend/`, `uniffi/ease_client_schema/` — **generated** UniFFI Kotlin bindings (gitignored; produced by [`pnpm build:jni`](#build--run-commands)).

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
| `ease-client-backend` | Main backend; `crate-type = ["cdylib", "rlib"]`, GPL-3.0. Provides the `Backend` UniFFI object, controllers/services/repositories. Source: `controllers/`, `services/`, `repositories/`, `objects/`, `ctx.rs`, `infra.rs`. |
| `ease-client-schema` | Sea-ORM entities, models, domain types. Exposes UniFFI-compatible schema types. |
| `ease-client-migration` | DB migration from legacy `redb` format to SQLite. Versioned upgraders in `src/legacy/` (`redb_v2`, `redb_v3`, `schema_v2`, `schema_v3`, `upgrader_v1_v2`, `upgrader_v2_v3`). Integration tests in `tests/`. |
| `ease-client-tokio` | Shared tokio multi-thread runtime accessor (`tokio_runtime()`). |
| `ease-client-android-ffi-builder` | Binary wrapping `uniffi bindgen` to generate the Kotlin bindings used by `build-jni-libs.ts`. |
| `ease-order-key` | Standalone orderable-key utility. **Dual MIT OR Apache-2.0 license** (different from the rest). |
| `ease-remote-storage` (path dep, not a workspace member) | WebDAV / OneDrive remote storage client (`reqwest`, `quick-xml`). GPL-3.0. |
| [`cantode/`](./cantode/) (repo root, **not** in `rust-libs/` workspace) | Standalone cross-platform audio engine: symphonia decode + cpal/AAudio output behind a trait-based API. Exposes `PlayerHandle` over UniFFI to the Android app; linked into the same `.so` as `ease-client-backend`. Edition 2024, **dual MIT OR Apache-2.0 license** (matches `ease-order-key`, different from the GPL-3.0 main app). |

Notable Rust constraints: UniFFI pinned to `=0.28.3` with the `tokio` feature; SQLite is force-bundled (`libsqlite3-sys` `bundled`) for cross-compilation; Sea-ORM 1.1 with sqlx-sqlite + runtime-tokio-rustls.

## Key patterns

### Dependency injection (Hilt)
- [`core/CoroutineScopeModule.kt`](./android/app/src/main/java/com/kutedev/easemusicplayer/core/CoroutineScopeModule.kt) — Hilt `@Module` (`SingletonComponent`) providing the app `CoroutineScope` (`SupervisorJob + Dispatchers.Default`).
- Repositories & `Bridge` are `@Inject constructor`-annotated `class`es; Hilt constructs them automatically. ViewModels are `@HiltViewModel` with `@Inject constructor`.
- `EaseMusicPlayerApplication` (top-level class in `MainActivity.kt`) is annotated `@HiltAndroidApp`. Activities/services use `@AndroidEntryPoint` + `@Inject lateinit var`.

### Bridge / FFI
[`singleton/Bridge.kt`](./android/app/src/main/java/com/kutedev/easemusicplayer/singleton/Bridge.kt) is a thin singleton wrapping the UniFFI `Backend`:
- `initialize()` / `destroy()` — create / dispose the backend.
- `run { backend -> ... }` — suspend, **swallows** exceptions and returns `null`.
- `runRaw { ... }` — suspend, **propagates** exceptions.
- `runSync { ... }` / `runSyncRaw { ... }` — non-suspend variants.

Repositories call backend functions through `bridge.run { }`. Backend function prefixes: `ct*` (controller), `cts*` (controller service). Argument structs are prefixed `Arg*` (e.g. `ArgUpsertStorage`, `ArgCreatePlaylist`, `ArgRemoveMusicFromPlaylist`). Domain types live in `ease-client-schema` and surface in Kotlin under `uniffi.ease_client_backend` (re-exported) and `uniffi.ease_client_schema`. A few types — notably `PlayMode`, `DataSourceKey`, `MusicId`, `PlaylistId`, `StorageId` — are importable only from `uniffi.ease_client_schema`.

The Rust backend spawns work on the shared tokio runtime via `ease_client_tokio::tokio_runtime()`; UniFFI async FFI controllers route through it (`tokio_runtime().handle().spawn(...).await`).

### Startup sequence
`MainActivity.onStart()` launches a `lifecycleScope` coroutine that calls `reload()` on `playerRepository`, `storageRepository`, `playlistRepository` (in that order). `PlaybackService` is started lazily on first play via `PlayerControllerRepository` (it owns the `MediaSessionCompat`); `MainActivity` no longer wires a `MediaController`. `Bridge.initialize()` is called earlier in `MainActivity.onCreate()` (after starting `KeepBackendService`).

### Repository pattern
All in `singleton/`, constructed with `Bridge` + `CoroutineScope`, expose `StateFlow` / `SharedFlow`:
- `PlayerRepository` — current music/playlist, play mode, derived `previousMusic` / `nextMusic` / `onCompleteMusic` flows, pause requests.
- `PlaylistRepository` — playlist list (debounced reload), reorder via `ease-order-key`, reacts to storage-removal events.
- `StorageRepository` — cloud storage list, OAuth refresh token, remove events.
- `AssetRepository` — in-memory cache for cover art bytes + decoded bitmaps.
- `PluginRepository` — plugin registry: `scanPlugins()` walks `assets/plugins/<id>/manifest.json` and publishes `enabledPlugins` / `pluginViews` / `storageProviders`; `bindPlayerEvents()` forwards player events to JS backends (see [Plugin system](#plugin-system-js-plugins)).
- `PlayerControllerRepository`, `PermissionRepository`, `ImportRepository`, `ToastRepository`.

### Plugin system (JS plugins)
Plugins live under [`plugins/`](./plugins/) (TS sources, rspack bundles) and ship into `android/app/src/main/assets/plugins/<id>/` (`pnpm run build` per plugin; it also copies `manifest.json`). Manifest schema:

```json
{
  "id": "com.ease.onedrive",
  "backend": "backend.js",                    // optional; long-lived module (headless tur instance)
  "events": ["music:play"],
  "contributions": {
    "storages": [{ "id": "onedrive", "view": "view.js" }],   // per-storage config view (short-lived)
    "views":    [{ "id": "main", "title": "…", "view": "view.js" }]
  }
}
```

- **`backend`** — one per plugin; all contributions share it. Loaded once by `KeepBackendService` into a headless tur instance stamped with `PluginId`; registers `tur:rpc` handlers (`registerHandler`) for storage ops (`onedrive:list` etc.) and/or event subscriptions (`music:play` in `com.ease.playcount`'s backend).
- **`view`** — per contribution; loaded into a `TurView` when the page opens (add-storage form, plugin page) and destroyed on leave.
- **Host event pipeline** (no per-plugin Kotlin logic): `PlayerControllerRepository.pluginEvents` → `PluginRepository.bindPlayerEvents` filters by the manifest's `events` list → `bridge.call(BridgeMethods.Plugin.EVENT, ArgPluginEvent(pluginId, type, payload))` → Rust `plugin.event` dispatch → `BackendContext.dispatch_plugin_event` → that plugin's `RpcClient.call(type, payload)` → JS `registerHandler(type, …)`.
- **RPC map**: `BackendContext.service_rpcs: RwLock<HashMap<String /* pluginId */, RpcClient>>` — one entry per backend, installed by the `wireServiceRpc(handle, pluginId)` JNI call (`ctx.rs`). Storage dispatch (`services/storage/mod.rs`) resolves the client by the storage row's `plugin_id`.
- **`ease` host module** (`plugin_runtime/plugin.rs`) exports 5 namespaces: `db` (KV, formerly `ease.storage`), `secret`, `oauth`, `themes`, `context`. All ctx-bound; identity comes from the per-instance `PluginId` data slot — never from JS args. Type declarations: [`plugins/infra/ease.d.ts`](./plugins/infra/ease.d.ts) + [`plugins/infra/tur-rpc.d.ts`](./plugins/infra/tur-rpc.d.ts).
- **Plugin TS layout**: `src/backend.ts` + `src/view.ts`, rspack entries `{ backend, view }`. Built bundles + copied `manifest.json` are gitignored.

### ViewModels
`@HiltViewModel` extending `androidx.lifecycle.ViewModel`, using `viewModelScope`. UI state classes are co-located (e.g. `PlaylistsState`, `SleepModeState`, `PlaylistsMode` enum). Example: `PlayerVM` polls playback position every 1 s.

## Build & run commands

### Prerequisites
- **JDK 21** (CI uses Zulu 21).
- **Rust stable** + `cargo-ndk@3.5.4` + `rustup target add aarch64-linux-android` + `cargo-nextest`.
- **Android SDK** (compileSdk 35 / minSdk 29 / targetSdk 34) + **NDK r27c** with `ANDROID_NDK_HOME` set.
- **pnpm** for running the TypeScript scripts.
- **Windows hosts only**: `build-jni-libs.ts` resolves the host cdylib as `ease_client_backend.dll` (the binding-generator step runs against the host build, not the cross-compiled `.so`). Linux/macOS hosts use `libease_client_backend.{so,dylib}`.

### Commands (run via `pnpm` from repo root)
| Command | What it does |
|---|---|
| `pnpm build:jni` | `cargo build -p ease-client-backend` (host debug) → regenerate UniFFI Kotlin bindings into `android/app/src/main/java/` → `cargo ndk` cross-compile `arm64-v8a` release into `android/app/src/main/jniLibs/`. |
| `pnpm build:apk` | `EBUILD=1 build:jni` + `:app:assembleRelease` + copy APK to `artifacts/apk/`. Requires `ANDROID_SIGN_JKS` (brotli + base64) and `ANDROID_SIGN_PASSWORD` secrets. |
| `pnpm test` | `cd rust-libs && cargo nextest run` (Rust tests). |

### Gradle tasks (run from `android/`)
- `cd android && ./gradlew :app:assembleDebug` — debug APK.
- `cd android && ./gradlew :app:assembleRelease` — release APK (after JNI libs are present).

### Generated / gitignored artifacts
These are **not checked in** and must be regenerated (via `pnpm build:jni`) before a clean checkout will compile:
- `android/app/src/main/java/uniffi/` — UniFFI Kotlin bindings.
- `android/app/src/main/jniLibs/arm64-v8a/` — Android native lib.

## Conventions

- **Kotlin code style**: `official` (per `android/gradle.properties`). No license headers. No ktlint / detekt.
- **Class naming**:
  - `*Repository` — data layer (under `singleton/`).
  - `*Controller` — platform / interaction (under `singleton/`).
  - `*VM` — `@HiltViewModel` ViewModels (under `viewmodels/`).
- **UI layout**: Compose screens under `widgets/<feature>/`; reusable composables under `components/`. Route helpers are top-level functions in `core/Routes.kt` (under `singleton/`).
- **UniFFI naming**: backend functions use `ct*` (controller) and `cts*` (controller service) prefixes; argument structs are prefixed `Arg*`.
- **Resources**: Android resources under `android/app/src/main/res/`; Compose resources under `android/app/src/main/assets/composeResources/`.
- **Branch naming**: `feat/v<version>` (e.g. `feat/v0.4`). PRs target `main`.
- **Commit messages**: semantic — `feat:` / `fix:` / `refactor:` / `chore:` / `test:` / `docs:`.
- **Release tags**: `vX.Y.Z` and `pre-vX.Y.Z-beta.N` (trigger the APK build/release CI).
- **ProGuard** (`android/app/proguard-rules.pro`): keeps `uniffi.**` and `com.sun.jna.**`; `-dontwarn` for AWT classes (legacy from the KMP experiment; harmless on Android); `-keepattributes LineNumberTable,SourceFile`.

## On-device verification

- **Device + adb**: follow the `android-dev` skill. The wireless adb link is flaky — reconnect before each call (the reliable pattern is a small `SH()` wrapper that `disconnect`→`connect`→sleep→runs the command). The device's wireless-debugging port changes when its adbd restarts — if the proxy target port stops accepting, check the port on the device (or use port `5555` if `adb tcpip` is enabled) and restart the proxy. A **dozing device** drops the link mid-transfer and produces empty screencaps — `input keyevent KEYCODE_WAKEUP; wm dismiss-keyguard` first.
- **Installing**: MIUI rejects silent installs — use `adb push` + `pm install -r` (run `pm install` detached via `nohup … > /data/local/tmp/install.log 2>&1 &` and poll `lastUpdateTime` in `dumpsys package` — the install outlives the stable link window). Large pushes (>40 MB) reliably die mid-transfer; **split the APK into 2–5 MB chunks** (`split -b 5m`), push each with a reconnect, `cat` on device, then install. Screencaps are **physical pixels** (1440×3200 on the test device, Mi 11); `uiautomator dump` bounds are exact — prefer them over screenshot-based estimates for Compose UI. tur-rendered plugin views don't expose text to `uiautomator dump` (only the Compose top bar does), so tap targets inside them must be found from a screenshot, not a UI dump.
- **Analyzing screenshots: ALWAYS delegate to the `image-reader` subagent** via the Task tool — do not hand-parse pixels with PIL/Python scripts. Give it full context in the prompt: screen size + dpr, what the screen should show, the colors/labels of the elements of interest, and the precise question (e.g. "is box X to the left or right of target Y, and is it clipped?"). For exact geometry, ask it for bounding boxes of distinctly-colored solid-fill elements (unique colors are easiest to measure); treat text-only estimates as ±tens of px.

## Gotchas

- **Gradle root is at `android/`, not the repo root.** All `gradlew` invocations must `cd android/` first (the `build-apk.ts` script does this automatically).
- **Android `arm64-v8a` only** — no x86 / armeabi targets.
- **`local.properties`** (repo root) currently contains `sdk.dir=/usr/local/share/android-commandlinetools`, a developer-specific path. Adjust to point at your local Android SDK before building, or place a `local.properties` under `android/`.
- **SQLite is force-bundled** (`libsqlite3-sys` `bundled` feature) for cross-compilation.
- **UniFFI is pinned** to `=0.28.3` with the `tokio` feature.
- **`.opencode/agents/git-end.md`** references `scripts/local_ci.cjs`, which does **not** currently exist in the tree.
- **License split**: the majority of this project is GPL-3.0; [`rust-libs/ease-order-key`](./rust-libs/ease-order-key) is dual-licensed MIT OR Apache-2.0. `rust-libs/ease-remote-storage` and `android/` each ship their own `LICENSE-GPL`.
- **Version**: `android/app/build.gradle.kts` is the single source of truth for `versionName` (`0.4.0-beta.0` at the time of writing). The pre-0.4 `platformAppVersion()` desktop gotcha is gone.
