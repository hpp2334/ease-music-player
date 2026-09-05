package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kutedev.easemusicplayer.singleton.Bridge
import com.kutedev.easemusicplayer.singleton.PluginRepository
import com.kutedev.easemusicplayer.singleton.StorageProvider
import com.kutedev.easemusicplayer.singleton.StorageRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import com.kutedev.easemusicplayer.singleton.types.StorageHandle
import com.kutedev.easemusicplayer.singleton.types.StorageId
import javax.inject.Inject

/** Resolved view descriptor for an edited plugin storage (edit mode only). */
data class EditPluginView(
    /** Module-source handle of the view JS (registered Rust-side by the
     *  `plugin.list` scan); `0` until the scan resolves. */
    val viewSourceHandle: Long,
    val pluginId: String,
    val pluginStorageId: String,
)

private data class EditPluginHandle(
    val pluginId: String,
    val pluginStorageId: String,
)


@HiltViewModel
class EditStorageVM @Inject constructor(
    private val bridge: Bridge,
    private val storageRepository: StorageRepository,
    private val pluginRepository: PluginRepository,
    savedStateHandle: SavedStateHandle
) : ViewModel() {

    private val _title = MutableStateFlow("")
    private val _musicCount = MutableStateFlow(0uL)
    // True when editing an existing JS-plugin storage. In create mode the
    // provider is chosen via the UI chooser, so this only reflects the loaded
    // storage's kind.
    private val _pluginMode = MutableStateFlow(false)
    // For an edited plugin storage: its (pluginId, pluginStorageId). Drives
    // the derived `editPluginView` (resolved against `storageProviders`).
    private val _editPluginHandle = MutableStateFlow<EditPluginHandle?>(null)
    private val _removedEvent = MutableSharedFlow<Unit>()

    val musicCount = _musicCount.asStateFlow()
    val title = _title.asStateFlow()
    val pluginMode = _pluginMode.asStateFlow()
    /** Discovered plugin storage providers (drives the create-mode chooser). */
    val storageProviders = pluginRepository.storageProviders
    /**
     * For an edited plugin storage: the resolved view JS file + ids (null
     * in create mode, or until `scanPlugins` resolves the provider).
     * `EditStoragesPage` renders this into a `TurView` stamped with the
     * storage id so `ease.context.storageId$` is non-null (edit mode).
     */
    val editPluginView: kotlinx.coroutines.flow.StateFlow<EditPluginView?> =
        combine(_editPluginHandle, pluginRepository.storageProviders) { handle, providers ->
            if (handle == null) {
                null
            } else {
                providers
                    .firstOrNull { it.pluginId == handle.pluginId }
                    ?.takeIf { it.viewSourceHandle != 0L }
                    ?.let {
                        EditPluginView(it.viewSourceHandle, handle.pluginId, handle.pluginStorageId)
                    }
            }
        }.stateIn(viewModelScope, SharingStarted.Eagerly, null)
    /** Fires when the edited storage disappears from the list (the plugin
     *  backend called `ease.context.removeStorage`, or the trash removed it).
     *  `EditStoragesPage` collects this to pop back. */
    val removedEvent = _removedEvent.asSharedFlow()
    /** Fires when a JS plugin registers a new storage instance — either an
     *  OAuth exchange (handled by `MainActivity` via the
     *  `easem://oauth2redirect` callback) or a non-OAuth backend
     *  (`ease.context.createStorage`, e.g. WebDAV's `webdav:connect`).
     *  `EditStoragesPage` collects this to pop back from the setup form. */
    val pluginConnectedEvent = storageRepository.pluginConnectedEvent

    /** Edited storage id (null in create mode). */
    private val _editId = MutableStateFlow<StorageId?>(null)
    val isCreated: kotlinx.coroutines.flow.StateFlow<Boolean> =
        _editId.map { it == null }.stateIn(viewModelScope, SharingStarted.Lazily, true)

    private val _removeModalOpen = MutableStateFlow(false)
    val removeModalOpen = _removeModalOpen.asStateFlow()

    fun openRemoveModal() {
        _removeModalOpen.value = true
    }

    fun closeRemoveModal() {
        _removeModalOpen.value = false
    }

    init {
        _title.value = ""
        _musicCount.value = 0u

        val id: Long? = savedStateHandle["id"]
        // The route determines mode: `RouteCreateStorage` passes no id
        // (id == null -> create new), `RouteEditStorage/{id}` passes a real
        // storage id (edit existing). No sentinel, no ambiguity.
        val storage = if (id == null) {
            null
        } else {
            storageRepository.storages.value.find { v -> v.id == StorageId(id) }
        }
        if (storage != null) {
            _editId.value = storage.id
            val isPlugin = storage.handle is StorageHandle.Plugin
            _pluginMode.value = isPlugin
            if (isPlugin) {
                val handle = storage.handle as StorageHandle.Plugin
                _editPluginHandle.value = EditPluginHandle(
                    pluginId = handle.pluginId.id,
                    pluginStorageId = handle.pluginStorageId.id,
                )
            }
            _title.value = storage.alias
            _musicCount.value = storage.musicCount

            // Pop the edit page when the storage is removed out from under
            // us (view-side disconnect via the plugin backend, or the
            // top-bar trash). `wasPresent` guards the initial load; `done`
            // stops repeats after the first removal (collect is
            // crossinline so a non-local return isn't allowed).
            viewModelScope.launch {
                var wasPresent = false
                var done = false
                storageRepository.storages.collect { list ->
                    if (!done) {
                        val editId = _editId.value
                        if (editId != null) {
                            val present = list.any { it.id == editId }
                            if (wasPresent && !present) {
                                _removedEvent.tryEmit(Unit)
                                done = true
                            }
                            wasPresent = present
                        }
                    }
                }
            }
        }

        // Discover plugin storage providers for the create-mode chooser.
        viewModelScope.launch {
            pluginRepository.scanPlugins()
        }
    }

    fun remove() {
        val id = _editId.value

        if (id != null) {
            viewModelScope.launch {
                storageRepository.pluginRemoveInstance(id)
            }
        }
    }
}
