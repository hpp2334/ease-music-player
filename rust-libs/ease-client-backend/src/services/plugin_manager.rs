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
use sea_orm::EntityTrait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::error::{BError, BResult};

// ============================================================================
// Constants
// ============================================================================

/// The one plugin bundled into the APK (`assets/plugin-bundles/`), installed
/// on first run so storage setup works offline.
pub const BUNDLED_PLUGIN_ID: &str = "com.ease.webdav";

const MAX_ENTRIES: usize = 200;
const MAX_TOTAL_BYTES: u64 = 20 * 1024 * 1024;

/// Cap for contribution icons read during the scan (base64 into the
/// `plugin.list` payload — keep small).
const MAX_ICON_BYTES: u64 = 128 * 1024;

// TODO: switch REPO_REF to `main` once feat/v0.4 merges.
const REPO: &str = "hpp2334/ease-music-player";
const REPO_REF: &str = "feat/v0.4";

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
}

/// Per-process shared manager state, held by [`crate::ctx::BackendContext`].
///
/// - `generation` — bumped on every install/uninstall/enable/disable
///   mutation; Kotlin mirrors it into its `revision` StateFlow.
/// - `install_lock` — serializes installs (staging + atomic swap).
/// - `runtime_handle` — the tur runtime handle from `bindPluginRuntime`
///   (0 = not bound yet; scans then return zero module-source handles).
/// - `asset_manager` — raw `*mut AAssetManager` (as usize) for reading
///   bundled APK assets during bootstrap. Android-only; 0 elsewhere.
#[derive(Default)]
pub struct PluginManagerShared {
    generation: AtomicU64,
    pub install_lock: tokio::sync::Mutex<()>,
    runtime_handle: RwLock<i64>,
    asset_manager: RwLock<usize>,
}

impl PluginManagerShared {
    pub fn generation(&self) -> i64 {
        self.generation.load(AtomicOrdering::SeqCst) as i64
    }

    pub fn bump_generation(&self) -> i64 {
        self.generation.fetch_add(1, AtomicOrdering::SeqCst) as i64 + 1
    }

    pub fn runtime_handle(&self) -> i64 {
        *self.runtime_handle.read().unwrap()
    }

    pub fn set_runtime_handle(&self, handle: i64) {
        *self.runtime_handle.write().unwrap() = handle;
    }

    pub fn asset_manager(&self) -> usize {
        *self.asset_manager.read().unwrap()
    }

    pub fn set_asset_manager(&self, mgr: usize) {
        *self.asset_manager.write().unwrap() = mgr;
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
    pub dashboard: Vec<ContributionRaw>,
    pub storages: Vec<ContributionRaw>,
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
    use base64::Engine as _;
    Some(base64::engine::general_purpose::STANDARD.encode(bytes))
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
            // Contribution icons: read here (blocking thread), regardless of
            // enabled state — the management page shows disabled plugins too.
            for c in m.dashboard.iter_mut().chain(m.storages.iter_mut()) {
                if let Some(icon) = c.icon.as_deref() {
                    c.icon_data = load_icon_base64(&path, icon);
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

/// Register `src` on the bound runtime's shared `ModuleSourceRegistry` and
/// return the opaque handle (0 when no runtime is bound or the handle is
/// stale). Thread-safe (the registry is mutex-guarded), so calling from the
/// bridge dispatcher's IO thread is fine.
pub fn register_module_source(runtime_handle: i64, src: String) -> i64 {
    if runtime_handle == 0 {
        return 0;
    }
    #[cfg(target_os = "android")]
    {
        tur_android::ops::with_runtime(runtime_handle, |rt| rt.module_sources.register(src) as i64)
            .unwrap_or(0)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = src;
        0
    }
}

fn read_registered(root: &Path, plugin_id: &str, file: &str, runtime_handle: i64) -> i64 {
    match std::fs::read_to_string(root.join(plugin_id).join(file)) {
        Ok(src) => register_module_source(runtime_handle, src),
        Err(_) => 0,
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
    runtime_handle: i64,
) -> Vec<ContributionInfo> {
    list.into_iter()
        .map(|c| ContributionInfo {
            source_handle: if enabled {
                c.view
                    .as_deref()
                    .map(|f| read_registered(root, plugin_id, f, runtime_handle))
                    .unwrap_or(0)
            } else {
                0
            },
            id: c.id,
            title: c.title,
            desc: c.desc,
            icon: c.icon,
            icon_data: c.icon_data,
            view: c.view,
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
    let entries = parse_registry(&body)?;
    let cache = registry_cache_file(app_document_dir, base_url);
    if let Some(parent) = cache.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&cache, &body);
    Ok(entries)
}

/// The last cached registry for `base_url`, if any (offline fallback).
pub fn cached_registry(app_document_dir: &str, base_url: &str) -> Option<Vec<RegistryEntry>> {
    let f = registry_cache_file(app_document_dir, base_url);
    let body = std::fs::read_to_string(f).ok()?;
    parse_registry(&body).ok()
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

/// Resolve an entry's zip URL against the source base URL.
pub fn entry_zip_url(entry: &RegistryEntry, base_url: &str) -> String {
    if entry.zip.starts_with("http") {
        entry.zip.clone()
    } else {
        format!(
            "{}/{}",
            base_url.trim_end_matches('/'),
            entry.zip.trim_start_matches('/')
        )
    }
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
    Ok(cx.plugin_manager().bump_generation())
}

/// Uninstall: delete the plugin folder + its enabled flag. The plugin's
/// persisted data (`plugin_kv`, secrets) and any storage rows survive —
/// storages whose provider is gone render as "removed" and come back if the
/// plugin is reinstalled.
pub async fn uninstall(
    cx: &crate::ctx::BackendContext,
    app_document_dir: &str,
    plugin_id: &str,
) -> BResult<i64> {
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
    mutate_state(app_document_dir, |s| PluginState {
        enabled: {
            let mut enabled = s.enabled;
            enabled.remove(plugin_id);
            enabled
        },
        ..s
    })?;
    tracing::info!("plugin uninstalled: {plugin_id}");
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

/// Read one bundled APK asset (Android-only; `mgr` is the raw
/// `*mut AAssetManager` stashed by the `bindPluginRuntime` trampoline).
#[cfg(target_os = "android")]
pub(crate) fn read_bundled_asset(mgr: usize, path: &str) -> Option<Vec<u8>> {
    crate::plugin_runtime::read_asset_bytes(mgr, path)
}

#[cfg(not(target_os = "android"))]
pub(crate) fn read_bundled_asset(_mgr: usize, _path: &str) -> Option<Vec<u8>> {
    None
}

/// First-run defaults: install the bundled WebDAV plugin, then any plugin
/// referenced by an existing storage row (upgrade path). Non-bundled
/// referenced plugins are fetched from the default registry source
/// best-effort. Idempotent (guarded by `firstRunDone`).
pub async fn bootstrap(cx: &crate::ctx::BackendContext, app_document_dir: &str) -> BResult<i64> {
    let shared = cx.plugin_manager();
    {
        let state = read_state(app_document_dir);
        if state.first_run_done {
            return Ok(shared.generation());
        }
    }

    let root = plugins_root(app_document_dir);

    // 1) Bundled WebDAV (offline-friendly).
    if !is_installed(&root, BUNDLED_PLUGIN_ID) {
        let mgr = shared.asset_manager();
        match read_bundled_asset(mgr, &format!("plugin-bundles/{BUNDLED_PLUGIN_ID}.zip")) {
            Some(bytes) => {
                if let Err(e) = install_bytes_and_enable(cx, app_document_dir, bytes).await {
                    tracing::error!("plugin bootstrap: bundled install failed: {e}");
                }
            }
            None => {
                tracing::error!("plugin bootstrap: bundled zip missing (asset manager not bound?)");
            }
        }
    }

    // 2) Plugins referenced by existing storage rows.
    let referenced: Vec<String> = collect_storage_plugin_ids(cx).await;
    for id in referenced {
        if id == BUNDLED_PLUGIN_ID || is_installed(&root, &id) {
            continue;
        }
        // Best-effort bundled install first (none besides webdav today),
        // then the default registry source.
        let mgr = shared.asset_manager();
        let bundled = read_bundled_asset(mgr, &format!("plugin-bundles/{id}.zip"));
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
    Ok(shared.bump_generation())
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
    pub dashboard: Vec<ContributionInfo>,
    pub storages: Vec<ContributionInfo>,
    pub enabled: bool,
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
/// bumps the generation and the service rescans).
pub async fn scan(
    cx: &crate::ctx::BackendContext,
    app_document_dir: &str,
) -> BResult<PluginListOut> {
    let root = plugins_root(app_document_dir);
    let dir = app_document_dir.to_string();
    let runtime_handle = cx.plugin_manager().runtime_handle();
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
                dashboard,
                storages,
            } = m;
            let enabled = state.enabled.get(&id).copied().unwrap_or(true);
            let backend_source_handle = if enabled {
                backend
                    .as_deref()
                    .map(|f| read_registered(&root, &id, f, runtime_handle))
                    .unwrap_or(0)
            } else {
                0
            };
            let dashboard_infos =
                contribution_infos(dashboard, enabled, &root, &id, runtime_handle);
            let storages_infos = contribution_infos(storages, enabled, &root, &id, runtime_handle);
            PluginScanInfo {
                id,
                name,
                version,
                description,
                backend,
                backend_source_handle,
                events,
                dashboard: dashboard_infos,
                storages: storages_infos,
                enabled,
            }
        })
        .collect();
    Ok(PluginListOut {
        generation: cx.plugin_manager().generation(),
        plugins,
    })
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PluginListOut {
    pub generation: i64,
    pub plugins: Vec<PluginScanInfo>,
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
            {"id":"com.ease.a","name":"A","version":"1.0.0","zip":"zips/a.zip","sha256":"00","size":10},
            {"id":"com.ease.b","version":"2.0.0"}
        ]}"#;
        let mut entries = parse_registry(body).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, LocalizedString::plain("A".into()));
        assert_eq!(entries[1].name, LocalizedString::plain("com.ease.b".into()));

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
    fn cache_filename_is_md5_hex() {
        let f = registry_cache_file("/data", "https://example.com/reg");
        assert_eq!(
            f.file_name().unwrap().to_str().unwrap(),
            format!("{:x}", md5::Md5::digest(b"https://example.com/reg")) + ".json"
        );
    }
}
