package com.kutedev.easemusicplayer.singleton

import android.content.Context
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONObject
import javax.inject.Inject
import javax.inject.Singleton

/**
 * One successfully-verified custom plugin source (a base URL serving
 * `plugins.json` + the `zips/…` it references).
 */
data class PluginSource(
    val url: String,
    val label: String,
    val preset: Boolean,
)

/**
 * Persisted plugin-install state, kept as JSON in
 * `filesDir/plugin-state.json`:
 *
 * ```json
 * {
 *   "firstRunDone": false,
 *   "enabled": { "com.ease.webdav": true },
 *   "lastSourceUrl": "https://cdn.jsdelivr.net/gh/...",
 *   "customSources": [{ "url": "...", "label": "..." }]
 * }
 * ```
 *
 * All access is serialized through [read]/[write]; mutation helpers run on
 * the caller's dispatcher but the file IO itself is confined inside (a
 * mutex-free design: every write is a full-file rewrite from the latest
 * in-memory snapshot, and all writers go through this singleton).
 */
@Singleton
class PluginStateStore @Inject constructor(
    @ApplicationContext private val context: Context,
) {
    data class State(
        val firstRunDone: Boolean = false,
        val enabled: Map<String, Boolean> = emptyMap(),
        val lastSourceUrl: String? = null,
        val customSources: List<PluginSource> = emptyList(),
    )

    private val file get() = java.io.File(context.filesDir, "plugin-state.json")

    fun read(): State = synchronized(this) {
        runCatching {
            val text = file.readText()
            val m = JSONObject(text)
            val enabled = mutableMapOf<String, Boolean>()
            val enabledObj = m.optJSONObject("enabled")
            if (enabledObj != null) {
                for (key in enabledObj.keys()) {
                    enabled[key] = enabledObj.getBoolean(key)
                }
            }
            val customs = mutableListOf<PluginSource>()
            val arr = m.optJSONArray("customSources")
            if (arr != null) {
                for (i in 0 until arr.length()) {
                    val s = arr.optJSONObject(i) ?: continue
                    val url = s.optString("url")
                    if (url.isNotBlank()) {
                        customs.add(PluginSource(url, s.optString("label", url), preset = false))
                    }
                }
            }
            State(
                firstRunDone = m.optBoolean("firstRunDone", false),
                enabled = enabled,
                lastSourceUrl = m.optString("lastSourceUrl", "").ifBlank { null },
                customSources = customs,
            )
        }.getOrDefault(State())
    }

    fun write(state: State) = synchronized(this) {
        val m = JSONObject()
        m.put("firstRunDone", state.firstRunDone)
        m.put("enabled", JSONObject(state.enabled))
        m.put("customSources", org.json.JSONArray().apply {
            for (s in state.customSources) {
                put(JSONObject().put("url", s.url).put("label", s.label))
            }
        })
        if (state.lastSourceUrl != null) m.put("lastSourceUrl", state.lastSourceUrl)
        file.parentFile?.mkdirs()
        file.writeText(m.toString(2))
    }

    suspend fun mutate(block: (State) -> State): State = withContext(Dispatchers.IO) {
        val next = block(read())
        write(next)
        next
    }
}
