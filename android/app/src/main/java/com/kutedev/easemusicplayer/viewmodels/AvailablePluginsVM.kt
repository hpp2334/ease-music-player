package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.singleton.PluginManager
import com.kutedev.easemusicplayer.singleton.PluginRegistryRepository
import com.kutedev.easemusicplayer.singleton.PluginRepository
import com.kutedev.easemusicplayer.singleton.types.PluginSource
import com.kutedev.easemusicplayer.singleton.types.RegistryPluginEntry
import com.kutedev.easemusicplayer.singleton.ToastRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

/** One row of the Available page's plugin list. `installedVersion` +
 *  `updateAvailable` arrive pre-stamped from Rust (`plugin.registryFetch`)
 *  — Kotlin never compares versions. */
data class AvailablePluginRow(
    val entry: RegistryPluginEntry,
)

/** List load state for the selected source. */
sealed class AvailableListState {
    data object Loading : AvailableListState()
    data class Ready(val rows: List<AvailablePluginRow>, val fromCache: Boolean) : AvailableListState()
    data class Failed(val error: String, val hasCache: Boolean) : AvailableListState()
}

/** Backs the Available (获取插件) page: source selection (presets + saved
 *  customs), custom-source verification, registry list + install/update. */
@HiltViewModel
class AvailablePluginsVM @Inject constructor(
    private val pluginManager: PluginManager,
    private val pluginRepository: PluginRepository,
    private val registryRepository: PluginRegistryRepository,
    private val toastRepository: ToastRepository,
) : ViewModel() {
    /** All selectable sources (presets + saved customs), refreshable. */
    private val _sources = MutableStateFlow<List<PluginSource>>(emptyList())
    val sources = _sources.asStateFlow()

    private val _selectedSourceUrl = MutableStateFlow("")
    val selectedSourceUrl = _selectedSourceUrl.asStateFlow()

    private val _listState = MutableStateFlow<AvailableListState>(AvailableListState.Loading)
    val listState = _listState.asStateFlow()

    /** Entry ids with a download in flight (drives row spinners). */
    private val _busyIds = MutableStateFlow<Set<String>>(emptySet())
    val busyIds = _busyIds.asStateFlow()

    init {
        viewModelScope.launch { pluginRepository.scanPlugins() }
        viewModelScope.launch {
            val sources = registryRepository.sources()
            _sources.value = sources.all
            // Rust drops a stale lastSourceUrl (matches no preset/custom);
            // fall back to the first preset.
            _selectedSourceUrl.value = sources.lastSourceUrl
                ?: sources.presets.firstOrNull()?.url.orEmpty()
            load(tryCacheFirst = false)
        }
    }

    fun refreshSources() {
        viewModelScope.launch { _sources.value = registryRepository.sources().all }
    }

    fun selectSource(url: String) {
        if (_selectedSourceUrl.value == url) return
        _selectedSourceUrl.value = url
        viewModelScope.launch { registryRepository.rememberSource(url) }
        load(tryCacheFirst = false)
    }

    fun retry() = load(tryCacheFirst = false)

    /** (Re)load the registry for the selected source. Network first; on
     *  failure fall back to the per-source cache. */
    fun load(tryCacheFirst: Boolean) {
        val url = _selectedSourceUrl.value
        if (url.isEmpty()) return
        _listState.value = AvailableListState.Loading
        viewModelScope.launch {
            val cached = registryRepository.cachedRegistry(url)
            if (tryCacheFirst && cached != null) {
                _listState.value = AvailableListState.Ready(buildRows(cached), fromCache = true)
                return@launch
            }
            registryRepository.fetchRegistry(url)
                .onSuccess { entries -> _listState.value = AvailableListState.Ready(buildRows(entries), fromCache = false) }
                .onFailure { e ->
                    if (cached != null) {
                        _listState.value = AvailableListState.Ready(buildRows(cached), fromCache = true)
                    } else {
                        _listState.value = AvailableListState.Failed(e.message ?: "error", hasCache = false)
                    }
                }
        }
    }

    /** Verify + persist a custom source; toasts the outcome. On success
     *  the source is selected and the dialog should close. */
    fun addCustomSource(url: String, onResult: (Boolean) -> Unit) {
        viewModelScope.launch {
            registryRepository.addCustomSource(url).fold(
                onSuccess = { entries ->
                    refreshSources()
                    selectSource(url.trim().trimEnd('/'))
                    toastRepository.emitToast(
                        "OK: ${entries.size} plugins",
                    )
                    onResult(true)
                },
                onFailure = {
                    toastRepository.emitToastRes(R.string.plugin_source_verify_fail)
                    onResult(false)
                },
            )
        }
    }

    fun removeCustomSource(url: String) {
        viewModelScope.launch {
            registryRepository.removeCustomSource(url)
            refreshSources()
            if (_selectedSourceUrl.value == url) {
                selectSource(registryRepository.defaultSourceUrl())
            }
        }
    }

    /** Download + install (or upgrade) one entry; toasts the outcome. */
    fun install(entry: RegistryPluginEntry) {
        viewModelScope.launch {
            _busyIds.value = _busyIds.value + entry.id
            try {
                val baseUrl = _selectedSourceUrl.value
                pluginManager.downloadAndInstall(entry, baseUrl).fold(
                    onSuccess = { toastRepository.emitToastRes(R.string.plugin_install_ok) },
                    onFailure = { e ->
                        toastRepository.emitToast(e.message ?: "install failed")
                    },
                )
                // Refresh rows (installed version changed).
                load(tryCacheFirst = true)
            } finally {
                _busyIds.value = _busyIds.value - entry.id
            }
        }
    }

    private fun buildRows(entries: List<RegistryPluginEntry>): List<AvailablePluginRow> =
        entries.map { AvailablePluginRow(entry = it) }
}
