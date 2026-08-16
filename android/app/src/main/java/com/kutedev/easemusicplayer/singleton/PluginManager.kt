package com.kutedev.easemusicplayer.singleton

import android.content.Context
import android.net.Uri
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.io.File
import java.io.FileOutputStream
import java.util.UUID
import java.util.zip.ZipFile
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Plugin install layer.
 *
 * Plugins live as folders under `filesDir/plugins/<id>/` (manifest.json +
 * JS bundles). This singleton owns every mutation of that tree —
 * install-from-zip (SAF sideload), install-from-registry (network,
 * sha256-verified), enable/disable, uninstall — plus the first-run
 * bootstrap. Every mutation rewrites [PluginStateStore], rescans
 * [PluginRepository] and bumps [revision]; `KeepBackendService` collects
 * the revision to tear down + reload the affected JS backends.
 *
 * Install validation: `manifest.json` at the zip root, a sane plugin id
 * (`^[A-Za-z0-9._-]+$`), sanitized entry names (no `..` / absolute /
 * backslash paths), ≤200 entries, ≤20 MB total. Extraction goes to a
 * staging dir, then atomically swaps into place (overwrite = upgrade).
 */
@Singleton
class PluginManager @Inject constructor(
    private val bridge: Bridge,
    private val pluginRepository: PluginRepository,
    private val stateStore: PluginStateStore,
    private val registryRepository: PluginRegistryRepository,
    private val storageRepository: StorageRepository,
    @ApplicationContext private val context: Context,
) {
    companion object {
        private const val MAX_ENTRIES = 200
        private const val MAX_TOTAL_BYTES = 20L * 1024 * 1024
        private val ID_REGEX = Regex("^[A-Za-z0-9._-]+$")
        /** The one plugin bundled into the APK (`assets/plugin-bundles/`),
         *  auto-installed on first run so storage setup works offline. */
        const val BUNDLED_PLUGIN_ID = "com.ease.webdav"
    }

    /** Bumped on every install / uninstall / enable / disable mutation;
     *  `KeepBackendService` collects it to reload plugin backends. */
    private val _revision = MutableStateFlow(0)
    val revision = _revision.asStateFlow()

    private val installMutex = Mutex()

    // === Bootstrap =========================================================

    /**
     * First-run defaults: install the bundled WebDAV plugin, then any
     * plugin referenced by an existing storage row (upgrade path — a
     * pre-plugin-system DB carries `com.ease.webdav` / `com.ease.onedrive`
     * storages). Non-bundled referenced plugins are fetched from the
     * default registry source best-effort; on failure the storages keep
     * their rows and simply render as "removed" until the user installs
     * the plugin from the Available page. Idempotent (guarded by
     * `firstRunDone` in the persisted state).
     */
    suspend fun bootstrapDefaults() = withContext(Dispatchers.IO) {
        val state = stateStore.read()
        if (state.firstRunDone) return@withContext
        runCatching {
            installBundled(BUNDLED_PLUGIN_ID)
            storageRepository.reload()
            val referenced = storageRepository.storages.value
                .mapNotNull { (it.handle as? com.kutedev.easemusicplayer.singleton.types.StorageHandle.Plugin)?.pluginId?.id }
                .toSet()
            for (id in referenced) {
                if (id == BUNDLED_PLUGIN_ID) continue
                if (isInstalled(id)) continue
                if (!installBundled(id).isSuccess) {
                    // No bundle in the APK — best-effort registry fetch.
                    installFromRegistryById(id, PluginRegistryRepository.PRESETS.first().url)
                        .onFailure { e ->
                            bridge.logRaw(
                                "error",
                                "plugin bootstrap: could not restore '$id' (${e.message}); storage will show removed until user installs it",
                            )
                        }
                }
            }
        }.onFailure {
            bridge.logRaw("error", "plugin bootstrap failed: ${it.message}")
        }
        stateStore.mutate { it.copy(firstRunDone = true) }
        pluginRepository.scanPlugins()
        _revision.value++
    }

    // === Queries ===========================================================

    fun isInstalled(pluginId: String): Boolean =
        File(pluginRepository.pluginsRoot(), pluginId).isDirectory

    fun isEnabled(pluginId: String): Boolean =
        stateStore.read().enabled[pluginId] ?: true

    /** Compare two dotted versions ("1.10.0" > "1.9.2"); non-numeric parts
     *  compare lexicographically. Returns positive if [a] is newer. */
    fun compareVersions(a: String, b: String): Int {
        val as_ = a.split('.')
        val bs = b.split('.')
        for (i in 0 until maxOf(as_.size, bs.size)) {
            val x = as_.getOrNull(i) ?: ""
            val y = bs.getOrNull(i) ?: ""
            val xn = x.toIntOrNull()
            val yn = y.toIntOrNull()
            val cmp = when {
                xn != null && yn != null -> xn.compareTo(yn)
                else -> x.compareTo(y)
            }
            if (cmp != 0) return cmp
        }
        return 0
    }

    // === Install ===========================================================

    /** Install (or upgrade) from a user-picked zip (SAF). */
    suspend fun installFromUri(uri: Uri): Result<String> = withContext(Dispatchers.IO) {
        runCatching {
            val temp = File.createTempFile("plugin-sideload", ".zip", context.cacheDir)
            context.contentResolver.openInputStream(uri).use { input ->
                checkNotNull(input) { "cannot open $uri" }
                FileOutputStream(temp).use { input.copyTo(it) }
            }
            installFromZipFile(temp).also { temp.delete() }.getOrThrow()
        }
    }

    /** Install (or upgrade) one entry from a registry source (network,
     *  sha256-verified download). */
    suspend fun downloadAndInstall(entry: RegistryPluginEntry, baseUrl: String): Result<String> =
        withContext(Dispatchers.IO) {
            runCatching {
                registryRepository.rememberSource(baseUrl)
                val zip = registryRepository.downloadZip(entry, baseUrl).getOrThrow()
                try {
                    installFromZipFile(zip).getOrThrow()
                } finally {
                    zip.delete()
                }
            }
        }

    private suspend fun installFromRegistryById(pluginId: String, baseUrl: String): Result<String> {
        val entries = registryRepository.fetchRegistry(baseUrl).getOrElse { e ->
            return Result.failure(e)
        }
        val entry = entries.firstOrNull { it.id == pluginId }
            ?: return Result.failure(IllegalStateException("'$pluginId' not in registry"))
        return downloadAndInstall(entry, baseUrl)
    }

    /** Extract a bundled zip from `assets/plugin-bundles/<id>.zip`. */
    private suspend fun installBundled(pluginId: String): Result<String> = withContext(Dispatchers.IO) {
        runCatching {
            if (isInstalled(pluginId)) return@runCatching pluginId
            val temp = File.createTempFile("plugin-bundled", ".zip", context.cacheDir)
            context.assets.open("plugin-bundles/$pluginId.zip").use { input ->
                FileOutputStream(temp).use { input.copyTo(it) }
            }
            installFromZipFile(temp).also { temp.delete() }.getOrThrow()
        }
    }

    /**
     * Validate + extract a plugin zip. On success the plugin folder is
     * swapped into `filesDir/plugins/<id>/` and enabled. Re-installing an
     * existing plugin is an upgrade (folder replaced).
     */
    private suspend fun installFromZipFile(zip: File): Result<String> = installMutex.withLock {
        withContext(Dispatchers.IO) {
            runCatching {
                val root = pluginRepository.pluginsRoot()
                val staging = File(root, ".staging-${UUID.randomUUID()}")
                try {
                    ZipFile(zip).use { z ->
                        val entries = z.entries().asSequence().toList()
                        check(entries.size <= MAX_ENTRIES) { "too many entries (${entries.size})" }
                        var total = 0L
                        val files = mutableListOf<Pair<java.util.zip.ZipEntry, File>>()
                        for (e in entries) {
                            val name = e.name
                            check(!name.startsWith("/") && !name.contains("..") && !name.contains('\\')) {
                                "unsafe entry: $name"
                            }
                            if (e.isDirectory) continue
                            total += e.size
                            check(total <= MAX_TOTAL_BYTES) { "zip too large" }
                            val dest = File(staging, name)
                            dest.parentFile?.mkdirs()
                            files.add(e to dest)
                        }
                        val manifestEntry = entries.firstOrNull { it.name == "manifest.json" }
                        checkNotNull(manifestEntry) { "manifest.json missing at zip root" }
                        for ((entry, dest) in files) {
                            z.getInputStream(entry).use { input ->
                                FileOutputStream(dest).use { input.copyTo(it) }
                            }
                        }
                        val manifest = JSONObject(File(staging, "manifest.json").readText())
                        val id = manifest.optString("id")
                        check(id.matches(ID_REGEX)) { "invalid plugin id: '$id'" }
                        val target = File(root, id)
                        if (target.exists()) target.deleteRecursively()
                        check(staging.renameTo(target)) { "install move failed" }
                        stateStore.mutate { s ->
                            s.copy(enabled = s.enabled + (id to true))
                        }
                        bridge.logRaw("info", "plugin installed: $id ${manifest.optString("version")}")
                        id
                    }
                } finally {
                    if (staging.exists()) staging.deleteRecursively()
                }
            }
        }.also {
            if (it.isSuccess) {
                pluginRepository.scanPlugins()
                _revision.value++
            } else {
                bridge.logRaw("error", "plugin install failed: ${it.exceptionOrNull()?.message}")
            }
        }
    }

    // === Enable / disable / uninstall ======================================

    suspend fun setEnabled(pluginId: String, enabled: Boolean) {
        stateStore.mutate { s -> s.copy(enabled = s.enabled + (pluginId to enabled)) }
        pluginRepository.scanPlugins()
        _revision.value++
        bridge.logRaw("info", "plugin ${if (enabled) "enabled" else "disabled"}: $pluginId")
    }

    /**
     * Uninstall: delete the plugin folder + its enabled flag. The plugin's
     * persisted data (`plugin_kv`, secrets) and any storage rows survive —
     * storages whose provider is gone render as "removed" and come back if
     * the plugin is reinstalled.
     */
    suspend fun uninstall(pluginId: String) {
        withContext(Dispatchers.IO) {
            File(pluginRepository.pluginsRoot(), pluginId).deleteRecursively()
        }
        stateStore.mutate { s -> s.copy(enabled = s.enabled - pluginId) }
        pluginRepository.scanPlugins()
        _revision.value++
        bridge.logRaw("info", "plugin uninstalled: $pluginId")
    }
}
