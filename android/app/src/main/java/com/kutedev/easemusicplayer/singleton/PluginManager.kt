package com.kutedev.easemusicplayer.singleton

import android.content.Context
import android.net.Uri
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.Dispatchers
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
import com.kutedev.easemusicplayer.singleton.types.ArgPluginSetLyricParserSelection
import com.kutedev.easemusicplayer.singleton.types.RegistryPluginEntry

/**
 * Kotlin facade over the Rust-side plugin install layer
 * (`services/plugin_manager.rs`, reached via `plugin.*` bridge methods).
 * The Rust side owns the install tree (`filesDir/plugins/`), the persisted
 * state, the registry fetch/download, sha256 verification, the manifest
 * scan — and, since the reload is Rust-driven, the live backend set too:
 * every mutation below triggers `reload_backends` inside the backend, so
 * there is no revision flow to mirror anymore. This class keeps only
 * platform glue: the SAF picker copy (a `content://` stream Rust cannot
 * open — stream-copied to a cache temp file, then handed over by **path**,
 * never by bytes) and post-mutation UI rescans.
 */
@Singleton
class PluginManager @Inject constructor(
    private val bridge: Bridge,
    private val pluginRepository: PluginRepository,
    @ApplicationContext private val context: Context,
) {
    // === Bootstrap =========================================================

    /**
     * First-run defaults (Rust): install the bundled WebDAV zip — read
     * natively via the NDK AssetManager stashed by `bindPluginRuntime` —
     * then any plugin referenced by an existing storage row (bundled, else
     * best-effort from the default registry source). Idempotent (guarded
     * by `firstRunDone` in the Rust-side persisted state). Installs
     * trigger the Rust-side backend reload on their own.
     */
    suspend fun bootstrapDefaults() {
        bridge.call(BridgeMethods.Plugin.BOOTSTRAP).unwrapOrNull()?.payload
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
                    ).unwrapOrThrow().also {
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
            bridge.call(
                BridgeMethods.Plugin.INSTALL_FROM_REGISTRY,
                ArgPluginInstallFromRegistry(entry, baseUrl),
            ).unwrapOrThrow().payload.id ?: error("no id in result")
        }.onFailure {
            bridge.logRaw("error", "plugin install failed: ${it.message}")
        }

    // === Enable / disable / uninstall ======================================

    suspend fun setEnabled(pluginId: String, enabled: Boolean) {
        bridge.call(
            BridgeMethods.Plugin.SET_ENABLE,
            ArgPluginSetEnable(pluginId, enabled),
        ).unwrapOrNull()?.payload
        pluginRepository.scanPlugins()
    }

    /**
     * Set (or clear, with `parser = null`) the user's per-extension
     * lyric-parser pick — the Lyric Parser settings page. Pure preference:
     * Rust never bumps the generation (no backend teardown/reload), and
     * dispatch reads the new pick at the next lyric load. Throws on an
     * invalid pick (unknown plugin/parser, or a parser not claiming the
     * extension) — callers toast.
     */
    suspend fun setLyricParserSelection(ext: String, parser: String?): Result<Map<String, String>> =
        runCatching {
            val selection = bridge.call(
                BridgeMethods.Plugin.SET_LYRIC_PARSER_SELECTION,
                ArgPluginSetLyricParserSelection(ext, parser),
            ).unwrapOrThrow().payload
            pluginRepository.updateLyricParserSelection(selection)
            selection
        }

    /**
     * Uninstall: Rust deletes the plugin folder + its enabled flag. The
     * plugin's persisted data (`plugin_kv`, secrets) and any storage rows
     * survive — storages whose provider is gone render as "removed" and
     * come back if the plugin is reinstalled.
     */
    suspend fun uninstall(pluginId: String) {
        bridge.call(
            BridgeMethods.Plugin.UNINSTALL,
            ArgPluginId(pluginId),
        ).unwrapOrNull()?.payload
        pluginRepository.scanPlugins()
    }

}
