package com.kutedev.easemusicplayer.singleton

import com.kutedev.easemusicplayer.singleton.types.ArgPluginBaseUrl
import com.kutedev.easemusicplayer.singleton.types.ArgPluginSourceAddCustom
import com.kutedev.easemusicplayer.singleton.types.PluginSource
import com.kutedev.easemusicplayer.singleton.types.RegistryPluginEntry
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Kotlin facade over the Rust-side remote-registry access
 * (`plugin.registryFetch` / `registryCached` / `plugin.sources*` bridge
 * methods). A *source* is a base URL serving `plugins.json` + the `zips/…`
 * archives it references; four hard-coded presets (Rust constants) plus
 * user-added custom sources (verified on add, persisted Rust-side). The
 * registry index is cached per source (md5 filename — existing on-device
 * caches survive) so the Available page works offline. All network IO,
 * sha256 verification, and `installedVersion`/`updateAvailable` stamping
 * happen in Rust; entries arrive pre-stamped so Kotlin never compares
 * versions.
 */
@Singleton
class PluginRegistryRepository @Inject constructor(
    private val bridge: Bridge,
) {
    /** All selectable sources (presets first, then saved customs). */
    suspend fun sources(): PluginSourcesInfo {
        val r = bridge.call(BridgeMethods.Plugin.SOURCES_LIST).unwrapOrNull()?.payload
            ?: return PluginSourcesInfo(emptyList(), emptyList(), null)
        return PluginSourcesInfo(r.presets, r.customSources, r.lastSourceUrl)
    }

    /** The default source url (first preset). */
    suspend fun defaultSourceUrl(): String =
        sources().presets.firstOrNull()?.url ?: ""

    /** The last successfully used source url (null → first preset). Rust
     *  drops a stale pin (e.g. a preset whose ref changed after an app
     *  update) so the page self-heals to the default. */
    suspend fun lastSourceUrl(): String? = sources().lastSourceUrl

    suspend fun rememberSource(url: String) {
        bridge.call(BridgeMethods.Plugin.SOURCE_REMEMBER, ArgPluginBaseUrl(url)).unwrapOrNull()
    }

    /** Verify + persist a custom source. Returns the parsed (stamped)
     *  entries on success; the caller toasts the failure otherwise. */
    suspend fun addCustomSource(url: String, label: String? = null): Result<List<RegistryPluginEntry>> =
        runCatching {
            bridge.call(
                BridgeMethods.Plugin.SOURCE_ADD_CUSTOM,
                ArgPluginSourceAddCustom(url, label),
            ).unwrapOrThrow().payload.entries
        }

    suspend fun removeCustomSource(url: String) {
        bridge.call(BridgeMethods.Plugin.SOURCE_REMOVE_CUSTOM, ArgPluginBaseUrl(url)).unwrapOrNull()
    }

    /** Fetch `<base>/plugins.json` (network, Rust-side; caches the body on
     *  success). Does NOT change the selected source. */
    suspend fun fetchRegistry(baseUrl: String): Result<List<RegistryPluginEntry>> =
        runCatching {
            bridge.call(BridgeMethods.Plugin.REGISTRY_FETCH, ArgPluginBaseUrl(baseUrl))
                .unwrapOrThrow().payload.entries
        }

    /** The last cached registry for [baseUrl], if any (offline fallback). */
    suspend fun cachedRegistry(baseUrl: String): List<RegistryPluginEntry>? =
        bridge.call(BridgeMethods.Plugin.REGISTRY_CACHED, ArgPluginBaseUrl(baseUrl))
            .unwrapOrNull()?.payload?.entries?.takeIf { it.isNotEmpty() }
}

/** Flattened `plugin.sourcesList` result. */
data class PluginSourcesInfo(
    val presets: List<PluginSource>,
    val customSources: List<PluginSource>,
    val lastSourceUrl: String?,
) {
    val all: List<PluginSource> get() = presets + customSources
}
