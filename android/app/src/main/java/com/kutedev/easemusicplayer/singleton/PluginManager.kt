package com.kutedev.easemusicplayer.singleton

import android.content.Context
import android.net.Uri
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.withContext
import java.io.File
import java.io.FileOutputStream
import java.util.UUID
import javax.inject.Inject
import javax.inject.Singleton
import com.kutedev.easemusicplayer.singleton.types.ArgPluginBaseUrl
import com.kutedev.easemusicplayer.singleton.types.ArgPluginId
import com.kutedev.easemusicplayer.singleton.types.ArgPluginInstallFromRegistry
import com.kutedev.easemusicplayer.singleton.types.ArgPluginInstallZipPath
import com.kutedev.easemusicplayer.singleton.types.ArgPluginSetEnable
import com.kutedev.easemusicplayer.singleton.types.RegistryPluginEntry

/**
 * Kotlin facade over the Rust-side plugin install layer
 * (`services/plugin_manager.rs`, reached via `plugin.*` bridge methods).
 * The Rust side owns the install tree (`filesDir/plugins/`), the persisted
 * state, the registry fetch/download, sha256 verification, and the
 * manifest scan; this class keeps only platform glue: the SAF picker
 * copy (a `content://` stream Rust cannot open — stream-copied to a
 * cache temp file, then handed over by **path**, never by bytes) and the
 * `revision` flow `KeepBackendService` collects to reload JS backends.
 *
 * Every bridge mutation returns a monotonic `generation` from Rust; it is
 * mirrored into [revision] (StateFlow dedups equal values).
 */
@Singleton
class PluginManager @Inject constructor(
    private val bridge: Bridge,
    private val pluginRepository: PluginRepository,
    @ApplicationContext private val context: Context,
) {
    /** Bumped (from the Rust generation) on every install / uninstall /
     *  enable / disable mutation; `KeepBackendService` collects it to
     *  reload plugin backends. */
    private val _revision = MutableStateFlow(0L)
    val revision = _revision.asStateFlow()

    // === Bootstrap =========================================================

    /**
     * First-run defaults (Rust): install the bundled WebDAV zip — read
     * natively via the NDK AssetManager stashed by `bindPluginRuntime` —
     * then any plugin referenced by an existing storage row (bundled, else
     * best-effort from the default registry source). Idempotent (guarded
     * by `firstRunDone` in the Rust-side persisted state).
     */
    suspend fun bootstrapDefaults() {
        val result = bridge.call(BridgeMethods.Plugin.BOOTSTRAP).unwrapOrNull()?.payload
        _revision.value = result?.generation ?: _revision.value
        pluginRepository.scanPlugins()
    }

    // === Queries ===========================================================

    fun isInstalled(pluginId: String): Boolean =
        pluginRepository.installedPlugins.value.any { it.id == pluginId }

    // === Install ===========================================================

    /** Install (or upgrade) from a user-picked zip (SAF). The
     *  `content://` stream is copied to a cache temp file here (plain IO —
     *  no large payload crosses JNI); Rust reads/validates/installs it by
     *  path. */
    suspend fun installFromUri(uri: Uri): Result<String> =
        withContext(Dispatchers.IO) {
            runCatching {
                val temp = File.createTempFile("plugin-sideload", ".zip", context.cacheDir)
                try {
                    context.contentResolver.openInputStream(uri).use { input ->
                        checkNotNull(input) { "cannot open $uri" }
                        FileOutputStream(temp).use { input.copyTo(it) }
                    }
                    bridge.call(
                        BridgeMethods.Plugin.INSTALL_ZIP_PATH,
                        ArgPluginInstallZipPath(temp.absolutePath),
                    ).unwrapOrThrow().also { ret ->
                        applyGeneration(ret.payload.generation)
                        pluginRepository.scanPlugins()
                    }.payload.id ?: error("no id in result")
                } finally {
                    temp.delete()
                }
            }.onFailure {
                bridge.logRaw("error", "plugin install failed: ${it.message}")
            }
        }

    /** Install (or upgrade) one entry from a registry source (Rust
     *  downloads + sha256-verifies + installs). */
    suspend fun downloadAndInstall(entry: RegistryPluginEntry, baseUrl: String): Result<String> =
        runCatching {
            val ret = bridge.call(
                BridgeMethods.Plugin.INSTALL_FROM_REGISTRY,
                ArgPluginInstallFromRegistry(entry, baseUrl),
            ).unwrapOrThrow()
            applyGeneration(ret.payload.generation)
            ret.payload.id ?: error("no id in result")
        }.onFailure {
            bridge.logRaw("error", "plugin install failed: ${it.message}")
        }

    // === Enable / disable / uninstall ======================================

    suspend fun setEnabled(pluginId: String, enabled: Boolean) {
        val ret = bridge.call(
            BridgeMethods.Plugin.SET_ENABLE,
            ArgPluginSetEnable(pluginId, enabled),
        ).unwrapOrNull()?.payload
        applyGeneration(ret?.generation)
        pluginRepository.scanPlugins()
    }

    /**
     * Uninstall: Rust deletes the plugin folder + its enabled flag. The
     * plugin's persisted data (`plugin_kv`, secrets) and any storage rows
     * survive — storages whose provider is gone render as "removed" and
     * come back if the plugin is reinstalled.
     */
    suspend fun uninstall(pluginId: String) {
        val ret = bridge.call(
            BridgeMethods.Plugin.UNINSTALL,
            ArgPluginId(pluginId),
        ).unwrapOrNull()?.payload
        applyGeneration(ret?.generation)
        pluginRepository.scanPlugins()
    }

    private fun applyGeneration(generation: Long?) {
        if (generation != null && generation > _revision.value) {
            _revision.value = generation
        }
    }
}
