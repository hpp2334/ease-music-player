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
    val dashboard: List<PluginContributionInfo> = emptyList(),
    val storages: List<PluginContributionInfo> = emptyList(),
    val enabled: Boolean = true,
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
