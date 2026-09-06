package com.kutedev.easemusicplayer.singleton.types

import kotlinx.serialization.Serializable
import com.kutedev.easemusicplayer.singleton.LocalizedText

// ============================================================================
// Plugin-manager wire types — mirror of Rust `services/plugin_manager.rs`
// (`plugin.list` / `plugin.registryFetch` / sources CRUD). Only small JSON
// crosses the bridge; plugin JS + zips are handled entirely Rust-side.
// ============================================================================

@Serializable
data class ArgPluginInstallZipPath(
    val path: String,
)

@Serializable
data class ArgPluginInstallFromRegistry(
    val entry: RegistryPluginEntry,
    val baseUrl: String,
)

@Serializable
data class ArgPluginSetEnable(
    val pluginId: String,
    val enabled: Boolean,
)

@Serializable
data class ArgPluginSetLyricParserSelection(
    /** File extension, lowercase without the dot (e.g. `"srt"`). */
    val ext: String,
    /** `"<pluginId>:<parserId>"`, or `null` to clear back to Auto. */
    val parser: String? = null,
)

@Serializable
data class ArgPluginId(
    val pluginId: String,
)

@Serializable
data class ArgPluginBaseUrl(
    val baseUrl: String,
)

@Serializable
data class ArgPluginSourceAddCustom(
    val url: String,
    val label: String? = null,
)

@Serializable
data class PluginMutationResult(
    val id: String? = null,
    val generation: Long = 0,
)

@Serializable
data class PluginListResult(
    val generation: Long = 0,
    val plugins: List<PluginScanInfo> = emptyList(),
    /** User's per-extension lyric-parser picks (extension →
     * `"<pluginId>:<parserId>"`); absent entry = Auto. */
    val lyricParserSelection: Map<String, String> = emptyMap(),
)

@Serializable
data class PluginScanInfo(
    val id: String,
    val name: LocalizedText,
    val version: String = "0.0.0",
    val description: LocalizedText = LocalizedText(""),
    val backend: String? = null,
    val backendSourceHandle: Long = 0,
    val events: List<String> = emptyList(),
    /** Base64 plugin icon bytes; the built-in extension glyph shows when absent. */
    val iconData: String? = null,
    val dashboard: List<PluginContributionInfo> = emptyList(),
    val storages: List<PluginContributionInfo> = emptyList(),
    val lyricParsers: List<LyricParserInfo> = emptyList(),
    val enabled: Boolean = true,
)

/** A `contributions.lyricParsers` entry — headless (no view/source
 * handle): the plugin backend's single `lyric:parse` host-RPC handler
 * serves all of its parsers. */
@Serializable
data class LyricParserInfo(
    val id: String,
    /** `null` when the manifest omitted `title` — callers fall back to the
     * plugin name. */
    val title: LocalizedText? = null,
    val desc: LocalizedText? = null,
    val icon: String? = null,
    val iconData: String? = null,
    /** Lowercase, dot-free extensions this parser claims. */
    val extensions: List<String> = emptyList(),
)

@Serializable
data class PluginContributionInfo(
    val id: String,
    /** `null` when the manifest omitted `title` — callers fall back to the
     * plugin name (never the raw contribution id). */
    val title: LocalizedText? = null,
    /** Short one-liner (dashboard card / chooser subtitle). */
    val desc: LocalizedText? = null,
    /** Icon file name (informational; rendering goes through [iconData]). */
    val icon: String? = null,
    /** Base64 icon bytes, or `null` when the file failed validation. */
    val iconData: String? = null,
    val view: String? = null,
    val sourceHandle: Long = 0,
)

@Serializable
data class RegistryPluginEntry(
    val id: String,
    val name: LocalizedText,
    val version: String = "0.0.0",
    val description: LocalizedText = LocalizedText(""),
    /** Zip path relative to the source base URL, or an absolute http(s) URL. */
    val zip: String = "",
    val sha256: String = "",
    val size: Long = 0,
    val minAppVersion: String? = null,
    /** Base64 icon bytes fetched + cached Rust-side; `null` when the
     * registry declared no icon or the fetch failed. */
    val iconData: String? = null,
    /** Stamped by Rust at fetch time — Kotlin never compares versions. */
    val installedVersion: String? = null,
    val updateAvailable: Boolean = false,
)

@Serializable
data class RegistryEntriesResult(
    val entries: List<RegistryPluginEntry> = emptyList(),
)

@Serializable
data class PluginSource(
    val url: String,
    val label: String,
    val preset: Boolean,
)

@Serializable
data class PluginSourcesResult(
    val presets: List<PluginSource> = emptyList(),
    val customSources: List<PluginSource> = emptyList(),
    /** Rust drops a stale pin (matches no preset/custom) to null. */
    val lastSourceUrl: String? = null,
)
