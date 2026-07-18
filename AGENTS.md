# AGENTS.md

Guide for coding agents working in this repository. Read this first.

## Project overview

Ease Music Player is a lightweight music player written in **Rust** (backend) and **Kotlin Multiplatform / Compose Multiplatform** (UI). It targets **Android** (`arm64-v8a`) and **Desktop JVM** (Windows, macOS, Linux).

Features: WebDAV and OneDrive cloud storage, playlist-based playback, music cover art, lyrics.

## Architecture at a glance

```
┌──────────────────────────────┐        UniFFI (JNA)        ┌──────────────────────────────┐
│  Kotlin / Compose Multiplat. │ ─────────────────────────▶ │  Rust workspace (rust-libs/) │
│  composeApp/                 │                            │  ease-client-backend         │
│                              │ ◀───────────────────────── │  (cdylib: libease_client_*)  │
│  commonMain / jvmShared      │   StateFlow / SharedFlow   │  + axum streaming server     │
│  + androidMain / desktopMain │   via repositories         │  + Sea-ORM / SQLite          │
└──────────────────────────────┘                            └──────────────────────────────┘
```

- **Rust side** ([`rust-libs/`](./rust-libs/)) exposes a UniFFI `Backend` object as a `cdylib`. It owns the database (SQLite via Sea-ORM), business logic, and an axum HTTP streaming server bound to an OS-assigned port (`http://127.0.0.1:<port>/music/:id`, range requests supported for seeking).
- **Kotlin side** ([`composeApp/`](./composeApp/)) talks to the backend through [`singleton/Bridge.kt`](./composeApp/src/jvmShared/kotlin/com/kutedev/easemusicplayer/singleton/Bridge.kt), which wraps the generated UniFFI bindings and exposes suspend + sync helpers.
- **Two player implementations** behind a shared [`PlayerController`](./composeApp/src/jvmShared/kotlin/com/kutedev/easemusicplayer/singleton/PlayerController.kt) interface:
  - Android: media3 `ExoPlayer` via [`PlaybackService`](./composeApp/src/androidMain/kotlin/com/kutedev/easemusicplayer/core/MusicPlayer.kt) (a `MediaSessionService`).
  - Desktop: JavaFX `MediaPlayer` ([`DesktopPlayerController`](./composeApp/src/desktopMain/kotlin/com/kutedev/easemusicplayer/singleton/DesktopPlayerController.kt)) streaming from the Rust axum server.

See [`docs/motivation.md`](./docs/motivation.md) for the evolution history (Flutter 0.1 → native Compose 0.2 → Rust-backend-only 0.3 → KMP/Compose Multiplatform + Rust axum streaming 0.4).

## Repository layout

| Path | Purpose |
|---|---|
| [`composeApp/`](./composeApp/) | The single Gradle module: Kotlin Multiplatform app (Android + Desktop JVM). |
| [`rust-libs/`](./rust-libs/) | Cargo workspace of Rust crates (backend, schema, migration, FFI builder, etc.). |
| [`scripts/`](./scripts/) | TypeScript build/run/test orchestration (run via `pnpm`/`tsx`). |
| [`docs/`](./docs/) | `motivation.md` + screenshots. |
| [`android/`](./android/) | Placeholder dir (just `LICENSE-GPL` + `.gitignore`). |
| [`gradle/`](./gradle/) | `libs.versions.toml` version catalog + wrapper. |
| [`.github/workflows/`](./.github/workflows/) | CI: build JNI, run Rust tests, build APK + desktop distributions on release tags. |
| [`.opencode/`](./.opencode/) | OpenCode agent config (subagents for git/PR finalization, image reading). |

Root package: `com.kutedev.easemusicplayer`. Namespace / applicationId: `com.kutedev.easemusicplayer`.

## Source-set hierarchy (`composeApp/src/`)

```
commonMain ── expect decls, theme, shared resources, utils
   │
   └── jvmShared ── bulk of the app; depends on commonMain; uses JNA + UniFFI
          ├── androidMain ── Android-only; depends on jvmShared
          └── desktopMain ── Desktop-only; depends on jvmShared
                 └── desktopTest
```

### `commonMain`
[`composeApp/src/commonMain/kotlin/com/kutedev/easemusicplayer/`](./composeApp/src/commonMain/kotlin/com/kutedev/easemusicplayer/)
- `platform/Platform.kt` — `expect` declarations + `AppPaths` data class (`documentDir`, `cacheDir`).
- `platform/BackHandler.kt` — `expect` composable.
- `ui/theme/Color.kt`, `ui/theme/Type.kt`.
- `utils/ByteQueue.kt`, `utils/Tick.kt`.
- Resources: `composeResources/drawable/`, `composeResources/font/noto_sans.ttf`, `composeResources/values/strings.xml` + `values-zh-rCN/strings.xml` (English + Simplified Chinese).

### `jvmShared` (the bulk)
[`composeApp/src/jvmShared/kotlin/com/kutedev/easemusicplayer/`](./composeApp/src/jvmShared/kotlin/com/kutedev/easemusicplayer/)
- `Root.kt` — main `@Composable` (`NavHost`, routes, theme). The real UI root (the `App.kt` in commonMain is a placeholder).
- `core/Routes.kt` — navigation route helpers (`RouteHome()`, `RoutePlaylist("{id}")`, ...).
- [`di/AppModule.kt`](./composeApp/src/jvmShared/kotlin/com/kutedev/easemusicplayer/di/AppModule.kt) — Koin shared module (`appModule`).
- [`lifecycle/AppLifecycle.kt`](./composeApp/src/jvmShared/kotlin/com/kutedev/easemusicplayer/lifecycle/AppLifecycle.kt) — startup hook.
- `singleton/` — repositories + `Bridge` + `PlayerController` interface (see [Key patterns](#key-patterns)).
- `viewmodels/` — ViewModels (`PlayerVM`, `PlaylistsVM`, `PlaylistVM`, `AssetVM`, `CreatePlaylistVM`, `EditPlaylistVM`, `EditStorageVM`, `ImportVM`, `StoragesVM`, `SleepModeVM`, `LogVM`, `DebugMoreVM`, `ToastVM`).
- `widgets/` — Compose screens organized by feature (`appbar/`, `dashboard/`, `devices/`, `home/`, `musics/`, `playlists/`, `settings/`, plus `ToastWidget.kt`).
- `components/` — reusable composables (Checkbox, ConfirmDialog, Form, MusicCover, ...).
- `ui/theme/Theme.kt`, `utils/Duration.kt`.
- `uniffi/ease_client_backend/`, `uniffi/ease_client_schema/` — **generated** UniFFI Kotlin bindings (gitignored; produced by [`pnpm build:jni`](#build--run-commands)).

### `androidMain`
[`composeApp/src/androidMain/`](./composeApp/src/androidMain/)
- `AndroidManifest.xml` — declares `MainActivity`, `PlaybackService` (media3 session), `KeepBackendService`, OAuth2 redirect (`easem://oauth2redirect`), `FileProvider`.
- `kotlin/.../MainActivity.kt`, `EaseMusicPlayerApplication.kt` (starts Koin with `appModule + androidModule`).
- `core/MusicPlayer.kt` (`PlaybackService` + media3 wiring), `core/KeepBackendService.kt`, `core/MusicPlayerDataSource.kt`, `core/MusicPlayerUtil.kt`.
- `di/AndroidModule.kt` — Koin module binding `PlayerControllerRepository`, `PermissionRepository`, `AppPaths`.
- `platform/Platform.android.kt`, `platform/BackHandler.android.kt` — `actual` implementations.
- `singleton/PermissionRepository.kt`, `singleton/PlayerControllerRepository.kt`.
- `res/` — Android resources (mipmaps, values, xml); `jniLibs/arm64-v8a/` (gitignored, generated).

### `desktopMain`
[`composeApp/src/desktopMain/`](./composeApp/src/desktopMain/)
- `kotlin/.../Main.kt` — `fun main()`: sets `jna.library.path`, starts Koin (`appModule + desktopModule`), creates the Compose `Window`, loads the app icon, installs `TrayController`.
- `di/DesktopModule.kt` — binds `DesktopPlayerController` (JavaFX `MediaPlayer`), `DesktopPermissionManager`, `AppPaths` (`~/.ease-music-player/`).
- `platform/Platform.desktop.kt`, `platform/BackHandler.desktop.kt` — `actual` implementations.
- `platform/TrayController.kt` — AWT `SystemTray` integration (show / play-pause / quit).
- `singleton/DesktopPlayerController.kt`, `singleton/DesktopStubs.kt`.
- `resources/` — `ic_launcher.png` + `natives/` (gitignored; host `.so`/`.dylib`/`.dll` copied here by `build-jni-libs.ts`).

### `desktopTest`
[`composeApp/src/desktopTest/kotlin/com/kutedev/easemusicplayer/`](./composeApp/src/desktopTest/kotlin/com/kutedev/easemusicplayer/)
- JUnit4 + Compose `uiTest`. Stands up a real Koin graph + real `Bridge` against a temp SQLite DB + a `TestPlayerController` stub. `BackendSmokeTest`, `UserFlowTest`, `WebdavStreamTest`, `platform/TrayControllerTest`.

## Rust crates (`rust-libs/`)

Workspace root: [`rust-libs/Cargo.toml`](./rust-libs/Cargo.toml) (resolver = `"2"`, centralized `[workspace.dependencies]`). `clippy.toml` sets `large-error-threshold = 256`.

| Crate | Purpose |
|---|---|
| `ease-client-backend` | Main backend; `crate-type = ["cdylib", "rlib"]`, GPL-3.0. Provides the `Backend` UniFFI object, controllers/services/repositories, the axum streaming server. Source: `controllers/`, `services/`, `repositories/`, `objects/`, `ctx.rs`, `infra.rs`, `streaming_server.rs`. |
| `ease-client-schema` | Sea-ORM entities, models, domain types. Exposes UniFFI-compatible schema types. |
| `ease-client-migration` | DB migration from legacy `redb` format to SQLite. Versioned upgraders in `src/legacy/` (`redb_v2`, `redb_v3`, `schema_v2`, `schema_v3`, `upgrader_v1_v2`, `upgrader_v2_v3`). Integration tests in `tests/`. |
| `ease-client-tokio` | Shared tokio multi-thread runtime accessor (`tokio_runtime()`). |
| `ease-client-android-ffi-builder` | Binary wrapping `uniffi bindgen` to generate the Kotlin bindings used by `build-jni-libs.ts`. |
| `ease-order-key` | Standalone orderable-key utility. **Dual MIT OR Apache-2.0 license** (different from the rest). |
| `ease-remote-storage` (path dep, not a workspace member) | WebDAV / OneDrive remote storage client (`reqwest`, `quick-xml`). GPL-3.0. |

Notable Rust constraints: UniFFI pinned to `=0.28.3` with the `tokio` feature; SQLite is force-bundled (`libsqlite3-sys` `bundled`) for cross-compilation; Sea-ORM 1.1 with sqlx-sqlite + runtime-tokio-rustls.

## Key patterns

### Dependency injection (Koin 4.0)
- [`di/AppModule.kt`](./composeApp/src/jvmShared/kotlin/com/kutedev/easemusicplayer/di/AppModule.kt) (jvmShared) — `appModule`: binds the app `CoroutineScope` (SupervisorJob + `Dispatchers.Default`), all repositories via `singleOf(::...)`, `Bridge`, `AppLifecycle`, and all ViewModels via `viewModelOf(::...)`.
- [`di/AndroidModule.kt`](./composeApp/src/androidMain/kotlin/com/kutedev/easemusicplayer/di/AndroidModule.kt) — `androidModule`: `AppPaths` (from `appContext.filesDir`), `PlayerController → PlayerControllerRepository`, `PermissionManager → PermissionRepository`.
- [`di/DesktopModule.kt`](./composeApp/src/desktopMain/kotlin/com/kutedev/easemusicplayer/di/DesktopModule.kt) — `desktopModule`: `AppPaths` (`~/.ease-music-player/`), `PlayerController → DesktopPlayerController`, `PermissionManager → DesktopPermissionManager`.
- Koin is started in `EaseMusicPlayerApplication.onCreate()` (Android) and `MainKt.main()` (Desktop) with `modules(appModule, <platformModule>)`. Activities/services use `KoinComponent` + `by inject()`.

### Bridge / FFI
[`singleton/Bridge.kt`](./composeApp/src/jvmShared/kotlin/com/kutedev/easemusicplayer/singleton/Bridge.kt) is a thin singleton wrapping the UniFFI `Backend`:
- `initialize()` / `destroy()` — create / dispose the backend.
- `run { backend -> ... }` — suspend, **swallows** exceptions and returns `null`.
- `runRaw { ... }` — suspend, **propagates** exceptions.
- `runSync { ... }` / `runSyncRaw { ... }` — non-suspend variants.

Repositories call backend functions through `bridge.run { }`. Backend function prefixes: `ct*` (controller), `cts*` (controller service). Argument structs are prefixed `Arg*` (e.g. `ArgUpsertStorage`, `ArgCreatePlaylist`, `ArgRemoveMusicFromPlaylist`). These types live in `ease-client-schema` and surface in Kotlin under `uniffi.ease_client_backend` / `uniffi.ease_client_schema`.

The Rust backend spawns work on the shared tokio runtime via `ease_client_tokio::tokio_runtime()`; UniFFI FFI controllers route through it.

### Lifecycle
[`lifecycle/AppLifecycle.kt`](./composeApp/src/jvmShared/kotlin/com/kutedev/easemusicplayer/lifecycle/AppLifecycle.kt) — `onStartup()` launches a coroutine that calls `reload()` on `playerRepository`, `storageRepository`, `playlistRepository` (in that order). Invoked from `MainActivity.onStart()` (Android) and from `Main.kt` after `bridge.initialize()` (desktop). No `onShutdown` is currently implemented.

### Repository pattern
All in `singleton/`, constructed with `Bridge` + `CoroutineScope`, expose `StateFlow` / `SharedFlow`:
- `PlayerRepository` — current music/playlist, play mode, derived `previousMusic` / `nextMusic` / `onCompleteMusic` flows, pause requests.
- `PlaylistRepository` — playlist list (debounced 500 ms reload), reorder via `ease-order-key`, reacts to storage-removal events.
- `StorageRepository` — cloud storage list, OAuth refresh token, remove events.
- `AssetRepository` — in-memory cache (HashMap) for cover art bytes + decoded bitmaps.
- `ImportRepository`, `ToastRepository`.

### ViewModels
Extend `androidx.lifecycle.ViewModel`, use `viewModelScope`. Bound via Koin `viewModelOf`. UI state classes are co-located (e.g. `PlaylistsState`, `SleepModeState`, `PlaylistsMode` enum). Example: `PlayerVM` polls playback position every 1 s.

### Platform abstraction (expect / actual)
- `commonMain/platform/Platform.kt`: `expect fun platformShowToast`, `platformOpenUrl`, `platformAppVersion`, `decodeImageBitmap`, `platformOpenFile`; plus the `AppPaths(documentDir, cacheDir)` data class.
- `commonMain/platform/BackHandler.kt`: `expect` composable.
- `actual` implementations live in `androidMain/platform/` and `desktopMain/platform/` (Android uses `Toast` / `BitmapFactory`; Desktop uses AWT `Desktop` / `ImageIO`).

### PlayerController (strategy)
[`singleton/PlayerController.kt`](./composeApp/src/jvmShared/kotlin/com/kutedev/easemusicplayer/singleton/PlayerController.kt) defines the interface (`play`, `resume`, `pause`, `stop`, `playNext`, `playPrevious`, `seek`, `getCurrentPosition`, `getBufferedPosition`, `scheduleSleep`, `cancelSleep`, `sleepState: StateFlow<SleepModeState>`). Android implements it with media3 (`PlayerControllerRepository`); Desktop implements it with JavaFX `MediaPlayer` (`DesktopPlayerController`).

## Build & run commands

### Prerequisites
- **JDK 21** (`run-desktop.ts` auto-detects Temurin / Eclipse Adoptium or Microsoft JDK under `C:\Program Files\` on Windows; CI uses Zulu 21).
- **Rust stable** + `cargo-ndk@3.5.4` + `rustup target add aarch64-linux-android` + `cargo-nextest`.
- **Android SDK** (compileSdk 35 / minSdk 29 / targetSdk 34) + **NDK r27c** with `ANDROID_NDK_HOME` set.
- **pnpm** for running the TypeScript scripts.

### Commands (run via `pnpm`)
| Command | What it does |
|---|---|
| `pnpm run:desktop` | Build host native lib + run `:composeApp:run` (desktop). Auto-detects JDK 21. |
| `pnpm build:jni` | `cargo build -p ease-client-backend` (debug) → regenerate UniFFI Kotlin bindings into `composeApp/src/jvmShared/kotlin/uniffi/` → copy host native lib to `composeApp/src/desktopMain/resources/natives/`. Pass `EBUILD=1` (`EBUILD=1 pnpm build:jni`) to skip the desktop copy (used for APK/CI builds). |
| `pnpm build:apk` | `EBUILD=1 build:jni` + cross-compile `arm64-v8a` release + `:composeApp:assembleRelease` + copy APK to `artifacts/apk/`. Requires `ANDROID_SIGN_JKS` (brotli + base64) and `ANDROID_SIGN_PASSWORD` secrets. |
| `pnpm test` | `cd rust-libs && cargo nextest run` (Rust tests). |

### Gradle tasks
- `./gradlew :composeApp:run` — run desktop app.
- `./gradlew :composeApp:test` — run `desktopTest` JUnit tests (**requires `pnpm build:jni` first**, because `jna.library.path = ../rust-libs/target/debug`).
- `./gradlew :composeApp:assembleRelease` — release APK (after JNI libs are present).
- `./gradlew :composeApp:packageDistributionForCurrentOS` — desktop Dmg / Deb / Msi.

### Generated / gitignored artifacts
These are **not checked in** and must be regenerated (via `pnpm build:jni`) before a clean checkout will compile:
- `composeApp/src/jvmShared/kotlin/uniffi/` — UniFFI Kotlin bindings.
- `composeApp/src/androidMain/jniLibs/arm64-v8a/` — Android native lib.
- `composeApp/src/desktopMain/resources/natives/` — desktop native lib.

## Conventions

- **Kotlin code style**: `official` (per `gradle.properties`). No license headers. No ktlint / detekt.
- **Class naming**:
  - `*Repository` — data layer (under `singleton/`).
  - `*Controller` — platform / interaction (under `singleton/` for app-level, under `platform/` for desktop-only AWT/JNA bits like `TrayController`).
  - `*VM` — ViewModels (under `viewmodels/`).
- **UI layout**: Compose screens under `widgets/<feature>/`; reusable composables under `components/`. Route helpers are top-level functions in `core/Routes.kt`.
- **UniFFI naming**: backend functions use `ct*` (controller) and `cts*` (controller service) prefixes; argument structs are prefixed `Arg*`.
- **Resources**:
  - Shared Compose resources under `commonMain/composeResources/{drawable,font,values,values-zh-rCN}`.
  - Desktop-only resources under `desktopMain/resources/` (`ic_launcher.png`, generated `natives/`).
  - Android resources under `androidMain/res/` (mipmaps, `values/strings.xml`, `xml/backup_rules.xml`, `xml/data_extraction_rules.xml`, `xml/file_paths.xml`).
- **Branch naming**: `feat/v<version>` (e.g. `feat/v0.4`). PRs target `main`.
- **Commit messages**: semantic — `feat:` / `fix:` / `refactor:` / `chore:` / `test:` / `docs:`.
- **Release tags**: `vX.Y.Z` and `pre-vX.Y.Z-beta.N` (trigger APK + desktop artifact CI).
- **ProGuard** (`composeApp/proguard-rules.pro`): keeps `uniffi.**`, `com.sun.jna.**`, `org.koin.**`, serializable companions/fields, and the whole `com.kutedev.easemusicplayer.**` package; `-dontwarn` for AWT headless / compose classes.

## Gotchas

- **`jna.library.path` is hardcoded** in [`Main.kt`](./composeApp/src/desktopMain/kotlin/com/kutedev/easemusicplayer/Main.kt) to `"../rust-libs/target/debug"`. Runs must execute from the `composeApp/` cwd (the gradle `run` task does this) or the native lib won't load.
- **Android `arm64-v8a` only** — no x86 / armeabi targets.
- **Version inconsistency**: `appVersion` is `"0.3.0"` in `composeApp/build.gradle.kts`, but desktop `platformAppVersion()` returns `"0.4.0-dev"`. Flag if touching versioning.
- **SQLite is force-bundled** (`libsqlite3-sys` `bundled` feature) for cross-compilation.
- **UniFFI is pinned** to `=0.28.3` with the `tokio` feature.
- **`.opencode/agents/git-end.md`** references `scripts/local_ci.cjs`, which does **not** currently exist in the tree.
- **License split**: the majority of this project is GPL-3.0; [`rust-libs/ease-order-key`](./rust-libs/ease-order-key) is dual-licensed MIT OR Apache-2.0. `rust-libs/ease-remote-storage` and `android/` each ship their own `LICENSE-GPL`.
- **`local.properties`** (repo root) contains a developer-specific Android SDK path and is checked in — be careful before committing changes to it.
