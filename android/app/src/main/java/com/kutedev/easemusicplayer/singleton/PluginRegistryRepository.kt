package com.kutedev.easemusicplayer.singleton

import android.content.Context
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.io.File
import java.io.FileOutputStream
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest
import javax.inject.Inject
import javax.inject.Singleton

/** One entry of the remote registry's `plugins.json`. */
data class RegistryPluginEntry(
    val id: String,
    val name: String,
    val version: String,
    val description: String,
    /** Zip path relative to the source base URL (e.g. `zips/<id>-<v>.zip`). */
    val zip: String,
    val sha256: String,
    val size: Long,
    val minAppVersion: String? = null,
)

/**
 * Remote plugin registry access.
 *
 * A *source* is a base URL serving `plugins.json` + the `zips/…` archives
 * it references. Four presets (hard-coded, all pointing at the repo's
 * `plugins/registry/` via jsDelivr / GitHub Raw — jsDelivr being
 * China-friendly) plus user-added custom sources (verified on add, saved
 * via [PluginStateStore]). The registry index is cached per source under
 * `filesDir/plugin-registry-cache/` so the Available page works offline
 * with a "cache" tag until refreshed.
 */
@Singleton
class PluginRegistryRepository @Inject constructor(
    private val stateStore: PluginStateStore,
    @ApplicationContext private val context: Context,
) {
    companion object {
        // TODO: switch to `main` once feat/v0.4 merges.
        const val REPO = "hpp2334/ease-music-player"
        private const val REPO_REF = "feat/v0.4"

        /** Hard-coded source presets — always offered in the source picker. */
        val PRESETS = listOf(
            PluginSource(
                "https://cdn.jsdelivr.net/gh/$REPO@$REPO_REF/plugins/registry",
                "jsDelivr (CDN)",
                preset = true,
            ),
            PluginSource(
                "https://fastly.jsdelivr.net/gh/$REPO@$REPO_REF/plugins/registry",
                "fastly.jsdelivr (CN)",
                preset = true,
            ),
            PluginSource(
                "https://gcore.jsdelivr.net/gh/$REPO@$REPO_REF/plugins/registry",
                "gcore.jsdelivr (CN)",
                preset = true,
            ),
            PluginSource(
                "https://raw.githubusercontent.com/$REPO/$REPO_REF/plugins/registry",
                "GitHub Raw",
                preset = true,
            ),
        )

        private const val CONNECT_TIMEOUT_MS = 10_000
        private const val READ_TIMEOUT_MS = 15_000
    }

    /** All selectable sources: presets first, then saved customs. */
    fun sources(): List<PluginSource> = PRESETS + stateStore.read().customSources

    /** The last successfully used source url (null → first preset). */
    fun lastSourceUrl(): String? = stateStore.read().lastSourceUrl

    suspend fun rememberSource(url: String) {
        stateStore.mutate { it.copy(lastSourceUrl = url) }
    }

    /** Verify + persist a custom source. Returns the parsed entries on
     *  success (the caller toasts the failure otherwise). */
    suspend fun addCustomSource(url: String, label: String? = null): Result<List<RegistryPluginEntry>> {
        val normalized = url.trim().trimEnd('/')
        val result = fetchRegistry(normalized)
        return result.map { entries ->
            stateStore.mutate { state ->
                if (state.customSources.any { it.url == normalized }) {
                    state
                } else {
                    state.copy(
                        customSources = state.customSources +
                            PluginSource(normalized, label ?: normalized.hostLabel(), preset = false),
                    )
                }
            }
            entries
        }
    }

    suspend fun removeCustomSource(url: String) {
        stateStore.mutate { state ->
            state.copy(
                customSources = state.customSources.filterNot { it.url == url },
                lastSourceUrl = if (state.lastSourceUrl == url) null else state.lastSourceUrl,
            )
        }
    }

    /** Fetch + parse `<base>/plugins.json` (network). Caches the body on
     *  success. Does NOT change the selected source. */
    suspend fun fetchRegistry(baseUrl: String): Result<List<RegistryPluginEntry>> =
        withContext(Dispatchers.IO) {
            runCatching {
                val body = httpGet("$baseUrl/plugins.json")
                val entries = parseRegistry(body)
                cacheFile(baseUrl).writeText(body)
                entries
            }
        }

    /** The last cached registry for [baseUrl], if any (offline fallback). */
    suspend fun cachedRegistry(baseUrl: String): List<RegistryPluginEntry>? =
        withContext(Dispatchers.IO) {
            val f = cacheFile(baseUrl)
            if (!f.isFile) return@withContext null
            runCatching { parseRegistry(f.readText()) }.getOrNull()
        }

    /** Download one entry's zip to a temp file (sha256-verified). */
    suspend fun downloadZip(entry: RegistryPluginEntry, baseUrl: String): Result<File> =
        withContext(Dispatchers.IO) {
            runCatching {
                val dest = File.createTempFile("plugin-download", ".zip", context.cacheDir)
                val url = URL(
                    if (entry.zip.startsWith("http")) entry.zip else "$baseUrl/${entry.zip.trimStart('/')}"
                )
                val conn = url.openConnection() as HttpURLConnection
                conn.connectTimeout = CONNECT_TIMEOUT_MS
                conn.readTimeout = 60_000
                try {
                    val code = conn.responseCode
                    check(code in 200..299) { "HTTP $code" }
                    FileOutputStream(dest).use { out ->
                        conn.inputStream.use { input -> input.copyTo(out) }
                    }
                } finally {
                    conn.disconnect()
                }
                val digest = MessageDigest.getInstance("SHA-256").digest(dest.readBytes())
                val hex = digest.joinToString("") { "%02x".format(it) }
                check(hex.equals(entry.sha256, ignoreCase = true)) {
                    "sha256 mismatch (expected ${entry.sha256}, got $hex)"
                }
                dest
            }
        }

    private fun httpGet(url: String): String {
        val conn = URL(url).openConnection() as HttpURLConnection
        conn.connectTimeout = CONNECT_TIMEOUT_MS
        conn.readTimeout = READ_TIMEOUT_MS
        try {
            val code = conn.responseCode
            check(code in 200..299) { "HTTP $code" }
            return conn.inputStream.bufferedReader().use { it.readText() }
        } finally {
            conn.disconnect()
        }
    }

    private fun parseRegistry(body: String): List<RegistryPluginEntry> {
        val m = JSONObject(body)
        val arr = m.optJSONArray("plugins") ?: return emptyList()
        val out = mutableListOf<RegistryPluginEntry>()
        for (i in 0 until arr.length()) {
            val e = arr.optJSONObject(i) ?: continue
            val id = e.optString("id")
            if (id.isBlank()) continue
            out.add(
                RegistryPluginEntry(
                    id = id,
                    name = e.optString("name", id),
                    version = e.optString("version", "0.0.0"),
                    description = e.optString("description", ""),
                    zip = e.optString("zip"),
                    sha256 = e.optString("sha256"),
                    size = e.optLong("size", 0),
                    minAppVersion = e.optString("minAppVersion", "").ifBlank { null },
                )
            )
        }
        return out
    }

    private fun cacheFile(baseUrl: String): File {
        val digest = MessageDigest.getInstance("MD5").digest(baseUrl.toByteArray())
        val name = digest.joinToString("") { "%02x".format(it) }
        val dir = File(context.filesDir, "plugin-registry-cache")
        dir.mkdirs()
        return File(dir, "$name.json")
    }

    private fun String.hostLabel(): String = runCatching { URL(this).host }.getOrDefault(this)
}
