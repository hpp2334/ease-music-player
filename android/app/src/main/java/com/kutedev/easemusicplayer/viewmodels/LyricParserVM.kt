package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kutedev.easemusicplayer.singleton.PluginManager
import com.kutedev.easemusicplayer.singleton.PluginRepository
import com.kutedev.easemusicplayer.singleton.LyricParserContribution
import com.kutedev.easemusicplayer.singleton.ToastRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

/** One extension's parser group: every enabled plugin parser claiming it,
 *  in default dispatch order (scan order), plus the effective user pick
 *  (`null` = Auto — also when the stored pick went stale). */
data class LyricParserGroup(
    val ext: String,
    val candidates: List<LyricParserContribution>,
    val selectedKey: String?,
)

/** Selection key of a parser contribution (stable across groups). */
fun LyricParserContribution.selectionKey(): String = "$pluginId:$parserId"

/** Group parsers by extension (scan order preserved within a group),
 *  sorted by extension. */
private fun buildGroups(
    parsers: List<LyricParserContribution>,
    selection: Map<String, String>,
): List<LyricParserGroup> {
    val byExt = LinkedHashMap<String, MutableList<LyricParserContribution>>()
    for (parser in parsers) {
        for (ext in parser.extensions) {
            byExt.getOrPut(ext) { mutableListOf() }.add(parser)
        }
    }
    return byExt.entries
        .sortedBy { it.key }
        .map { entry ->
            val ext = entry.key
            val candidates = entry.value
            val stored = selection[ext]
            // A stale pick (uninstalled/disabled plugin) renders as Auto.
            val valid = stored != null && candidates.any { it.selectionKey() == stored }
            LyricParserGroup(
                ext = ext,
                candidates = candidates,
                selectedKey = if (valid) stored else null,
            )
        }
}

/**
 * Lyric Parser settings page: per-extension groups of the plugin-provided
 * parsers (there is no built-in parser), with an Auto (default order)
 * default and a user pick persisted Rust-side (`plugin-state.json`).
 */
@HiltViewModel
class LyricParserVM @Inject constructor(
    private val pluginManager: PluginManager,
    pluginRepository: PluginRepository,
    private val toastRepository: ToastRepository,
) : ViewModel() {
    init {
        // Refresh in case the page is opened before the startup scan ran.
        viewModelScope.launch { pluginRepository.scanPlugins() }
    }

    val groups = combine(
        pluginRepository.lyricParsers,
        pluginRepository.lyricParserSelection,
    ) { parsers, selection ->
        buildGroups(parsers, selection)
    }.stateIn(viewModelScope, SharingStarted.Lazily, emptyList())

    val empty = groups.map { it.isEmpty() }
        .stateIn(viewModelScope, SharingStarted.Lazily, true)

    /** Pick a parser (or Auto with `key = null`) for one extension. */
    fun select(ext: String, key: String?) {
        viewModelScope.launch {
            pluginManager.setLyricParserSelection(ext, key)
                .onFailure {
                    toastRepository.emitToast(it.message ?: "failed to set lyric parser")
                }
        }
    }
}
