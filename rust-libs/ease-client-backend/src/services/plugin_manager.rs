//! Runtime-installable plugin manager — the Rust side of the plugin system.
//!
//! Plugins live as folders under `<app_document_dir>/plugins/<id>/`
//! (`manifest.json` + JS bundles). This module owns every mutation of that
//! tree — install-from-zip (SAF sideload via a temp path Kotlin copies the
//! `content://` stream to, registry download), enable/disable, uninstall —
//! plus the persisted install state (`plugin-state.json`), the remote
//! registry (fetch/cache/download+sha256-verify), the first-run bootstrap,
//! and the manifest scan. Kotlin keeps only platform glue: the SAF picker,
//! VMs, and the tur instance lifecycle.
//!
//! Module loading uses the handle-based path (tur #198): when
//! [`PluginManagerShared::set_runtime_handle`] has been called (from the
//! `bindPluginRuntime` JNI trampoline, right after `EasePluginBridge
//! .runtime(context)`), the scan reads each backend/view JS file and
//! registers it on the runtime's `ModuleSourceRegistry`. `plugin.list`
//! returns the opaque `jlong` handles; Kotlin loads them via
//! `TurInstance.loadModule(handle)` / `TurView(sourceHandle = …)`. The JS
//! bytes never cross the Kotlin↔Rust boundary.
//!
//! Every mutation bumps a monotonic `generation` (returned in the response)
//! which Kotlin mirrors into its `revision` StateFlow so
//! `KeepBackendService` tears down + reloads the affected JS backends.
//!
//! Wire format note: persisted JSON keeps the exact schema the pre-Rust
//! Kotlin implementation wrote (`firstRunDone` / `enabled` /
//! `lastSourceUrl` / `customSources`), so an upgraded install keeps its
//! state, and the per-source registry cache filenames stay md5-based so
//! existing caches survive.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::{OnceLock, RwLock};
use std::time::Duration;

use ease_client_schema::entities::storage;
use ease_client_schema::StorageId;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::error::{BError, BResult};

// ============================================================================
// Constants
// ============================================================================

/// Plugin zips shipped inside the APK (`assets/plugin-bundles/`),
/// ensure-installed by [`bootstrap`] (fresh installs *and* upgrades —
/// see `PluginState::bundled_installed`). `com.ease.lyricformats` is the
/// lyric-parser provider (LRC/SRT/VTT/QRC/YRC/TTML); the built-in Rust
/// LRC parser is gone, so it must ship offline.
pub const BUNDLED_PLUGINS: &[&str] = &["com.ease.webdav", "com.ease.lyricformats"];

const MAX_ENTRIES: usize = 200;
const MAX_TOTAL_BYTES: u64 = 20 * 1024 * 1024;

/// Cap for contribution icons read during the scan (base64 into the
/// `plugin.list` payload — keep small).
const MAX_ICON_BYTES: u64 = 128 * 1024;

/// Cap on `lyricParsers` extensions accepted per parser contribution.
const MAX_PARSER_EXTENSIONS: usize = 16;

/// Cap on sibling-file extensions probed per lyric load (one storage GET
/// each, worst case).
const MAX_SIBLING_EXTENSIONS: usize = 8;

const REPO: &str = "hpp2334/ease-music-player";
const REPO_REF: &str = "main";

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const JSON_TIMEOUT: Duration = Duration::from_secs(15);
const ZIP_TIMEOUT: Duration = Duration::from_secs(60);

// ============================================================================
// Persisted state + shared runtime state
// ============================================================================

/// One successfully-verified custom plugin source (a base URL serving
/// `plugins.json` + the `zips/…` it references). Wire-compatible with the
/// legacy Kotlin `plugin-state.json` entry.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CustomSource {
    pub url: String,
    #[serde(default)]
    pub label: String,
}

/// Persisted plugin-install state (`<app_document_dir>/plugin-state.json`).
/// Field names/shape are wire-compatible with the legacy Kotlin writer.
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct PluginState {
    #[serde(rename = "firstRunDone", default)]
    pub first_run_done: bool,
    #[serde(default)]
    pub enabled: HashMap<String, bool>,
    #[serde(
        rename = "lastSourceUrl",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub last_source_url: Option<String>,
    #[serde(rename = "customSources", default)]
    pub custom_sources: Vec<CustomSource>,
    /// User's per-extension lyric-parser pick (Lyric Parser settings page):
    /// extension (lowercase, no dot) → `"<pluginId>:<parserId>"`. Absent
    /// entry = Auto (default dispatch order). Never a generation-bumping
    /// mutation — dispatch reads it at call time.
    #[serde(rename = "lyricParserSelection", default)]
    pub lyric_parser_selection: BTreeMap<String, String>,
    /// Bundled APK plugins this install has already received via
    /// [`bootstrap`]'s ensure-installed pass (see [`BUNDLED_PLUGINS`]).
    /// Appending a new bundled id reaches existing installs on upgrade;
    /// a later user uninstall is never re-forced (the id stays recorded).
    #[serde(rename = "bundledInstalled", default)]
    pub bundled_installed: Vec<String>,
}

/// One plugin's `lyricParsers` contribution as held in the in-process
/// registry snapshot consumed by the lyric dispatch
/// ([`crate::services::lyrics`]).
#[derive(Clone, Debug)]
pub struct LyricParserEntry {
    pub plugin_id: String,
    pub enabled: bool,
    pub parsers: Vec<LyricParserRaw>,
}

/// The engine-binding seam between this (platform-agnostic) crate and the
/// Android embedder crate (`ease-client-android`), which implements it
/// over the tur engine.
///
/// One implementor instance exists per `bindPluginRuntime` call; it is
/// attached to [`PluginManagerShared`] by the JNI trampoline and
/// compare-and-set detached by `unbindPluginRuntime`. Host builds (tests,
/// `cargo check`) never attach one — the scan then reports zero
/// module-source handles plus loud warnings instead of failing silently,
/// and bootstrap reports its bundled zips as unreadable.
///
/// The headless-backend lifecycle is fully engine-side: the backend decides
/// *what* should run ([`reload_backends`]) and calls these methods to make
/// it so — no Kotlin-side orchestration of plugin instances remains.
pub trait PluginEngineHost: Send + Sync {
    /// Opaque identity of this binding (the tur runtime handle). Used by
    /// [`PluginManagerShared::detach_engine_host`] for the compare-and-set.
    fn id(&self) -> i64;

    /// Register module JS on the bound runtime's shared
    /// `ModuleSourceRegistry` and return its opaque handle. `0` on
    /// failure (never silent — implementors log loudly).
    fn register_source(&self, src: String) -> i64;

    /// Read one bundled APK asset (e.g. `plugin-bundles/<id>.zip`).
    /// `None` when unreadable / no asset manager bound.
    fn read_bundled_asset(&self, path: &str) -> Option<Vec<u8>>;

    /// Spawn a headless instance stamped with the plugin's id (assigned to
    /// the shared `ease-plugin-backend` worker pool) and return its opaque
    /// handle. The instance's pump wiring (the Kotlin `FrameLoop`) is the
    /// implementor's concern.
    fn spawn_headless(&self, plugin_id: &str) -> anyhow::Result<i64>;

    /// Evaluate the registered module source into the instance. FIFO-ordered
    /// behind the spawn on the engine's host thread.
    fn load_headless_module(&self, instance_handle: i64, source_handle: i64);

    /// Extract the `Send` RPC client for a spawned instance's event bus
    /// (a blocking round-trip that settles behind the spawn + module load —
    /// by the time it runs, the JS backend handlers are registered).
    /// `None` on a stale/invalid handle.
    fn extract_rpc(&self, instance_handle: i64) -> Option<ease_tur_rpc::RpcClient>;

    /// Close a spawned headless instance (cancel its pump wiring + destroy).
    /// Safe on stale handles (no-op).
    fn close_headless(&self, instance_handle: i64);

    /// Fire-and-forget upcall into the Kotlin host (the `EaseSignalHost`
    /// static). No return value ever — host data flows to Rust as state
    /// pushes, not queries. Op codes: see the `SIGNAL_*` constants.
    fn emit_signal(&self, op: u32, payload: &str);
}

/// Signal op codes shared between Rust emission and the Kotlin
/// `EaseSignalHost` consumer (mirror in `BackendSignal.kt`).
pub const SIGNAL_PLUGINS_CHANGED: u32 = 1;

/// Per-process shared manager state, held by [`crate::ctx::BackendContext`].
///
/// - `generation` — bumped on every install/uninstall/enable/disable
///   mutation; Kotlin mirrors it into its `revision` StateFlow.
/// - `install_lock` — serializes installs (staging + atomic swap).
/// - `engine_host` — the attached engine binding (tur runtime handle +
///   raw `AAssetManager`), set by the Android crate's `bindPluginRuntime`
///   and compare-and-set cleared by `unbindPluginRuntime`. `None` = not
///   bound (host builds, pre-bind, post-unbind).
/// - `headless` — the Rust-owned headless backend instances
///   (plugin id → instance handle); torn down + rebuilt by
///   [`reload_backends`].
/// - `reload_lock` — serializes backend reloads (bind-triggered +
///   mutation-triggered).
/// - `lyric_parsers` — the scan-populated lyric-parser registry (dispatch
///   reads it per lyric load; empty before the first `plugin.list`).
/// - `lyric_parser_selection` — the user's per-extension parser picks,
///   mirrored from `plugin-state.json` (see [`PluginState`]).
#[derive(Default)]
pub struct PluginManagerShared {
    generation: AtomicU64,
    pub install_lock: tokio::sync::Mutex<()>,
    engine_host: RwLock<Option<std::sync::Arc<dyn PluginEngineHost>>>,
    headless: RwLock<HashMap<String, i64>>,
    reload_lock: tokio::sync::Mutex<()>,
    lyric_parsers: RwLock<std::sync::Arc<Vec<LyricParserEntry>>>,
    lyric_parser_selection: RwLock<BTreeMap<String, String>>,
}

impl PluginManagerShared {
    pub fn generation(&self) -> i64 {
        self.generation.load(AtomicOrdering::SeqCst) as i64
    }

    pub fn bump_generation(&self) -> i64 {
        self.generation.fetch_add(1, AtomicOrdering::SeqCst) as i64 + 1
    }

    /// The attached engine binding, if any. Cloned out as an `Arc` —
    /// callers can hold it across awaits without the lock.
    pub fn engine_host(&self) -> Option<std::sync::Arc<dyn PluginEngineHost>> {
        self.engine_host.read().unwrap().clone()
    }

    /// Attach an engine binding (from the `bindPluginRuntime` JNI
    /// trampoline). Replaces any previous binding — the caller sequence
    /// (create runtime → bind) guarantees the previous one was already
    /// detached, and a would-be double bind is logged loudly.
    pub fn attach_engine_host(&self, host: std::sync::Arc<dyn PluginEngineHost>) {
        let mut w = self.engine_host.write().unwrap();
        if let Some(old) = w.as_ref() {
            tracing::warn!(
                "plugin manager: attach_engine_host({}) replacing existing binding {} \
                 — double bind?",
                host.id(),
                old.id()
            );
        } else {
            tracing::info!("plugin manager: engine host attached ({})", host.id());
        }
        *w = Some(host);
    }

    /// Detach the engine binding, but only when its id still equals
    /// `expected` (compare-and-set) — a stale teardown racing a newer
    /// `bindPluginRuntime` must never clobber the fresh binding. Returns
    /// the detached host (for caller-side teardown) when the binding was
    /// actually cleared.
    pub fn detach_engine_host(&self, expected: i64) -> Option<std::sync::Arc<dyn PluginEngineHost>> {
        let mut w = self.engine_host.write().unwrap();
        let matches = w.as_ref().map(|h| h.id()) == Some(expected);
        if matches {
            let taken = w.take();
            tracing::info!("plugin manager: engine host {expected} detached");
            taken
        } else {
            tracing::warn!(
                "plugin manager: detach_engine_host({expected}) skipped — current binding \
                 is {} (a newer runtime is bound; this teardown is stale)",
                w.as_ref().map(|h| h.id()).unwrap_or(0)
            );
            None
        }
    }

    /// Snapshot of the lyric-parser registry (scan order: plugins sorted
    /// by id, contributions in manifest order).
    pub fn lyric_parser_snapshot(&self) -> std::sync::Arc<Vec<LyricParserEntry>> {
        self.lyric_parsers.read().unwrap().clone()
    }

    /// Record a live headless backend instance (called by
    /// [`reload_backends`] after a successful spawn + wire).
    pub fn record_headless(&self, plugin_id: &str, instance_handle: i64) {
        self.headless
            .write()
            .unwrap()
            .insert(plugin_id.to_string(), instance_handle);
    }

    /// Take the live headless backend instance set (plugin id → instance
    /// handle), leaving it empty — the caller closes them.
    pub fn drain_headless(&self) -> Vec<(String, i64)> {
        std::mem::take(&mut *self.headless.write().unwrap())
            .into_iter()
            .collect()
    }

    /// The live headless backend instance for `plugin_id`, if any.
    pub fn headless_for(&self, plugin_id: &str) -> Option<i64> {
        self.headless.read().unwrap().get(plugin_id).copied()
    }

    /// Swap the lyric-parser registry (called by [`scan`]).
    pub fn set_lyric_parser_snapshot(&self, entries: Vec<LyricParserEntry>) {
        *self.lyric_parsers.write().unwrap() = std::sync::Arc::new(entries);
    }

    /// Current per-extension parser picks (cloned; small map).
    pub fn lyric_parser_selection(&self) -> BTreeMap<String, String> {
        self.lyric_parser_selection.read().unwrap().clone()
    }

    /// Mirror the persisted selection into the in-memory map (called by
    /// [`scan`] and the selection setter — never bumps the generation).
    pub fn set_lyric_parser_selection_map(&self, map: BTreeMap<String, String>) {
        *self.lyric_parser_selection.write().unwrap() = map;
    }

    /// Sibling-file extensions probed by the lyric resolver: every
    /// extension claimed by an enabled plugin's parser, scan order,
    /// deduped, capped. Deliberately ignores the user selection —
    /// selection only reorders parse candidates, not which sibling files
    /// are probed.
    pub fn lyric_sibling_extensions(&self) -> Vec<String> {
        let snapshot = self.lyric_parser_snapshot();
        let mut out: Vec<String> = Vec::new();
        for entry in snapshot.iter() {
            if !entry.enabled {
                continue;
            }
            for parser in &entry.parsers {
                for ext in &parser.extensions {
                    if !out.contains(ext) {
                        out.push(ext.clone());
                    }
                }
            }
        }
        out.truncate(MAX_SIBLING_EXTENSIONS);
        out
    }
}

// ============================================================================
// Path helpers
// ============================================================================

/// The installed-plugin root: `<app_document_dir>/plugins`.
pub fn plugins_root(app_document_dir: &str) -> PathBuf {
    Path::new(app_document_dir).join("plugins")
}

fn state_file(app_document_dir: &str) -> PathBuf {
    Path::new(app_document_dir).join("plugin-state.json")
}

fn registry_cache_dir(app_document_dir: &str) -> PathBuf {
    Path::new(app_document_dir).join("plugin-registry-cache")
}

fn registry_cache_file(app_document_dir: &str, base_url: &str) -> PathBuf {
    use md5::Md5;
    let digest = Md5::digest(base_url.as_bytes());
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    registry_cache_dir(app_document_dir).join(format!("{hex}.json"))
}

/// Raw-bytes cache for one registry icon (offline rehydration in
/// [`cached_registry`]), keyed by the resolved icon URL.
fn icon_cache_file(app_document_dir: &str, icon_url: &str) -> PathBuf {
    use md5::Md5;
    let digest = Md5::digest(icon_url.as_bytes());
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    registry_cache_dir(app_document_dir).join(format!("{hex}.icon"))
}

fn write_icon_cache(app_document_dir: &str, icon_url: &str, bytes: &[u8]) -> Option<()> {
    let f = icon_cache_file(app_document_dir, icon_url);
    std::fs::create_dir_all(f.parent()?).ok()?;
    std::fs::write(f, bytes).ok()
}

fn read_icon_cache(app_document_dir: &str, icon_url: &str) -> Option<Vec<u8>> {
    let bytes = std::fs::read(icon_cache_file(app_document_dir, icon_url)).ok()?;
    if bytes.len() as u64 > MAX_ICON_BYTES {
        return None;
    }
    Some(bytes)
}

fn b64_encode(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

// ============================================================================
// State read/write
// ============================================================================

pub fn read_state(app_document_dir: &str) -> PluginState {
    read_state_at(&state_file(app_document_dir))
}

fn read_state_at(path: &Path) -> PluginState {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn write_state(app_document_dir: &str, state: &PluginState) -> BResult<()> {
    let path = state_file(app_document_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(state).map_err(|e| BError::CustomError {
        message: format!("serialize plugin state: {e}"),
    })?;
    std::fs::write(path, text)?;
    Ok(())
}

/// Read-modify-write the persisted state. All writers go through this.
fn mutate_state<F>(app_document_dir: &str, f: F) -> BResult<PluginState>
where
    F: FnOnce(PluginState) -> PluginState,
{
    let next = f(read_state(app_document_dir));
    write_state(app_document_dir, &next)?;
    Ok(next)
}

// ============================================================================
// Install (zip validation + staging + atomic swap)
// ============================================================================

/// Id regex: `^[A-Za-z0-9._-]+$` (same as the legacy Kotlin check).
fn valid_plugin_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
}

fn unsafe_entry_name(name: &str) -> bool {
    name.starts_with('/') || name.contains("..") || name.contains('\\')
}

/// Validate + extract a plugin zip in-memory. On success the plugin folder
/// is swapped into `<root>/<id>/` (overwrite = upgrade) and the manifest is
/// returned. Mirrors the legacy Kotlin validation exactly: `manifest.json`
/// at the zip root, sane id, sanitized entry names, ≤200 entries, ≤20 MB.
pub fn install_zip_bytes_blocking(root: &Path, bytes: Vec<u8>) -> BResult<ManifestRaw> {
    let reader = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader).map_err(|e| BError::CustomError {
        message: format!("bad zip: {e}"),
    })?;

    let entries = archive.len();
    if entries > MAX_ENTRIES {
        return Err(BError::CustomError {
            message: format!("too many entries ({entries})"),
        });
    }

    // Pass 1 — validate entry names + total size, collect the file list.
    let mut total: u64 = 0;
    let mut files: Vec<(String, u64)> = Vec::new();
    let mut has_manifest = false;
    for i in 0..entries {
        let entry = archive.by_index_raw(i).map_err(|e| BError::CustomError {
            message: format!("zip entry {i}: {e}"),
        })?;
        let name = entry.name().to_string();
        if unsafe_entry_name(&name) {
            return Err(BError::CustomError {
                message: format!("unsafe entry: {name}"),
            });
        }
        if name == "manifest.json" {
            has_manifest = true;
        }
        if entry.is_dir() {
            continue;
        }
        total += entry.size();
        if total > MAX_TOTAL_BYTES {
            return Err(BError::CustomError {
                message: "zip too large".into(),
            });
        }
        files.push((name, entry.size()));
    }
    if !has_manifest {
        return Err(BError::CustomError {
            message: "manifest.json missing at zip root".into(),
        });
    }

    // Manifest id decides the target dir; validate before touching the tree.
    let manifest_idx = (0..entries)
        .find(|&i| {
            archive
                .by_index_raw(i)
                .map(|e| e.name() == "manifest.json")
                .unwrap_or(false)
        })
        .expect("has_manifest checked above");
    let mut manifest_bytes = Vec::new();
    {
        let mut manifest_file =
            archive
                .by_index(manifest_idx)
                .map_err(|e| BError::CustomError {
                    message: format!("read manifest.json: {e}"),
                })?;
        std::io::copy(&mut manifest_file, &mut manifest_bytes)?;
    }
    let manifest_text = String::from_utf8(manifest_bytes).map_err(|_| BError::CustomError {
        message: "manifest.json is not utf-8".into(),
    })?;
    let manifest = parse_manifest(&manifest_text)?;
    if !valid_plugin_id(&manifest.id) {
        return Err(BError::CustomError {
            message: format!("invalid plugin id: '{}'", manifest.id),
        });
    }

    std::fs::create_dir_all(root)?;
    let staging = root.join(format!(
        ".staging-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&staging)?;
    let result = (|| -> BResult<ManifestRaw> {
        // Pass 2 — extract.
        for i in 0..entries {
            let name = {
                let entry = archive.by_index_raw(i).map_err(|e| BError::CustomError {
                    message: format!("zip entry {i}: {e}"),
                })?;
                if entry.is_dir() {
                    String::new()
                } else {
                    entry.name().to_string()
                }
            };
            if name.is_empty() || name == "manifest.json" {
                continue; // dirs skipped; manifest already read
            }
            let dest = staging.join(&name);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out = std::fs::File::create(&dest)?;
            let mut fh = archive.by_index(i).map_err(|e| BError::CustomError {
                message: format!("zip entry {i}: {e}"),
            })?;
            std::io::copy(&mut fh, &mut out)?;
        }
        std::fs::write(staging.join("manifest.json"), &manifest_text)?;

        let target = root.join(&manifest.id);
        if target.exists() {
            std::fs::remove_dir_all(&target)?;
        }
        std::fs::rename(&staging, &target)?;
        Ok(manifest)
    })();

    if staging.exists() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    result
}

// ============================================================================
// Localized manifest text
// ============================================================================

/// Manifest text that may be localized per locale tag.
///
/// A localizable manifest field (`name`, `description`, contribution
/// `title` / `desc`) accepts either a plain string (the default/base text —
/// all pre-intl manifests) or a tag→string map
/// (`{"en-US": "Play Counts", "zh-CN": "播放计数"}`); this is the normalized
/// form both parse into. Kotlin resolves against the activity locale at
/// render time (exact tag → language prefix → base) — the Rust side never
/// picks a locale.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct LocalizedString {
    /// Fallback text: the plain string, or the map's `en-US` → `en` →
    /// lexicographically-first entry (maps are expected to carry `en-US`).
    pub base: String,
    /// tag → text overrides (`"zh-CN"` → `"播放计数"`); empty for plain
    /// strings.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub locales: BTreeMap<String, String>,
}

impl LocalizedString {
    pub fn plain(base: String) -> Self {
        LocalizedString {
            base,
            locales: BTreeMap::new(),
        }
    }
}

/// The shapes [`LocalizedString`] deserializes from. Variant order matters:
/// the normalized object must be tried before the tag map (a tag map would
/// otherwise swallow `{"base": …, "locales": …}`).
#[derive(Deserialize)]
#[serde(untagged)]
enum LocalizedRaw {
    Plain(String),
    Normalized {
        base: String,
        #[serde(default)]
        locales: BTreeMap<String, String>,
    },
    Map(BTreeMap<String, String>),
}

impl From<LocalizedRaw> for LocalizedString {
    fn from(raw: LocalizedRaw) -> Self {
        match raw {
            LocalizedRaw::Plain(base) => LocalizedString::plain(base),
            LocalizedRaw::Normalized { base, locales } => LocalizedString { base, locales },
            LocalizedRaw::Map(locales) => {
                let base = locales
                    .get("en-US")
                    .or_else(|| locales.get("en"))
                    .or_else(|| locales.values().next())
                    .cloned()
                    .unwrap_or_default();
                LocalizedString { base, locales }
            }
        }
    }
}

impl<'de> Deserialize<'de> for LocalizedString {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        LocalizedRaw::deserialize(deserializer).map(LocalizedString::from)
    }
}

// ============================================================================
// Manifest scan
// ============================================================================

/// Parsed `manifest.json` (permissive — mirrors the legacy Kotlin parse).
#[derive(Clone, Debug)]
pub struct ManifestRaw {
    pub id: String,
    pub name: LocalizedString,
    pub version: String,
    pub description: LocalizedString,
    pub backend: Option<String>,
    pub events: Vec<String>,
    /// Plugin-level icon file name relative to the plugin root (raster
    /// only) — shown on the installed-plugins list.
    pub icon: Option<String>,
    /// Base64 icon bytes — never parsed from the manifest; filled in by
    /// the scan ([`load_icon_base64`]).
    pub icon_data: Option<String>,
    pub dashboard: Vec<ContributionRaw>,
    pub storages: Vec<ContributionRaw>,
    pub lyric_parsers: Vec<LyricParserRaw>,
}

#[derive(Clone, Debug)]
pub struct ContributionRaw {
    pub id: String,
    /// Contribution title; `None` when the manifest omitted it (the UI then
    /// falls back to the plugin name — NOT the contribution id).
    pub title: Option<LocalizedString>,
    /// Short one-liner (dashboard card subtitle / chooser subtitle).
    pub desc: Option<LocalizedString>,
    pub view: Option<String>,
    /// Icon file name relative to the plugin root (raster only).
    pub icon: Option<String>,
    /// Base64 icon bytes — never parsed from the manifest; filled in by the
    /// scan ([`load_icon_base64`]).
    pub icon_data: Option<String>,
}

/// A `contributions.lyricParsers` entry. Headless-only (no `view`) — the
/// plugin's backend registers a single `lyric:parse` host-RPC handler
/// serving all of its parser contributions; `extensions` is what the
/// host dispatches on (file extension, normalized).
#[derive(Clone, Debug)]
pub struct LyricParserRaw {
    pub id: String,
    /// Parser title (Lyric Parser settings page); `None` → plugin name.
    pub title: Option<LocalizedString>,
    pub desc: Option<LocalizedString>,
    pub icon: Option<String>,
    pub icon_data: Option<String>,
    /// Lowercase, dot-free extensions this parser claims (e.g.
    /// `["srt", "vtt"]`), deduped, ≤ [`MAX_PARSER_EXTENSIONS`].
    pub extensions: Vec<String>,
}

/// Normalize one manifest extension entry: trim, strip a leading dot,
/// lowercase. `Some` only for `^[a-z0-9]{1,12}$` results.
fn normalize_extension(ext: &str) -> Option<String> {
    let ext = ext.trim().trim_start_matches('.').to_ascii_lowercase();
    if ext.is_empty() || ext.len() > 12 {
        return None;
    }
    if !ext.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    Some(ext)
}

fn opt_str(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
}

/// Parse a localizable field (`string | { "<tag>": string }`); `None` when
/// absent, not a string/map, or entirely empty (mirrors `opt_str`).
fn opt_localized(v: &Value, key: &str) -> Option<LocalizedString> {
    let x = v.get(key).filter(|x| !x.is_null())?;
    let parsed = serde_json::from_value::<LocalizedString>(x.clone()).ok()?;
    if parsed.base.is_empty() && parsed.locales.is_empty() {
        return None;
    }
    Some(parsed)
}

pub fn parse_manifest(text: &str) -> BResult<ManifestRaw> {
    let v: Value = serde_json::from_str(text).map_err(|e| BError::CustomError {
        message: format!("bad manifest.json: {e}"),
    })?;
    let id = opt_str(&v, "id").unwrap_or_default();
    let contributions = v.get("contributions").cloned().unwrap_or(Value::Null);
    let parse_list = |key: &str| -> Vec<ContributionRaw> {
        contributions
            .get(key)
            .and_then(|x| x.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|c| {
                        let cid = c.get("id").and_then(|x| x.as_str())?;
                        if cid.is_empty() {
                            return None;
                        }
                        Some(ContributionRaw {
                            id: cid.to_string(),
                            title: opt_localized(c, "title"),
                            desc: opt_localized(c, "desc"),
                            view: opt_str(c, "view"),
                            icon: opt_str(c, "icon"),
                            icon_data: None,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    let name = opt_localized(&v, "name").unwrap_or_else(|| LocalizedString::plain(id.clone()));
    let lyric_parsers = contributions
        .get("lyricParsers")
        .and_then(|x| x.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|c| {
                    let pid = c.get("id").and_then(|x| x.as_str())?;
                    if pid.is_empty() {
                        return None;
                    }
                    let mut extensions: Vec<String> = Vec::new();
                    if let Some(list) = c.get("extensions").and_then(|x| x.as_array()) {
                        for e in list.iter().filter_map(|x| x.as_str()) {
                            match normalize_extension(e) {
                                Some(ext) => {
                                    if !extensions.contains(&ext) {
                                        extensions.push(ext);
                                    }
                                }
                                None => tracing::warn!(
                                    "lyricParsers '{pid}': bad extension '{e}' — dropped"
                                ),
                            }
                        }
                    }
                    if extensions.is_empty() {
                        tracing::warn!(
                            "lyricParsers '{pid}': no valid extensions — contribution dropped"
                        );
                        return None;
                    }
                    extensions.truncate(MAX_PARSER_EXTENSIONS);
                    Some(LyricParserRaw {
                        id: pid.to_string(),
                        title: opt_localized(c, "title"),
                        desc: opt_localized(c, "desc"),
                        icon: opt_str(c, "icon"),
                        icon_data: None,
                        extensions,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(ManifestRaw {
        id,
        name,
        version: opt_str(&v, "version").unwrap_or_else(|| "0.0.0".into()),
        description: opt_localized(&v, "description").unwrap_or_default(),
        backend: opt_str(&v, "backend"),
        events: v
            .get("events")
            .and_then(|x| x.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|e| e.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default(),
        dashboard: parse_list("dashboard"),
        storages: parse_list("storages"),
        lyric_parsers,
        icon: opt_str(&v, "icon"),
        icon_data: None,
    })
}

pub fn is_installed(root: &Path, plugin_id: &str) -> bool {
    root.join(plugin_id).join("manifest.json").is_file()
}

/// Read + base64 a contribution icon (raster only: PNG/WebP/JPEG, ≤
/// [`MAX_ICON_BYTES`]). Name rules mirror zip-entry sanitization, so a
/// hand-edited manifest can't escape the plugin dir. Any violation logs a
/// warning and drops the icon — the UI falls back to the built-in glyph.
/// File IO stays here on the scan's blocking thread.
fn load_icon_base64(plugin_dir: &Path, icon: &str) -> Option<String> {
    if unsafe_entry_name(icon) {
        tracing::warn!("plugin icon: unsafe name '{icon}' — dropped");
        return None;
    }
    let lower = icon.to_ascii_lowercase();
    if !["png", "webp", "jpg", "jpeg"]
        .iter()
        .any(|e| lower.ends_with(&format!(".{e}")))
    {
        tracing::warn!("plugin icon: unsupported type '{icon}' — dropped");
        return None;
    }
    let file = plugin_dir.join(icon);
    let Ok(meta) = std::fs::metadata(&file) else {
        tracing::warn!("plugin icon: '{icon}' not found — dropped");
        return None;
    };
    if !meta.is_file() {
        tracing::warn!("plugin icon: '{icon}' is not a file — dropped");
        return None;
    }
    if meta.len() > MAX_ICON_BYTES {
        tracing::warn!(
            "plugin icon: '{icon}' exceeds {MAX_ICON_BYTES} bytes — dropped"
        );
        return None;
    }
    let bytes = std::fs::read(&file).ok()?;
    Some(b64_encode(&bytes))
}

/// Walk `<root>/*/manifest.json` and parse each. Folders starting with `.`
/// (staging leftovers) are skipped.
fn scan_manifests_blocking(root: &Path) -> Vec<(PathBuf, ManifestRaw)> {
    let Ok(dirs) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut out: Vec<(PathBuf, ManifestRaw)> = Vec::new();
    for dir in dirs.flatten() {
        let path = dir.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        let manifest_file = path.join("manifest.json");
        if !manifest_file.is_file() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&manifest_file) else {
            continue;
        };
        if let Ok(mut m) = parse_manifest(&text) {
            let id = if m.id.is_empty() {
                name.to_string()
            } else {
                m.id
            };
            // Contribution + plugin-level icons: read here (blocking
            // thread), regardless of enabled state — the management page
            // shows disabled plugins too.
            if let Some(icon) = m.icon.as_deref() {
                m.icon_data = load_icon_base64(&path, icon);
            }
            for c in m.dashboard.iter_mut().chain(m.storages.iter_mut()) {
                if let Some(icon) = c.icon.as_deref() {
                    c.icon_data = load_icon_base64(&path, icon);
                }
            }
            for p in m.lyric_parsers.iter_mut() {
                if let Some(icon) = p.icon.as_deref() {
                    p.icon_data = load_icon_base64(&path, icon);
                }
            }
            out.push((path, ManifestRaw { id, ..m }));
        }
    }
    out.sort_by(|a, b| a.1.id.cmp(&b.1.id));
    out
}

// ============================================================================
// Module-source registration (tur #198 handle-based loading)
// ============================================================================

/// Read one module file and register it on the bound engine host. Failures
/// are logged with the full path and also collected into the scan's
/// `warnings` so they surface beyond the logs (the `plugin.list` payload
/// carries them).
fn read_registered(
    root: &Path,
    plugin_id: &str,
    file: &str,
    host: Option<&std::sync::Arc<dyn PluginEngineHost>>,
    warnings: &mut Vec<String>,
) -> i64 {
    let path = root.join(plugin_id).join(file);
    match std::fs::read_to_string(&path) {
        Ok(src) => {
            let handle = match host {
                Some(h) => h.register_source(src),
                None => {
                    tracing::error!(
                        "read_registered: no engine host bound — bindPluginRuntime not \
                         called (or already unbound); source NOT registered"
                    );
                    0
                }
            };
            if handle == 0 {
                warnings.push(format!(
                    "{plugin_id}: module source {file} not registered (see logs)"
                ));
            }
            handle
        }
        Err(e) => {
            let msg = format!("{plugin_id}: read module {file} failed: {e}");
            tracing::error!("plugin scan: {msg} (path {path:?})");
            warnings.push(msg);
            0
        }
    }
}

/// Map one contribution list to wire shape, registering view module sources
/// (enabled plugins only; disabled ones get zero handles — a re-enable bumps
/// the generation and the service rescans).
fn contribution_infos(
    list: Vec<ContributionRaw>,
    enabled: bool,
    root: &Path,
    plugin_id: &str,
    host: Option<&std::sync::Arc<dyn PluginEngineHost>>,
    warnings: &mut Vec<String>,
) -> Vec<ContributionInfo> {
    list.into_iter()
        .map(|c| {
            let source_handle = if enabled {
                c.view
                    .as_deref()
                    .map(|f| read_registered(root, plugin_id, f, host, warnings))
                    .unwrap_or(0)
            } else {
                0
            };
            ContributionInfo {
                source_handle,
                id: c.id,
                title: c.title,
                desc: c.desc,
                icon: c.icon,
                icon_data: c.icon_data,
                view: c.view,
            }
        })
        .collect()
}

// ============================================================================
// Registry (fetch / parse / cache / download)
// ============================================================================

/// One entry of a registry `plugins.json`. `installed_version` /
/// `update_available` are stamped at fetch time by comparing against the
/// installed tree (Kotlin never compares versions). `name` / `description`
/// accept the localized forms (plain string or tag map) — old registries
/// with plain strings parse unchanged, and the normalized shape round-trips
/// when Kotlin sends the entry back via `plugin.installFromRegistry`.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct RegistryEntry {
    pub id: String,
    pub name: LocalizedString,
    pub version: String,
    pub description: LocalizedString,
    /// Zip path relative to the source base URL (e.g. `zips/<id>-<v>.zip`),
    /// or an absolute http(s) URL.
    pub zip: String,
    pub sha256: String,
    pub size: u64,
    pub min_app_version: Option<String>,
    /// Icon path relative to the source base URL (e.g. `icons/<id>.png`),
    /// or an absolute http(s) URL — Rust-side input only: resolved, fetched
    /// and cached into `icon_data`; never serialized to Kotlin (Kotlin
    /// renders the bytes, it has no use for the path).
    #[serde(skip_serializing)]
    pub icon: Option<String>,
    /// Base64 icon bytes fetched during `fetch_registry` (or rehydrated
    /// from the per-source icon cache by `cached_registry`). `None` when
    /// the registry declared no icon or the fetch failed — the UI falls
    /// back to the built-in glyph.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed_version: Option<String>,
    pub update_available: bool,
}

fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .expect("reqwest client build")
    })
}

pub async fn http_get_text(url: &str) -> BResult<String> {
    let resp = http_client()
        .get(url)
        .timeout(JSON_TIMEOUT)
        .send()
        .await
        .map_err(|e| BError::CustomError {
            message: format!("GET {url}: {e}"),
        })?;
    let status = resp.status();
    if !status.is_success() {
        return Err(BError::CustomError {
            message: format!("GET {url}: HTTP {status}"),
        });
    }
    resp.text().await.map_err(|e| BError::CustomError {
        message: format!("GET {url}: {e}"),
    })
}

pub async fn http_download(url: &str) -> BResult<Vec<u8>> {
    let resp = http_client()
        .get(url)
        .timeout(ZIP_TIMEOUT)
        .send()
        .await
        .map_err(|e| BError::CustomError {
            message: format!("GET {url}: {e}"),
        })?;
    let status = resp.status();
    if !status.is_success() {
        return Err(BError::CustomError {
            message: format!("GET {url}: HTTP {status}"),
        });
    }
    let bytes = resp.bytes().await.map_err(|e| BError::CustomError {
        message: format!("GET {url}: {e}"),
    })?;
    Ok(bytes.into())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn parse_registry(body: &str) -> BResult<Vec<RegistryEntry>> {
    let v: Value = serde_json::from_str(body).map_err(|e| BError::CustomError {
        message: format!("bad plugins.json: {e}"),
    })?;
    let arr = v.get("plugins").and_then(|x| x.as_array());
    Ok(arr
        .map(|arr| {
            arr.iter()
                .filter_map(|e| {
                    let id = e.get("id").and_then(|x| x.as_str()).unwrap_or("");
                    if id.is_empty() {
                        return None;
                    }
                    Some(RegistryEntry {
                        id: id.to_string(),
                        name: opt_localized(e, "name")
                            .unwrap_or_else(|| LocalizedString::plain(id.to_string())),
                        version: opt_str(e, "version").unwrap_or_else(|| "0.0.0".into()),
                        description: opt_localized(e, "description").unwrap_or_default(),
                        zip: opt_str(e, "zip").unwrap_or_default(),
                        sha256: opt_str(e, "sha256").unwrap_or_default(),
                        size: e.get("size").and_then(|x| x.as_u64()).unwrap_or(0),
                        min_app_version: opt_str(e, "minAppVersion"),
                        icon: opt_str(e, "icon"),
                        icon_data: None,
                        installed_version: None,
                        update_available: false,
                    })
                })
                .collect()
        })
        .unwrap_or_default())
}

/// Fetch `<base>/plugins.json`, cache the body, return stamped entries.
pub async fn fetch_registry(app_document_dir: &str, base_url: &str) -> BResult<Vec<RegistryEntry>> {
    let url = format!("{}/plugins.json", base_url.trim_end_matches('/'));
    let body = http_get_text(&url).await?;
    let mut entries = parse_registry(&body)?;
    let cache = registry_cache_file(app_document_dir, base_url);
    if let Some(parent) = cache.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&cache, &body);
    // Icons: fetched + validated + base64'd here, cached as raw bytes so
    // `cached_registry` can rehydrate them offline. Failures are non-fatal
    // (the entry keeps rendering with the built-in glyph).
    for e in entries.iter_mut() {
        if let Some(icon) = e.icon.clone() {
            let icon_url = entry_asset_url(&icon, base_url);
            match http_download(&icon_url).await {
                Ok(bytes) if bytes.len() as u64 <= MAX_ICON_BYTES => {
                    let _ = write_icon_cache(app_document_dir, &icon_url, &bytes);
                    e.icon_data = Some(b64_encode(&bytes));
                }
                Ok(_) => tracing::warn!("registry icon too large: {icon_url}"),
                Err(err) => tracing::warn!("registry icon fetch failed: {icon_url}: {err}"),
            }
        }
    }
    Ok(entries)
}

/// The last cached registry for `base_url`, if any (offline fallback).
pub fn cached_registry(app_document_dir: &str, base_url: &str) -> Option<Vec<RegistryEntry>> {
    let f = registry_cache_file(app_document_dir, base_url);
    let body = std::fs::read_to_string(f).ok()?;
    let mut entries = parse_registry(&body).ok()?;
    for e in entries.iter_mut() {
        if let Some(icon) = e.icon.as_deref() {
            let icon_url = entry_asset_url(icon, base_url);
            if let Some(bytes) = read_icon_cache(app_document_dir, &icon_url) {
                e.icon_data = Some(b64_encode(&bytes));
            }
        }
    }
    Some(entries)
}

/// Stamp `installed_version` + `update_available` against the installed tree.
pub fn stamp_entries(entries: Vec<RegistryEntry>, root: &Path) -> Vec<RegistryEntry> {
    let installed = scan_manifests_blocking(root);
    entries
        .into_iter()
        .map(|mut e| {
            if let Some((_, m)) = installed.iter().find(|(_, m)| m.id == e.id) {
                let installed_version = m.version.clone();
                e.update_available =
                    compare_versions(&e.version, &installed_version) == Ordering::Greater;
                e.installed_version = Some(installed_version);
            }
            e
        })
        .collect()
}

/// Resolve a registry-relative asset path against the source base URL
/// (shared by `zip` and `icon` fields).
pub fn entry_asset_url(path: &str, base_url: &str) -> String {
    if path.starts_with("http") {
        path.to_string()
    } else {
        format!(
            "{}/{}",
            base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }
}

/// Resolve an entry's zip URL against the source base URL.
pub fn entry_zip_url(entry: &RegistryEntry, base_url: &str) -> String {
    entry_asset_url(&entry.zip, base_url)
}

// ============================================================================
// Version compare
// ============================================================================

/// Compare two dotted versions ("1.10.0" > "1.9.2"); non-numeric parts
/// compare lexicographically (exact parity with the legacy Kotlin impl —
/// a missing segment behaves as `""`, which sorts before any non-empty
/// part, so "1.0" < "1.0.0").
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let mut as_ = a.split('.');
    let mut bs = b.split('.');
    loop {
        match (as_.next(), bs.next()) {
            (None, None) => return Ordering::Equal,
            (Some(x), Some(y)) => {
                let cmp = match (x.parse::<i64>(), y.parse::<i64>()) {
                    (Ok(xn), Ok(yn)) => xn.cmp(&yn),
                    _ => x.cmp(y),
                };
                if cmp != Ordering::Equal {
                    return cmp;
                }
            }
            (Some(x), None) => {
                if !x.is_empty() {
                    return Ordering::Greater;
                }
            }
            (None, Some(y)) => {
                if !y.is_empty() {
                    return Ordering::Less;
                }
            }
        }
    }
}

// ============================================================================
// Sources
// ============================================================================

/// Hard-coded source presets — always offered in the source picker.
pub fn preset_sources() -> Vec<(String, String)> {
    vec![
        (
            format!("https://cdn.jsdelivr.net/gh/{REPO}@{REPO_REF}/plugins/registry"),
            "jsDelivr (CDN)".into(),
        ),
        (
            format!("https://fastly.jsdelivr.net/gh/{REPO}@{REPO_REF}/plugins/registry"),
            "fastly.jsdelivr (CN)".into(),
        ),
        (
            format!("https://gcore.jsdelivr.net/gh/{REPO}@{REPO_REF}/plugins/registry"),
            "gcore.jsdelivr (CN)".into(),
        ),
        (
            format!("https://raw.githubusercontent.com/{REPO}/{REPO_REF}/plugins/registry"),
            "GitHub Raw".into(),
        ),
    ]
}

fn host_label(url: &str) -> String {
    url.trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or(url)
        .to_string()
}

/// The persisted `lastSourceUrl` when it still names a selectable source
/// (preset or saved custom); `None` otherwise (stale pin — e.g. a preset
/// whose ref changed — so the caller falls back to the first preset).
pub fn effective_last_source(state: &PluginState) -> Option<String> {
    let url = state.last_source_url.as_ref()?;
    let known = preset_sources().iter().any(|(u, _)| u == url)
        || state.custom_sources.iter().any(|s| &s.url == url);
    if known {
        Some(url.clone())
    } else {
        None
    }
}

// ============================================================================
// High-level operations (bridge-facing)
// ============================================================================

/// Install from a local zip path (Kotlin stream-copies the SAF `content://`
/// pick here first; the bytes never cross JNI).
pub async fn install_from_zip_path(
    cx: &crate::ctx::BackendContext,
    app_document_dir: &str,
    path: &str,
) -> BResult<(String, i64)> {
    let _guard = cx.plugin_manager().install_lock.lock().await;
    let path = path.to_string();
    let bytes = tokio::task::spawn_blocking(move || std::fs::read(path))
        .await
        .map_err(|e| BError::CustomError {
            message: format!("read zip: {e}"),
        })?
        .map_err(|e| BError::CustomError {
            message: format!("read zip: {e}"),
        })?;
    install_bytes_and_enable(cx, app_document_dir, bytes).await
}

/// Install (or upgrade) one entry from a registry source: download +
/// sha256-verify + install.
pub async fn install_from_registry(
    cx: &crate::ctx::BackendContext,
    app_document_dir: &str,
    entry: &RegistryEntry,
    base_url: &str,
) -> BResult<(String, i64)> {
    let _guard = cx.plugin_manager().install_lock.lock().await;
    remember_source(app_document_dir, base_url)?;
    let url = entry_zip_url(entry, base_url);
    let bytes = http_download(&url).await?;
    let hex = sha256_hex(&bytes);
    if !hex.eq_ignore_ascii_case(&entry.sha256) {
        return Err(BError::CustomError {
            message: format!("sha256 mismatch (expected {}, got {hex})", entry.sha256),
        });
    }
    install_bytes_and_enable(cx, app_document_dir, bytes).await
}

async fn install_bytes_and_enable(
    cx: &crate::ctx::BackendContext,
    app_document_dir: &str,
    bytes: Vec<u8>,
) -> BResult<(String, i64)> {
    let dir = app_document_dir.to_string();
    let manifest =
        tokio::task::spawn_blocking(move || install_zip_bytes_blocking(&plugins_root(&dir), bytes))
            .await
            .map_err(|e| BError::CustomError {
                message: format!("install task: {e}"),
            })??;
    mutate_state(app_document_dir, |s| PluginState {
        enabled: {
            let mut enabled = s.enabled;
            enabled.insert(manifest.id.clone(), true);
            enabled
        },
        ..s
    })?;
    tracing::info!("plugin installed: {} {}", manifest.id, manifest.version);
    // The new/updated backend (if any) loads in the background reload.
    spawn_reload_backends(cx);
    Ok((manifest.id.clone(), cx.plugin_manager().bump_generation()))
}

pub async fn set_enabled(
    cx: &crate::ctx::BackendContext,
    app_document_dir: &str,
    plugin_id: &str,
    enabled: bool,
) -> BResult<i64> {
    mutate_state(app_document_dir, |s| PluginState {
        enabled: {
            let mut enabled_map = s.enabled;
            enabled_map.insert(plugin_id.to_string(), enabled);
            enabled_map
        },
        ..s
    })?;
    tracing::info!(
        "plugin {}: {plugin_id}",
        if enabled { "enabled" } else { "disabled" }
    );
    // Enable/disable changes the live backend set — reload in the background.
    spawn_reload_backends(cx);
    Ok(cx.plugin_manager().bump_generation())
}

/// Set (or clear, with `key: None`) the user's parser pick for one
/// extension — the Lyric Parser settings page. Validates `key` against
/// the current scan snapshot, persists to `plugin-state.json` and mirrors
/// into the in-memory map. **Never bumps the generation**: selection
/// changes no backend lifecycle (a bump would tear down + reload every
/// plugin backend for a pure preference change), and dispatch reads the
/// in-memory map at call time. Returns the updated selection map.
pub async fn set_lyric_parser_selection(
    cx: &crate::ctx::BackendContext,
    app_document_dir: &str,
    ext: &str,
    key: Option<&str>,
) -> BResult<BTreeMap<String, String>> {
    let ext = normalize_extension(ext).ok_or_else(|| BError::CustomError {
        message: format!("bad lyric parser extension '{ext}'"),
    })?;
    if let Some(key) = key {
        let valid = cx.plugin_manager().lyric_parser_snapshot().iter().any(|e| {
            e.enabled
                && e.parsers.iter().any(|p| {
                    p.extensions.contains(&ext) && format!("{}:{}", e.plugin_id, p.id) == key
                })
        });
        if !valid {
            return Err(BError::CustomError {
                message: format!("no enabled lyric parser '{key}' for '{ext}'"),
            });
        }
    }
    let ext_for_closure = ext.clone();
    let key = key.map(str::to_string);
    let next = mutate_state(app_document_dir, move |s| {
        let mut selection = s.lyric_parser_selection.clone();
        match &key {
            Some(k) => {
                selection.insert(ext_for_closure.clone(), k.clone());
            }
            None => {
                selection.remove(&ext_for_closure);
            }
        }
        PluginState {
            lyric_parser_selection: selection,
            ..s
        }
    })?;
    cx.plugin_manager()
        .set_lyric_parser_selection_map(next.lyric_parser_selection.clone());
    Ok(next.lyric_parser_selection)
}

/// Uninstall: delete the plugin folder + its enabled flag, and wipe all of
/// the plugin's persisted data — its storage rows (cascading to their
/// musics, playlist entries and cover blobs, exactly like removing the
/// storage by hand), its `plugin_kv` store and its scoped secrets. Nothing
/// survives a reinstall.
pub async fn uninstall(
    cx: &crate::ctx::BackendContext,
    app_document_dir: &str,
    plugin_id: &str,
) -> BResult<i64> {
    // Plugin-owned storage rows first: the remove_storage cascade drops the
    // musics / playlist entries / cover blobs referencing them.
    let db = cx.database_server().db();
    let plugin_storages = storage::Entity::find()
        .filter(storage::Column::PluginId.eq(plugin_id))
        .all(&db)
        .await?;
    for row in plugin_storages {
        crate::services::remove_storage(cx, StorageId::wrap(row.id)).await?;
    }
    // Then the plugin's own persisted data (KV + secrets).
    cx.database_server().plugin_kv_delete_all(plugin_id).await?;
    cx.database_server()
        .secret_remove_all_for_plugin(plugin_id)
        .await?;

    let root = plugins_root(app_document_dir);
    let target = root.join(plugin_id);
    let _ = tokio::task::spawn_blocking(move || {
        if target.exists() {
            std::fs::remove_dir_all(target)
        } else {
            Ok(())
        }
    })
    .await
    .map_err(|e| BError::CustomError {
        message: format!("uninstall task: {e}"),
    })?;
    mutate_state(app_document_dir, |s| {
        let prefix = format!("{plugin_id}:");
        let lyric_parser_selection = s
            .lyric_parser_selection
            .iter()
            .filter(|(_, v)| !v.starts_with(&prefix))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        PluginState {
            enabled: {
                let mut enabled = s.enabled;
                enabled.remove(plugin_id);
                enabled
            },
            lyric_parser_selection,
            ..s
        }
    })?;
    // Mirror the selection cleanup into the in-memory dispatch map.
    let state_after = read_state(app_document_dir);
    cx.plugin_manager()
        .set_lyric_parser_selection_map(state_after.lyric_parser_selection);
    tracing::info!("plugin uninstalled: {plugin_id}");
    // Tears down the (now orphaned) backend instance + storage rows.
    spawn_reload_backends(cx);
    Ok(cx.plugin_manager().bump_generation())
}

pub fn remember_source(app_document_dir: &str, url: &str) -> BResult<()> {
    mutate_state(app_document_dir, |s| PluginState {
        last_source_url: Some(url.to_string()),
        ..s
    })?;
    Ok(())
}

/// Verify + persist a custom source. Returns the parsed entries on success.
pub async fn add_custom_source(
    app_document_dir: &str,
    url: &str,
    label: Option<&str>,
) -> BResult<Vec<RegistryEntry>> {
    let normalized = url.trim().trim_end_matches('/').to_string();
    let entries = fetch_registry(app_document_dir, &normalized).await?;
    mutate_state(app_document_dir, |s| {
        if s.custom_sources.iter().any(|c| c.url == normalized) {
            s
        } else {
            PluginState {
                custom_sources: s
                    .custom_sources
                    .iter()
                    .cloned()
                    .chain([CustomSource {
                        url: normalized.clone(),
                        label: label
                            .map(|l| l.to_string())
                            .unwrap_or_else(|| host_label(&normalized)),
                    }])
                    .collect(),
                ..s
            }
        }
    })?;
    Ok(entries)
}

pub fn remove_custom_source(app_document_dir: &str, url: &str) -> BResult<()> {
    mutate_state(app_document_dir, |s| PluginState {
        custom_sources: s
            .custom_sources
            .iter()
            .filter(|c| c.url != url)
            .cloned()
            .collect(),
        last_source_url: if s.last_source_url.as_deref() == Some(url) {
            None
        } else {
            s.last_source_url.clone()
        },
        ..s
    })?;
    Ok(())
}

/// Startup defaults, run on every `KeepBackendService` create:
///
/// 1. **Ensure-installed pass** for [`BUNDLED_PLUGINS`]: each bundled id
///    not yet recorded in `PluginState::bundled_installed` is installed
///    from its APK asset (offline-friendly). Recording happens after a
///    successful install (or when the folder already exists — e.g. the
///    user installed it from the registry first — which preserves their
///    version + enabled flag), so adding a new bundled id reaches
///    existing installs on upgrade while a later uninstall is never
///    re-forced. A failed install stays unrecorded and retries next
///    start.
/// 2. **First-run pass** (guarded by `firstRunDone`): install any plugin
///    referenced by an existing storage row (upgrade path); non-bundled
///    referenced plugins are fetched from the default registry source
///    best-effort.
pub async fn bootstrap(cx: &crate::ctx::BackendContext, app_document_dir: &str) -> BResult<i64> {
    let shared = cx.plugin_manager();
    let root = plugins_root(app_document_dir);
    let mut mutated = false;

    // 1) Bundled plugins — ensure-installed (fresh installs AND upgrades).
    {
        let state = read_state(app_document_dir);
        let mut recorded = state.bundled_installed.clone();
        let mut changed = false;
        for id in BUNDLED_PLUGINS {
            if recorded.iter().any(|r| r == id) {
                continue;
            }
            if is_installed(&root, id) {
                // Already present (e.g. installed from the registry before
                // this app version bundled it) — keep the user's copy.
                recorded.push(id.to_string());
                changed = true;
                continue;
            }
            let host = shared.engine_host();
            match host.as_ref().and_then(|h| h.read_bundled_asset(&format!("plugin-bundles/{id}.zip"))) {
                Some(bytes) => {
                    match install_bytes_and_enable(cx, app_document_dir, bytes).await {
                        Ok(_) => {
                            recorded.push(id.to_string());
                            changed = true;
                            mutated = true;
                        }
                        Err(e) => {
                            tracing::error!("plugin bootstrap: bundled install '{id}' failed: {e}")
                        }
                    }
                }
                None => tracing::error!(
                    "plugin bootstrap: bundled zip '{id}' missing (engine host not bound?)"
                ),
            }
        }
        if changed {
            mutate_state(app_document_dir, move |s| PluginState {
                bundled_installed: recorded,
                ..s
            })?;
        }
    }

    // 2) First-run-only: plugins referenced by existing storage rows.
    {
        let state = read_state(app_document_dir);
        if !state.first_run_done {
            let referenced: Vec<String> = collect_storage_plugin_ids(cx).await;
            for id in referenced {
                if BUNDLED_PLUGINS.contains(&id.as_str()) || is_installed(&root, &id) {
                    continue;
                }
                // Best-effort bundled install first, then the default
                // registry source.
                let host = shared.engine_host();
                let bundled = host
                    .as_ref()
                    .and_then(|h| h.read_bundled_asset(&format!("plugin-bundles/{id}.zip")));
                if let Some(bytes) = bundled {
                    if let Err(e) = install_bytes_and_enable(cx, app_document_dir, bytes).await {
                        tracing::error!("plugin bootstrap: bundled install '{id}' failed: {e}");
                    }
                    continue;
                }
                let base = preset_sources()[0].0.clone();
                match fetch_registry(app_document_dir, &base).await {
                    Ok(entries) => {
                        let entry = entries.iter().find(|e| e.id == id);
                        match entry {
                            Some(entry) => {
                                if let Err(e) =
                                    install_from_registry(cx, app_document_dir, entry, &base).await
                                {
                                    tracing::error!(
                                        "plugin bootstrap: could not restore '{id}' ({e}); \
                                         storage will show removed until user installs it"
                                    );
                                }
                            }
                            None => tracing::error!(
                                "plugin bootstrap: '{id}' not in registry; storage will show removed \
                                 until user installs it"
                            ),
                        }
                    }
                    Err(e) => tracing::error!(
                        "plugin bootstrap: registry fetch failed ({e}); storage will show removed \
                         until user installs it"
                    ),
                }
            }

            mutate_state(app_document_dir, |s| PluginState {
                first_run_done: true,
                ..s
            })?;
            mutated = true;
        }
    }

    if mutated {
        // Fresh installs (or upgrades) need their backends loaded.
        spawn_reload_backends(cx);
        Ok(shared.bump_generation())
    } else {
        Ok(shared.generation())
    }
}

async fn collect_storage_plugin_ids(cx: &crate::ctx::BackendContext) -> Vec<String> {
    let db = cx.database_server().db();
    let Ok(rows) = storage::Entity::find().all(&db).await else {
        return Vec::new();
    };
    let mut ids: Vec<String> = rows
        .into_iter()
        .filter(|r| r.r#type == 2) // StorageType::Plugin
        .filter_map(|r| r.plugin_id)
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

// ============================================================================
// Headless backend lifecycle (Rust-owned)
// ============================================================================

/// Close every live headless backend instance + unwire its service RPC
/// entry. Used by [`reload_backends`] (teardown half) and the unbind path
/// (the runtime is about to die — stale `RpcClient`s must not survive into
/// storage dispatch).
pub fn teardown_headless_backends(
    cx: &crate::ctx::BackendContext,
    host: &dyn PluginEngineHost,
) {
    for (pid, instance) in cx.plugin_manager().drain_headless() {
        cx.remove_service_rpc(&pid);
        host.close_headless(instance);
        tracing::debug!("plugin backend torn down: {pid} (instance {instance})");
    }
}

/// (Re)load the headless backend instances for all enabled plugins — the
/// single reload path, triggered by the engine binding (bind) and by every
/// set-changing mutation (install / uninstall / enable / disable /
/// bootstrap). Serialized by the shared reload lock; a concurrent
/// unbind CAS-detaches the host, which the next reload observes (scans then
/// come back with zero handles + warnings — the loud degradation path).
pub async fn reload_backends(cx: &crate::ctx::BackendContext, app_document_dir: &str) {
    let _guard = cx.plugin_manager().reload_lock.lock().await;
    let Some(host) = cx.plugin_manager().engine_host() else {
        tracing::error!(
            "reload_backends: no engine host bound — plugin backends not loaded \
             (bindPluginRuntime not called / already unbound)"
        );
        return;
    };
    // Teardown: close old instances + unwire their RPC entries so storage
    // dispatch + events for a disabled/uninstalled plugin stop at the source.
    teardown_headless_backends(cx, host.as_ref());
    // Rescan (registers module sources for the enabled set on this host).
    let list = match scan(cx, app_document_dir).await {
        Ok(list) => list,
        Err(e) => {
            tracing::error!("reload_backends: scan failed: {e}");
            return;
        }
    };
    // Spawn + load + wire every enabled backend.
    let mut loaded = 0usize;
    for plugin in &list.plugins {
        if !plugin.enabled || plugin.backend.is_none() || plugin.backend_source_handle == 0 {
            if plugin.enabled && plugin.backend.is_some() {
                tracing::error!(
                    "plugin backend skipped (no source handle): {} — see the plugin \
                     scan warnings in the log",
                    plugin.id
                );
            }
            continue;
        }
        let instance = match host.spawn_headless(&plugin.id) {
            Ok(h) => h,
            Err(e) => {
                tracing::error!("plugin backend load failed: {} ({e})", plugin.id);
                continue;
            }
        };
        host.load_headless_module(instance, plugin.backend_source_handle);
        match host.extract_rpc(instance) {
            Some(rpc) => {
                cx.set_service_rpc(&plugin.id, rpc);
                cx.plugin_manager().record_headless(&plugin.id, instance);
                tracing::info!(
                    "plugin backend loaded: {}/{:?} (instance {instance})",
                    plugin.id,
                    plugin.backend
                );
                loaded += 1;
            }
            None => {
                tracing::error!(
                    "plugin backend wire failed (no RpcClient): {} — see logs",
                    plugin.id
                );
                host.close_headless(instance);
            }
        }
    }
    let gen = cx.plugin_manager().bump_generation();
    tracing::info!("plugin backends reloaded: {loaded} live (generation {gen})");
    host.emit_signal(SIGNAL_PLUGINS_CHANGED, &gen.to_string());
}

/// Fire-and-forget [`reload_backends`] on the shared tokio runtime — the
/// mutation path (bridge calls) must not block its response on the full
/// spawn + module-eval cycle.
pub fn spawn_reload_backends(cx: &crate::ctx::BackendContext) {
    let cx = cx.clone();
    let dir = cx.get_app_document_dir();
    ease_client_tokio::tokio_runtime().spawn(async move {
        reload_backends(&cx, &dir).await;
    });
}

// ============================================================================
// Scan → wire payload
// ============================================================================

/// One plugin's scan result, wire-shaped for `plugin.list`.
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PluginScanInfo {
    pub id: String,
    pub name: LocalizedString,
    pub version: String,
    pub description: LocalizedString,
    pub backend: Option<String>,
    pub backend_source_handle: i64,
    pub events: Vec<String>,
    /// Base64 plugin icon bytes (present only when the manifest's
    /// plugin-level icon file passed validation). The file name itself
    /// stays Rust-side — Kotlin only renders the bytes.
    pub icon_data: Option<String>,
    pub dashboard: Vec<ContributionInfo>,
    pub storages: Vec<ContributionInfo>,
    pub lyric_parsers: Vec<LyricParserInfo>,
    pub enabled: bool,
}

/// One `lyricParsers` contribution, wire-shaped for `plugin.list`. No
/// `view`/source handle — parsing is headless-only (the plugin backend's
/// `lyric:parse` handler).
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LyricParserInfo {
    pub id: String,
    pub title: Option<LocalizedString>,
    pub desc: Option<LocalizedString>,
    pub icon: Option<String>,
    pub icon_data: Option<String>,
    pub extensions: Vec<String>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ContributionInfo {
    pub id: String,
    /// `None` when the manifest omitted `title` (UI falls back to the
    /// plugin name).
    pub title: Option<LocalizedString>,
    pub desc: Option<LocalizedString>,
    /// Icon file name (informational).
    pub icon: Option<String>,
    /// Base64 icon bytes (present only when the file passed validation).
    pub icon_data: Option<String>,
    pub view: Option<String>,
    pub source_handle: i64,
}

/// Scan the installed tree and register module sources for **enabled**
/// plugins only (disabled plugins come back with zero handles; a re-enable
/// bumps the generation and the service rescans). Also swaps the
/// in-process lyric-parser registry + selection mirror consumed by the
/// lyric dispatch ([`crate::services::lyrics`]).
pub async fn scan(
    cx: &crate::ctx::BackendContext,
    app_document_dir: &str,
) -> BResult<PluginListOut> {
    let root = plugins_root(app_document_dir);
    let dir = app_document_dir.to_string();
    let host = cx.plugin_manager().engine_host();
    if host.is_none() {
        tracing::error!(
            "plugin scan: no tur runtime bound — every module-source handle will be 0 \
             (bindPluginRuntime not called / already unbound); plugin views + backends \
             will not load"
        );
    }
    let (scanned, state) = tokio::task::spawn_blocking(move || {
        (
            scan_manifests_blocking(&plugins_root(&dir)),
            read_state(&dir),
        )
    })
    .await
    .map_err(|e| BError::CustomError {
        message: format!("scan task: {e}"),
    })?;

    let mut warnings: Vec<String> = Vec::new();
    if host.is_none() {
        warnings.push(
            "no tur runtime bound — module sources not registered (plugin views/backends \
             will not load)"
                .to_string(),
        );
    }
    let mut registry: Vec<LyricParserEntry> = Vec::with_capacity(scanned.len());
    let plugins = scanned
        .into_iter()
        .map(|(_, m)| {
            let ManifestRaw {
                id,
                name,
                version,
                description,
                backend,
                events,
                icon: _,
                icon_data,
                dashboard,
                storages,
                lyric_parsers,
            } = m;
            let enabled = state.enabled.get(&id).copied().unwrap_or(true);
            let backend_source_handle = if enabled {
                backend
                    .as_deref()
                    .map(|f| read_registered(&root, &id, f, host.as_ref(), &mut warnings))
                    .unwrap_or(0)
            } else {
                0
            };
            let dashboard_infos = contribution_infos(
                dashboard,
                enabled,
                &root,
                &id,
                host.as_ref(),
                &mut warnings,
            );
            let storages_infos =
                contribution_infos(storages, enabled, &root, &id, host.as_ref(), &mut warnings);
            registry.push(LyricParserEntry {
                plugin_id: id.clone(),
                enabled,
                parsers: lyric_parsers.clone(),
            });
            let parser_infos = lyric_parsers
                .into_iter()
                .map(|p| LyricParserInfo {
                    id: p.id,
                    title: p.title,
                    desc: p.desc,
                    icon: p.icon,
                    icon_data: p.icon_data,
                    extensions: p.extensions,
                })
                .collect();
            PluginScanInfo {
                id,
                name,
                version,
                description,
                backend,
                backend_source_handle,
                events,
                icon_data,
                dashboard: dashboard_infos,
                storages: storages_infos,
                lyric_parsers: parser_infos,
                enabled,
            }
        })
        .collect();
    let shared = cx.plugin_manager();
    shared.set_lyric_parser_snapshot(registry);
    shared.set_lyric_parser_selection_map(state.lyric_parser_selection.clone());
    Ok(PluginListOut {
        generation: shared.generation(),
        lyric_parser_selection: state.lyric_parser_selection,
        plugins,
        warnings,
    })
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PluginListOut {
    pub generation: i64,
    /// User's per-extension parser picks (extension →
    /// `"<pluginId>:<parserId>"`) — everything the Lyric Parser settings
    /// page renders rides this one payload.
    pub lyric_parser_selection: BTreeMap<String, String>,
    pub plugins: Vec<PluginScanInfo>,
    /// Non-fatal scan problems (module-source registration failures, unreadable
    /// view files, …). Also logged Rust-side; carried on the wire so future UI
    /// can surface them.
    pub warnings: Vec<String>,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_zip(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let opts: zip::write::SimpleFileOptions = Default::default();
        for (name, content) in entries {
            w.start_file::<_, ()>(name.to_string(), opts).unwrap();
            w.write_all(content.as_bytes()).unwrap();
        }
        let cursor = w.finish().unwrap();
        cursor.into_inner()
    }

    fn manifest_json(id: &str, version: &str) -> String {
        format!(
            r#"{{"id":"{id}","name":"{id}","version":"{version}","backend":"backend.js",
                "contributions":{{"dashboard":[{{"id":"main","title":"Main","view":"view.js"}}]}}}}"#
        )
    }

    fn temp_root() -> (tempdir_guard::TempDir, PathBuf) {
        let dir = tempdir_guard::TempDir::new();
        let path = dir.path().join("plugins");
        std::fs::create_dir_all(&path).unwrap();
        (dir, path)
    }

    /// Minimal temp-dir guard (no extra test deps).
    mod tempdir_guard {
        use std::path::{Path, PathBuf};

        pub struct TempDir(PathBuf);

        impl TempDir {
            pub fn new() -> Self {
                let base = std::env::temp_dir();
                let unique = format!(
                    "ease-plugin-test-{}-{}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                );
                let path = base.join(unique);
                std::fs::create_dir_all(&path).unwrap();
                TempDir(path)
            }

            pub fn path(&self) -> &Path {
                &self.0
            }
        }

        impl Drop for TempDir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    #[test]
    fn install_and_rescan_roundtrip() {
        let (_guard, root) = temp_root();
        let zip = make_zip(&[
            ("manifest.json", &manifest_json("com.ease.test", "1.0.0")),
            ("backend.js", "export function start() {}"),
            ("view.js", "export function start() {}"),
        ]);
        let m = install_zip_bytes_blocking(&root, zip).unwrap();
        assert_eq!(m.id, "com.ease.test");
        assert!(is_installed(&root, "com.ease.test"));
        assert!(root.join("com.ease.test/backend.js").is_file());

        let scanned = scan_manifests_blocking(&root);
        assert_eq!(scanned.len(), 1);
        assert_eq!(scanned[0].1.id, "com.ease.test");
        assert_eq!(scanned[0].1.dashboard.len(), 1);
        assert_eq!(scanned[0].1.dashboard[0].view.as_deref(), Some("view.js"));
    }

    #[test]
    fn upgrade_replaces_folder() {
        let (_guard, root) = temp_root();
        install_zip_bytes_blocking(
            &root,
            make_zip(&[("manifest.json", &manifest_json("com.ease.test", "1.0.0"))]),
        )
        .unwrap();
        install_zip_bytes_blocking(
            &root,
            make_zip(&[
                ("manifest.json", &manifest_json("com.ease.test", "2.0.0")),
                ("extra.js", "// v2"),
            ]),
        )
        .unwrap();
        assert!(root.join("com.ease.test/extra.js").is_file());
        assert_eq!(scan_manifests_blocking(&root)[0].1.version, "2.0.0");
    }

    #[test]
    fn rejects_missing_manifest() {
        let (_guard, root) = temp_root();
        let err = install_zip_bytes_blocking(&root, make_zip(&[("a.js", "// x")])).unwrap_err();
        assert!(err.to_string().contains("manifest.json"));
    }

    #[test]
    fn rejects_bad_plugin_id() {
        let (_guard, root) = temp_root();
        let err = install_zip_bytes_blocking(
            &root,
            make_zip(&[("manifest.json", r#"{"id":"../evil"}"#)]),
        )
        .unwrap_err();
        assert!(err.to_string().contains("invalid plugin id"));
    }

    #[test]
    fn rejects_unsafe_entry_names() {
        let (_guard, root) = temp_root();
        let err = install_zip_bytes_blocking(
            &root,
            make_zip(&[
                ("manifest.json", &manifest_json("ok", "1.0.0")),
                ("../escape.js", "// x"),
            ]),
        )
        .unwrap_err();
        assert!(err.to_string().contains("unsafe entry"));
    }

    #[test]
    fn rejects_too_many_entries() {
        let (_guard, root) = temp_root();
        let mut entries: Vec<(String, String)> =
            vec![("manifest.json".to_string(), manifest_json("many", "1.0.0"))];
        for i in 0..MAX_ENTRIES {
            entries.push((format!("f{i}.js"), "// x".to_string()));
        }
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let opts: zip::write::SimpleFileOptions = Default::default();
        for (name, content) in &entries {
            w.start_file::<_, ()>(name.clone(), opts).unwrap();
            w.write_all(content.as_bytes()).unwrap();
        }
        let zip = w.finish().unwrap().into_inner();
        let err = install_zip_bytes_blocking(&root, zip).unwrap_err();
        assert!(err.to_string().contains("too many entries"));
    }

    #[test]
    fn state_roundtrip_keeps_legacy_schema() {
        let (_guard, root) = temp_root();
        let dir = root.parent().unwrap();
        let legacy = r#"{
            "firstRunDone": true,
            "enabled": {"com.ease.webdav": false},
            "lastSourceUrl": "https://example.com/reg",
            "customSources": [{"url": "http://x/y", "label": "x"}]
        }"#;
        std::fs::write(dir.join("plugin-state.json"), legacy).unwrap();
        let state = read_state(dir.to_str().unwrap());
        assert!(state.first_run_done);
        assert_eq!(state.enabled.get("com.ease.webdav"), Some(&false));
        assert_eq!(state.custom_sources.len(), 1);

        write_state(dir.to_str().unwrap(), &state).unwrap();
        let reread = read_state(dir.to_str().unwrap());
        assert_eq!(reread, state);
    }

    #[test]
    fn registry_parse_and_stamp() {
        let body = r#"{"plugins":[
            {"id":"com.ease.a","name":"A","version":"1.0.0","zip":"zips/a.zip","sha256":"00","size":10,
             "icon":"icons/com.ease.a.png"},
            {"id":"com.ease.b","version":"2.0.0"}
        ]}"#;
        let mut entries = parse_registry(body).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, LocalizedString::plain("A".into()));
        assert_eq!(entries[1].name, LocalizedString::plain("com.ease.b".into()));
        assert_eq!(
            entries[0].icon.as_deref(),
            Some("icons/com.ease.a.png"),
            "registry icon path parses Rust-side"
        );
        assert!(entries[1].icon.is_none());
        assert!(entries[0].icon_data.is_none(), "icon bytes fill at fetch time");
        // `icon` never serializes to Kotlin; `iconData` is omitted when None
        // (round-trip parity with `plugin.installFromRegistry` echo).
        let json = serde_json::to_value(&entries[0]).unwrap();
        assert!(json.get("icon").is_none());
        assert!(json.get("iconData").is_none());

        let (_guard, root) = temp_root();
        install_zip_bytes_blocking(
            &root,
            make_zip(&[("manifest.json", &manifest_json("com.ease.a", "0.9.0"))]),
        )
        .unwrap();
        entries = stamp_entries(entries, &root);
        assert_eq!(entries[0].installed_version.as_deref(), Some("0.9.0"));
        assert!(entries[0].update_available);
        assert!(entries[1].installed_version.is_none());
        assert!(!entries[1].update_available);
    }

    #[test]
    fn localized_registry_entries_parse() {
        // New registries may carry tag maps; the normalized shape must also
        // round-trip (Kotlin sends the entry back via installFromRegistry).
        let body = r#"{"plugins":[
            {"id":"com.ease.a",
             "name":{"en-US":"A","zh-CN":"甲"},
             "description":"plain desc"}
        ]}"#;
        let entries = parse_registry(body).unwrap();
        assert_eq!(entries[0].name.base, "A");
        assert_eq!(entries[0].name.locales.get("zh-CN").map(String::as_str), Some("甲"));
        assert_eq!(entries[0].description, LocalizedString::plain("plain desc".into()));

        // Round-trip through the normalized wire shape.
        let json = serde_json::to_value(&entries[0]).unwrap();
        let back: RegistryEntry = serde_json::from_value(json).unwrap();
        assert_eq!(back.name, entries[0].name);
    }

    #[test]
    fn localized_manifest_fields_parse() {
        let text = r#"{
            "id": "com.ease.test",
            "name": {"en-US": "Test", "zh-CN": "测试"},
            "description": {"zh-CN": "只有中文"},
            "contributions": {
                "dashboard": [
                    {"id": "main",
                     "title": {"en-US": "Main", "zh-CN": "主页"},
                     "desc": "plain subtitle"}
                ]
            }
        }"#;
        let m = parse_manifest(text).unwrap();
        assert_eq!(m.name.base, "Test");
        assert_eq!(m.name.locales.get("zh-CN").map(String::as_str), Some("测试"));
        // Map without en-US/en: base falls back to the lexicographically
        // first tag.
        assert_eq!(m.description.base, "只有中文");
        let d = &m.dashboard[0];
        assert_eq!(d.title.as_ref().unwrap().base, "Main");
        assert_eq!(
            d.title.as_ref().unwrap().locales.get("zh-CN").map(String::as_str),
            Some("主页")
        );
        assert_eq!(d.desc.as_ref().unwrap().base, "plain subtitle");
        assert!(d.icon.is_none());
    }

    #[test]
    fn icon_scan_rules() {
        let (_guard, root) = temp_root();
        let dir = root.join("com.ease.test");
        std::fs::create_dir_all(&dir).unwrap();
        let png = [0x89u8, b'P', b'N', b'G', 1, 2, 3, 4];
        std::fs::write(dir.join("icon.png"), png).unwrap();
        std::fs::write(
            dir.join("manifest.json"),
            r#"{"id":"com.ease.test",
                "contributions":{"dashboard":[{"id":"a","icon":"icon.png"},
                             {"id":"b","icon":"../escape.png"},
                             {"id":"c","icon":"logo.svg"},
                             {"id":"d","icon":"missing.png"}]}}"#,
        )
        .unwrap();

        let scanned = scan_manifests_blocking(&root);
        assert_eq!(scanned.len(), 1);
        let m = &scanned[0].1;
        use base64::Engine as _;
        let expect = base64::engine::general_purpose::STANDARD.encode(png);
        assert_eq!(m.dashboard[0].icon_data.as_deref(), Some(expect.as_str()));
        // Unsafe / unsupported / missing → dropped, never an install failure.
        assert!(m.dashboard[1].icon_data.is_none());
        assert!(m.dashboard[2].icon_data.is_none());
        assert!(m.dashboard[3].icon_data.is_none());

        // Oversized → dropped.
        std::fs::write(dir.join("big.png"), vec![0u8; (MAX_ICON_BYTES + 1) as usize]).unwrap();
        std::fs::write(
            dir.join("manifest.json"),
            r#"{"id":"com.ease.test","contributions":{"dashboard":[{"id":"a","icon":"big.png"}]}}"#,
        )
        .unwrap();
        assert!(scan_manifests_blocking(&root)[0].1.dashboard[0].icon_data.is_none());
    }

    /// Plugin-level (root manifest) icons follow the same validation +
    /// base64 path as contribution icons.
    #[test]
    fn root_icon_scan_rules() {
        let (_guard, root) = temp_root();
        let dir = root.join("com.ease.test");
        std::fs::create_dir_all(&dir).unwrap();
        let png = [0x89u8, b'P', b'N', b'G', 9, 8, 7, 6];
        std::fs::write(dir.join("icon.png"), png).unwrap();
        std::fs::write(
            dir.join("manifest.json"),
            r#"{"id":"com.ease.test","icon":"icon.png"}"#,
        )
        .unwrap();
        let m = &scan_manifests_blocking(&root)[0].1;
        assert_eq!(m.icon.as_deref(), Some("icon.png"));
        use base64::Engine as _;
        let expect = base64::engine::general_purpose::STANDARD.encode(png);
        assert_eq!(m.icon_data.as_deref(), Some(expect.as_str()));

        // Unsafe name → dropped; absent field → None. Neither is fatal.
        std::fs::write(
            dir.join("manifest.json"),
            r#"{"id":"com.ease.test","icon":"../escape.png"}"#,
        )
        .unwrap();
        assert!(scan_manifests_blocking(&root)[0].1.icon_data.is_none());
        std::fs::write(dir.join("manifest.json"), r#"{"id":"com.ease.test"}"#).unwrap();
        let m = &scan_manifests_blocking(&root)[0].1;
        assert!(m.icon.is_none());
        assert!(m.icon_data.is_none());
    }

    #[test]
    fn zip_url_resolution() {
        let entry = RegistryEntry {
            zip: "zips/a-1.0.0.zip".into(),
            ..Default::default()
        };
        assert_eq!(
            entry_zip_url(&entry, "https://x.example/registry/"),
            "https://x.example/registry/zips/a-1.0.0.zip"
        );
        let abs = RegistryEntry {
            zip: "http://mirror/a.zip".into(),
            ..Default::default()
        };
        assert_eq!(entry_zip_url(&abs, "https://x/"), "http://mirror/a.zip");
    }

    #[test]
    fn version_compare_semantics() {
        assert_eq!(compare_versions("1.10.0", "1.9.2"), Ordering::Greater);
        assert_eq!(compare_versions("1.0.0", "1.0.0"), Ordering::Equal);
        assert_eq!(compare_versions("0.1.0", "1.0.0"), Ordering::Less);
        // Kotlin parity: a missing segment behaves as "" (< any non-empty).
        assert_eq!(compare_versions("1.0", "1.0.0"), Ordering::Less);
        assert_eq!(compare_versions("1.0.0", "1.0"), Ordering::Greater);
        // Non-numeric parts compare lexicographically.
        assert_eq!(compare_versions("1.0-rc", "1.0.0"), Ordering::Greater);
    }

    #[test]
    fn effective_last_source_drops_stale_pins() {
        let state = PluginState {
            last_source_url: Some("https://cdn.jsdelivr.net/gh/old@main/plugins/registry".into()),
            custom_sources: vec![CustomSource {
                url: "http://192.168.1.1:8899".into(),
                label: "lan".into(),
            }],
            ..Default::default()
        };
        // @main pin no longer matches any preset → dropped.
        assert_eq!(effective_last_source(&state), None);

        let state = PluginState {
            last_source_url: Some("http://192.168.1.1:8899".into()),
            ..state
        };
        // Still names a saved custom source → kept.
        assert_eq!(
            effective_last_source(&state).as_deref(),
            Some("http://192.168.1.1:8899")
        );
    }

    #[test]
    fn staging_dirs_are_ignored_by_scan() {
        let (_guard, root) = temp_root();
        std::fs::create_dir_all(root.join(".staging-123")).unwrap();
        install_zip_bytes_blocking(
            &root,
            make_zip(&[("manifest.json", &manifest_json("com.ease.x", "1.0.0"))]),
        )
        .unwrap();
        assert_eq!(scan_manifests_blocking(&root).len(), 1);
    }

    #[test]
    fn lyric_parser_manifest_rules() {
        let text = r#"{
            "id": "com.ease.test",
            "contributions": {"lyricParsers": [
                {"id": "lrc", "title": {"en-US": "LRC", "zh-CN": "LRC"},
                 "extensions": ["lrc"]},
                {"id": "subtitle", "extensions": [".SRT", " vtt ", "vtt", "../x", "", "waytoolongextension"]},
                {"id": "noext", "extensions": ["??"]},
                {"extensions": ["lrc"]}
            ]}
        }"#;
        let m = parse_manifest(text).unwrap();
        assert_eq!(m.lyric_parsers.len(), 2, "entries without id or extensions drop");
        let lrc = &m.lyric_parsers[0];
        assert_eq!(lrc.id, "lrc");
        assert_eq!(lrc.extensions, vec!["lrc".to_string()]);
        assert_eq!(lrc.title.as_ref().unwrap().base, "LRC");
        let subtitle = &m.lyric_parsers[1];
        // Leading dot / case / surrounding whitespace normalize; dupes,
        // unsafe and malformed entries drop.
        assert_eq!(subtitle.extensions, vec!["srt".to_string(), "vtt".to_string()]);

        // Absent section parses to empty.
        let m = parse_manifest(&manifest_json("com.ease.plain", "1.0.0")).unwrap();
        assert!(m.lyric_parsers.is_empty());
    }

    #[tokio::test]
    async fn lyric_parser_scan_registry_and_selection() {
        let (guard, root) = temp_root();
        let app_document_dir = guard.path().to_str().unwrap().to_string();
        install_zip_bytes_blocking(
            &root,
            make_zip(&[
                (
                    "manifest.json",
                    r#"{"id":"com.ease.test","name":"Test","version":"1.0.0",
                        "contributions":{"lyricParsers":[
                            {"id":"lrc","extensions":["lrc"]},
                            {"id":"subtitle","extensions":["srt","vtt"]}]}}"#,
                ),
                ("backend.js", "export function start() {}"),
            ]),
        )
        .unwrap();

        let cx = crate::ctx::BackendContext::new();
        cx.database_server()
            .init(app_document_dir.clone())
            .await
            .unwrap();
        let out = scan(&cx, &app_document_dir).await.unwrap();
        assert_eq!(out.plugins[0].lyric_parsers.len(), 2);
        assert_eq!(
            out.plugins[0].lyric_parsers[1].extensions,
            vec!["srt".to_string(), "vtt".to_string()]
        );
        assert!(out.lyric_parser_selection.is_empty());

        // Registry snapshot + sibling extensions (enabled by default).
        let shared = cx.plugin_manager();
        assert_eq!(
            shared.lyric_sibling_extensions(),
            vec!["lrc".to_string(), "srt".to_string(), "vtt".to_string()]
        );

        // Valid pick persists + mirrors; invalid pick errors; clear → Auto.
        let sel = set_lyric_parser_selection(&cx, &app_document_dir, ".SRT", Some("com.ease.test:subtitle"))
            .await
            .unwrap();
        assert_eq!(sel.get("srt").map(String::as_str), Some("com.ease.test:subtitle"));
        assert_eq!(read_state(&app_document_dir).lyric_parser_selection.get("srt").map(String::as_str),
            Some("com.ease.test:subtitle"));
        assert!(set_lyric_parser_selection(&cx, &app_document_dir, "lrc", Some("com.ease.test:subtitle"))
            .await
            .is_err(), "parser must claim the extension");
        assert!(set_lyric_parser_selection(&cx, &app_document_dir, "lrc", Some("com.ease.other:lrc"))
            .await
            .is_err(), "plugin must be installed");
        let sel = set_lyric_parser_selection(&cx, &app_document_dir, "srt", None)
            .await
            .unwrap();
        assert!(!sel.contains_key("srt"));

        // Uninstall drops the plugin's selection entries. (The in-memory
        // registry snapshot refreshes on the next `scan` — Kotlin rescans
        // after every generation bump.)
        uninstall(&cx, &app_document_dir, "com.ease.test").await.unwrap();
        assert!(read_state(&app_document_dir).lyric_parser_selection.is_empty());
    }

    #[test]
    fn cache_filename_is_md5_hex() {
        let f = registry_cache_file("/data", "https://example.com/reg");
        assert_eq!(
            f.file_name().unwrap().to_str().unwrap(),
            format!("{:x}", md5::Md5::digest(b"https://example.com/reg")) + ".json"
        );
    }

    /// Uninstall must wipe all plugin data: storage rows, `plugin_kv` and
    /// scoped secrets — while other plugins' and host-owned (`internal`)
    /// rows survive.
    #[test]
    fn uninstall_wipes_plugin_data() {
        use crate::repositories::secret::SecretStore;
        use ease_client_schema::{PluginId, PluginStorageId, SecretScope, StorageHandle};

        const A: &str = "com.ease.a";
        const B: &str = "com.ease.b";

        let (guard, root) = temp_root();
        let app_document_dir = guard.path().to_str().unwrap().to_string();
        install_zip_bytes_blocking(
            &root,
            make_zip(&[
                ("manifest.json", &manifest_json(A, "1.0.0")),
                ("backend.js", "export function start() {}"),
            ]),
        )
        .unwrap();
        install_zip_bytes_blocking(&root, make_zip(&[("manifest.json", &manifest_json(B, "1.0.0"))]))
            .unwrap();

        let cx = crate::ctx::BackendContext::new();
        ease_client_tokio::tokio_runtime().block_on(async {
            let db_server = cx.database_server();
            db_server.init(app_document_dir.clone()).await.unwrap();

            // Seed: KV (single + multi), one plugin-scoped secret and one
            // storage row per plugin, plus one host-owned secret.
            for p in [A, B] {
                db_server.plugin_kv_single_set(p, "cfg", "v1").await.unwrap();
                db_server
                    .plugin_kv_multi_append(p, "events", "e1")
                    .await
                    .unwrap();
            }
            let a_secret = db_server
                .secret_put(SecretScope::Plugin(PluginId::new(A)), "tok-a".into())
                .await
                .unwrap();
            let b_secret = db_server
                .secret_put(SecretScope::Plugin(PluginId::new(B)), "tok-b".into())
                .await
                .unwrap();
            let internal_secret = db_server
                .secret_put(SecretScope::Internal, "internal".into())
                .await
                .unwrap();
            for (p, instance) in [(A, "a:1"), (B, "b:1")] {
                db_server
                    .obtain_storage(&StorageHandle::Plugin {
                        plugin_id: PluginId::new(p),
                        plugin_storage_id: PluginStorageId::new(instance),
                    })
                    .await
                    .unwrap();
            }

            // Uninstall A.
            let generation = uninstall(&cx, &app_document_dir, A).await.unwrap();

            // A's data is gone…
            assert!(db_server.plugin_kv_single_get(A, "cfg").await.unwrap().is_none());
            assert!(db_server.plugin_kv_list_keys(A, "").await.unwrap().is_empty());
            assert!(
                db_server
                    .secret_get(SecretScope::Plugin(PluginId::new(A)), a_secret)
                    .await
                    .unwrap()
                    .is_none()
            );
            let db = db_server.db();
            let a_storages = storage::Entity::find()
                .filter(storage::Column::PluginId.eq(A))
                .all(&db)
                .await
                .unwrap();
            assert!(a_storages.is_empty());
            assert!(!is_installed(&root, A));

            // …B's and the host's data survives.
            assert_eq!(
                db_server.plugin_kv_single_get(B, "cfg").await.unwrap().as_deref(),
                Some("v1")
            );
            assert_eq!(db_server.plugin_kv_list_keys(B, "").await.unwrap().len(), 2);
            assert_eq!(
                db_server
                    .secret_get(SecretScope::Plugin(PluginId::new(B)), b_secret)
                    .await
                    .unwrap()
                    .as_deref(),
                Some("tok-b")
            );
            assert_eq!(
                db_server
                    .secret_get(SecretScope::Internal, internal_secret)
                    .await
                    .unwrap()
                    .as_deref(),
                Some("internal")
            );
            let b_storages = storage::Entity::find()
                .filter(storage::Column::PluginId.eq(B))
                .all(&db)
                .await
                .unwrap();
            assert_eq!(b_storages.len(), 1);
            assert!(is_installed(&root, B));
            assert!(generation > 0);
        });
    }

    /// A recording engine host for tests — hands out deterministic
    /// module-source handles (id * 100 + n) and remembers what was asked
    /// of it.
    struct RecordingHost {
        id: i64,
        registered: std::sync::Mutex<Vec<String>>,
    }

    impl PluginEngineHost for RecordingHost {
        fn id(&self) -> i64 {
            self.id
        }

        fn register_source(&self, src: String) -> i64 {
            let mut w = self.registered.lock().unwrap();
            w.push(src);
            (self.id * 100 + w.len() as i64) as i64
        }

        fn read_bundled_asset(&self, _path: &str) -> Option<Vec<u8>> {
            None
        }

        fn spawn_headless(&self, _plugin_id: &str) -> anyhow::Result<i64> {
            Ok(self.id + 1)
        }

        fn load_headless_module(&self, _instance_handle: i64, _source_handle: i64) {}

        fn extract_rpc(&self, _instance_handle: i64) -> Option<ease_tur_rpc::RpcClient> {
            None
        }

        fn close_headless(&self, _instance_handle: i64) {}

        fn emit_signal(&self, _op: u32, _payload: &str) {}
    }

    /// `detach_engine_host` is compare-and-set: a stale teardown (racing a
    /// newer `bindPluginRuntime`) must never clobber the fresh binding, and
    /// an unbound manager is a no-op miss.
    #[test]
    fn engine_host_cas_detach() {
        let shared = PluginManagerShared::default();

        // Nothing bound: detaching any id is a miss, stays unbound.
        assert!(shared.detach_engine_host(7).is_none());
        assert!(shared.engine_host().is_none());

        // Attach then detach with the right id.
        shared.attach_engine_host(std::sync::Arc::new(RecordingHost {
            id: 7,
            registered: Default::default(),
        }));
        assert_eq!(shared.engine_host().unwrap().id(), 7);
        assert!(shared.detach_engine_host(7).is_some());
        assert!(shared.engine_host().is_none());

        // Stale teardown: 7 was destroyed without unbind, 8 is now bound.
        shared.attach_engine_host(std::sync::Arc::new(RecordingHost {
            id: 8,
            registered: Default::default(),
        }));
        assert!(shared.detach_engine_host(7).is_none());
        assert_eq!(
            shared.engine_host().unwrap().id(),
            8,
            "newer binding must survive"
        );
    }

    /// A scan with an attached (recording) host registers module sources
    /// through it and reports non-zero handles.
    #[tokio::test]
    async fn scan_registers_through_engine_host() {
        let (guard, root) = temp_root();
        let app_document_dir = guard.path().to_str().unwrap().to_string();
        install_zip_bytes_blocking(
            &root,
            make_zip(&[
                ("manifest.json", &manifest_json("com.ease.test", "1.0.0")),
                ("view.js", "export function start() {}"),
                ("backend.js", "export function start() {}"),
            ]),
        )
        .unwrap();

        let cx = crate::ctx::BackendContext::new();
        cx.database_server()
            .init(app_document_dir.clone())
            .await
            .unwrap();
        let host = std::sync::Arc::new(RecordingHost {
            id: 42,
            registered: Default::default(),
        });
        cx.plugin_manager().attach_engine_host(host.clone());
        let out = scan(&cx, &app_document_dir).await.unwrap();
        assert!(out.warnings.is_empty());
        let info = out
            .plugins
            .iter()
            .find(|p| p.id == "com.ease.test")
            .unwrap();
        assert_ne!(info.backend_source_handle, 0);
        // backend.js + view.js both went through the host.
        assert_eq!(host.registered.lock().unwrap().len(), 2);
    }

    /// A scan with no runtime bound must come back with zero module-source
    /// handles AND a warning saying why — this is the exact state that used
    /// to blank plugin pages silently.
    #[tokio::test]
    async fn scan_without_runtime_warns() {
        let (guard, root) = temp_root();
        let app_document_dir = guard.path().to_str().unwrap().to_string();
        install_zip_bytes_blocking(
            &root,
            make_zip(&[
                ("manifest.json", &manifest_json("com.ease.test", "1.0.0")),
                ("view.js", "export function start() {}"),
                ("backend.js", "export function start() {}"),
            ]),
        )
        .unwrap();

        let cx = crate::ctx::BackendContext::new();
        cx.database_server()
            .init(app_document_dir.clone())
            .await
            .unwrap();
        let out = scan(&cx, &app_document_dir).await.unwrap();
        assert!(
            out.warnings
                .iter()
                .any(|w| w.contains("no tur runtime bound"))
        );
        let info = out
            .plugins
            .iter()
            .find(|p| p.id == "com.ease.test")
            .unwrap();
        assert_eq!(info.backend_source_handle, 0);
        assert!(info.dashboard.iter().all(|c| c.source_handle == 0));
    }
}
