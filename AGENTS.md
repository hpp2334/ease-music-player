# AGENTS.md

Guide for coding agents working in this repository. Read this first.

## Project overview

Ease Music Player is a lightweight **Android** music player written in **Kotlin / Jetpack Compose** (UI) and **Rust** (backend). It targets Android `arm64-v8a` only.

Features: WebDAV and OneDrive cloud storage, playlist-based playback, music cover art, lyrics.

> **History note (0.3 → 0.4):** version 0.4 briefly migrated the UI to Kotlin Multiplatform / Compose Multiplatform with a Desktop JVM target (JavaFX `MediaPlayer` + Skiko). The desktop build was dropped for 0.4.0-beta.0 — memory overhead (~half a GB at idle, mostly from loading two rendering stacks) and lack of user-facing benefit made the single-target Android app the better shape. The Rust-side improvements from that era are kept (axum streaming server, `ease-client-schema` / `ease-client-migration` crate split, UniFFI tokio routing). See [`docs/motivation.md`](./docs/motivation.md).

## Architecture at a glance

```
┌──────────────────────────────┐        UniFFI (JNA)         ┌──────────────────────────────┐
│  Kotlin / Jetpack Compose    │ ──────────────────────────▶ │  Rust workspace (rust-libs/) │
│  android/app/  (Gradle :app) │                            │  ease-client-backend         │
│                              │ ◀────────────────────────── │  (cdylib: libease_client_*)  │
│  Hilt DI, media3 ExoPlayer   │   StateFlow / SharedFlow    │  + axum streaming server*    │
│                              │   via repositories          │  + Sea-ORM / SQLite          │
└──────────────────────────────┘                            └──────────────────────────────┘
```
\* The axum server is started by `Backend::init()` but Android does not consume it — Android streams audio via the FFI `ctGetAssetStream` callback through `MusicPlayerDataSource`. The server remains because it's harmless and the same backend drives potential future desktop clients.

- **Rust side** ([`rust-libs/`](./rust-libs/)) exposes a UniFFI `Backend` object as a `cdylib`. It owns the database (SQLite via Sea-ORM), business logic, controllers/services/repositories, and an axum HTTP streaming server bound to an OS-assigned port (`http://127.0.0.1:<port>/music/:id`, range requests supported for seeking).
- **Kotlin side** ([`android/app/`](./android/app/)) talks to the backend through [`singleton/Bridge.kt`](./android/app/src/main/java/com/kutedev/easemusicplayer/singleton/Bridge.kt), which wraps the generated UniFFI bindings and exposes suspend + sync helpers.
- **Playback**: media3 `ExoPlayer` via [`PlaybackService`](./android/app/src/main/java/com/kutedev/easemusicplayer/core/MusicPlayer.kt) (a `MediaSessionService`). Audio bytes are pulled from the Rust backend through [`MusicPlayerDataSource`](./android/app/src/main/java/com/kutedev/easemusicplayer/core/MusicPlayerDataSource.kt), which calls the FFI `ctGetAssetStream` and pipes chunks into ExoPlayer's `DataSource`.

## Repository layout

| Path | Purpose |
|---|---|
| [`android/`](./android/) | **Gradle root** of the Android project: `settings.gradle.kts`, `build.gradle.kts`, `gradle/`, `gradlew*`, `gradle.properties`, `gradle/libs.versions.toml`. |
| [`android/app/`](./android/app/) | The `:app` Gradle module — the Android application (Kotlin + Compose + Hilt + media3). |
| [`rust-libs/`](./rust-libs/) | Cargo workspace of Rust crates (backend, schema, migration, FFI builder, etc.). |
| [`scripts/`](./scripts/) | TypeScript build/test orchestration (run via `pnpm`/`tsx`). |
| [`docs/`](./docs/) | `motivation.md` + screenshots. |
| [`.github/workflows/`](./.github/workflows/) | CI: build JNI, run Rust tests, build APK on release tags. |
| [`.opencode/`](./.opencode/) | OpenCode agent config (subagents for git/PR finalization, image reading). |

Root Java package: `com.kutedev.easemusicplayer`. Namespace / applicationId: `com.kutedev.easemusicplayer`.

## Kotlin source layout (`android/app/src/main/java/com/kutedev/easemusicplayer/`)

- `MainActivity.kt` — `@AndroidEntryPoint` `ComponentActivity`; also declares the top-level `@HiltAndroidApp class EaseMusicPlayerApplication`. Hosts `setContent { Root() }`, requests permissions, wires the media3 `MediaController`, and runs the startup reload sequence.
- `Root.kt` — main `@Composable` (`NavHost`, routes, theme).
- `core/`
  - `MusicPlayer.kt` — `PlaybackService` (`MediaSessionService`) + media3 wiring. `@AndroidEntryPoint`.
  - `KeepBackendService.kt` — foreground service that keeps the Rust backend process alive. `@AndroidEntryPoint`.
  - `MusicPlayerDataSource.kt` — media3 `DataSource` that streams from the Rust backend via `ctGetAssetStream`.
  - `MusicPlayerUtil.kt` — media3 helpers (cover / duration probing via FFI).
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
- `AndroidManifest.xml` — declares `EaseMusicPlayerApplication`, `MainActivity`, `PlaybackService` (media3 session), `KeepBackendService`, OAuth2 redirect (`easem://oauth2redirect`), `FileProvider`.
- `res/` — Android resources (mipmaps, `values/strings.xml`, `values-zh-rCN/strings.xml`, `xml/backup_rules.xml`, `xml/data_extraction_rules.xml`, `xml/file_paths.xml`).
- `assets/` — Compose resources (`composeResources/drawable/`, `composeResources/font/noto_sans.ttf`).
- `jniLibs/arm64-v8a/` — gitignored, generated by `pnpm build:jni`.

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
`MainActivity.onStart()` launches a `lifecycleScope` coroutine that calls `reload()` on `playerRepository`, `storageRepository`, `playlistRepository` (in that order), then sets up the media3 `MediaController`. `Bridge.initialize()` is called earlier in `MainActivity.onCreate()` (after starting `KeepBackendService`).

### Repository pattern
All in `singleton/`, constructed with `Bridge` + `CoroutineScope`, expose `StateFlow` / `SharedFlow`:
- `PlayerRepository` — current music/playlist, play mode, derived `previousMusic` / `nextMusic` / `onCompleteMusic` flows, pause requests.
- `PlaylistRepository` — playlist list (debounced reload), reorder via `ease-order-key`, reacts to storage-removal events.
- `StorageRepository` — cloud storage list, OAuth refresh token, remove events.
- `AssetRepository` — in-memory cache for cover art bytes + decoded bitmaps.
- `PlayerControllerRepository`, `PermissionRepository`, `ImportRepository`, `ToastRepository`.

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

## Gotchas

- **Gradle root is at `android/`, not the repo root.** All `gradlew` invocations must `cd android/` first (the `build-apk.ts` script does this automatically).
- **Android `arm64-v8a` only** — no x86 / armeabi targets.
- **`local.properties`** (repo root) currently contains `sdk.dir=/usr/local/share/android-commandlinetools`, a developer-specific path. Adjust to point at your local Android SDK before building, or place a `local.properties` under `android/`.
- **SQLite is force-bundled** (`libsqlite3-sys` `bundled` feature) for cross-compilation.
- **UniFFI is pinned** to `=0.28.3` with the `tokio` feature.
- **`.opencode/agents/git-end.md`** references `scripts/local_ci.cjs`, which does **not** currently exist in the tree.
- **License split**: the majority of this project is GPL-3.0; [`rust-libs/ease-order-key`](./rust-libs/ease-order-key) is dual-licensed MIT OR Apache-2.0. `rust-libs/ease-remote-storage` and `android/` each ship their own `LICENSE-GPL`.
- **Version**: `android/app/build.gradle.kts` is the single source of truth for `versionName` (`0.4.0-beta.0` at the time of writing). The pre-0.4 `platformAppVersion()` desktop gotcha is gone.
