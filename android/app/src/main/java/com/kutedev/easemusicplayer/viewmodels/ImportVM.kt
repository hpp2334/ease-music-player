package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kutedev.easemusicplayer.singleton.Bridge
import com.kutedev.easemusicplayer.singleton.BridgeMethods
import com.kutedev.easemusicplayer.singleton.ImportRepository
import com.kutedev.easemusicplayer.singleton.PermissionRepository
import com.kutedev.easemusicplayer.singleton.StorageRepository
import kotlinx.collections.immutable.persistentHashSetOf
import kotlinx.collections.immutable.persistentListOf
import javax.inject.Inject
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import com.kutedev.easemusicplayer.singleton.types.CurrentStorageStateType
import com.kutedev.easemusicplayer.singleton.types.ListStorageEntryChildrenResp
import com.kutedev.easemusicplayer.singleton.types.Storage
import com.kutedev.easemusicplayer.singleton.types.StorageEntry
import com.kutedev.easemusicplayer.singleton.types.StorageEntryLoc
import com.kutedev.easemusicplayer.singleton.types.StorageEntryType
import com.kutedev.easemusicplayer.singleton.types.StorageHandle
import com.kutedev.easemusicplayer.singleton.types.StorageId
import dagger.hilt.android.lifecycle.HiltViewModel
import java.net.URLDecoder

data class SplitPathItem(
    val path: String,
    val name: String,
)

private fun defaultSplitPaths(): List<SplitPathItem> {
    return listOf()
}

@HiltViewModel
class ImportVM @Inject constructor(
    private val storageRepository: StorageRepository,
    private val importRepository: ImportRepository,
    private val permissionRepository: PermissionRepository,
    private val bridge: Bridge
) : ViewModel() {
    private val _currentPath = MutableStateFlow("/")
    private val _splitPaths = _currentPath.map { path ->
        val components = path.split('/').filter { it.isNotEmpty() }
        val splitPaths = mutableListOf<SplitPathItem>()

        var currentPath = ""
        for (component in components) {
            currentPath = if (currentPath == "/") {
                "/$component"
            } else {
                "$currentPath/$component"
            }
            val name = try {
                URLDecoder.decode(component, "UTF-8")
            } catch (e: Exception) {
                component
            }
            splitPaths.add(SplitPathItem(currentPath, name))
        }

        splitPaths
    }.stateIn(viewModelScope, SharingStarted.Lazily, defaultSplitPaths())
    private val _selected = MutableStateFlow(persistentHashSetOf<String>())
    private val _entries = MutableStateFlow(listOf<StorageEntry>())
    private val _selectedStorageId = MutableStateFlow(filterStorages(
        storageRepository.storages.value,
        importRepository.allowedStorageIds.value
    ).firstOrNull()?.id)
    private val _loadState = MutableStateFlow(CurrentStorageStateType.LOADING)
    // Toggle-all is disabled when there is nothing *selectable* — entries
    // whose type the current import accepts (dirs and mismatched files,
    // e.g. a .wma during a music import, are never selectable).
    private val _disabledToggleAll =
        combine(_entries, importRepository.allowTypes) { entries, types ->
            selectableEntries(entries, types).isEmpty()
        }.stateIn(viewModelScope, SharingStarted.Lazily, true)
    private val _undoStack = MutableStateFlow(persistentListOf<String>())

    /**
     * Storages the prepared import may actually use — the configured
     * storage list filtered by the source restriction
     * ([ImportRepository.allowedStorageIds]; `null` = unrestricted).
     * Drives default/fallback selection; disallowed storages stay
     * visible in the picker but render disabled.
     */
    private fun filterStorages(
        storages: List<Storage>,
        allowed: List<StorageId>?
    ): List<Storage> {
        if (allowed == null) {
            return storages
        }
        return storages.filter { storage -> allowed.contains(storage.id) }
    }

    private val selectableStorages = combine(
        storageRepository.storages,
        importRepository.allowedStorageIds
    ) {
            storages, allowed ->
        filterStorages(storages, allowed)
    }.stateIn(
        scope = viewModelScope,
        started = SharingStarted.Lazily,
        initialValue = filterStorages(
            storageRepository.storages.value,
            importRepository.allowedStorageIds.value
        )
    )

    /** All configured storages — the picker renders these, dimming the disallowed ones. */
    val storages = storageRepository.storages

    /** Storages excluded by the prepared import's source restriction (empty = unrestricted). */
    val disabledStorageIds = combine(
        storageRepository.storages,
        importRepository.allowedStorageIds
    ) {
            storages, allowed ->
        if (allowed == null) {
            emptySet()
        } else {
            storages
                .filter { storage -> !allowed.contains(storage.id) }
                .map { storage -> storage.id }
                .toSet()
        }
    }.stateIn(
        scope = viewModelScope,
        started = SharingStarted.Lazily,
        initialValue = emptySet()
    )

    val splitPaths = _splitPaths
    val selectedCount = _selected.combine(_entries) { selected, entries ->
        entries.count { entry -> selected.contains(entry.path) }
    }.stateIn(viewModelScope, SharingStarted.Lazily, 0)
    val entries = _entries.asStateFlow()
    val selected = _selected.asStateFlow()
    val allowTypes = importRepository.allowTypes
    val selectedStorageId = _selectedStorageId.asStateFlow()
    val loadState = _loadState.asStateFlow()
    val disabledToggleAll = _disabledToggleAll
    val canUndo =
        _undoStack.map {
            undoStack -> undoStack.isNotEmpty()
        }.stateIn(viewModelScope, SharingStarted.Lazily, false)


    init {
        viewModelScope.launch {
            selectableStorages.collect { storages ->
                val storage = storages.find { storage -> storage.id == _selectedStorageId.value }
                if (storage == null) {
                    _selectedStorageId.value = storages.firstOrNull()?.id
                }

                reload()
            }
        }
        viewModelScope.launch {
            reload()
        }
        viewModelScope.launch {
            permissionRepository.havePermission.collect {
                reload()
            }
        }
    }

    fun clickEntry(entry: StorageEntry) {
        if (entry.isDir) {
            navigateDir(entry.path)
        } else if (allowTypes.value.contains(entry.entryTyp())) {
            toggleSelect(entry.path)
        }
    }

    fun navigateDir(path: String) {
        pushCurrentToUndoStack()
        navigateDirImpl(path)
    }

    private fun toggleSelect(path: String) {
        val selected = _selected.value
        val next = {
            if (selected.contains(path)) {
                selected.remove(path)
            } else {
                selected.add(path)
            }
        }()
        _selected.value = next
    }

    fun finish() {
        val v = _entries.value.filter { entry -> _selected.value.contains(entry.path) }
        importRepository.onFinish(v)
    }

    fun requestPermission() {
        permissionRepository.requestStoragePermission()
    }

    fun selectStorage(storageId: StorageId) {
        if (disabledStorageIds.value.contains(storageId)) {
            return
        }
        _selectedStorageId.value = storageId
        _undoStack.value = persistentListOf()

        navigateDirImpl("/")
    }

    fun toggleAll() {
        // Only selectable entries participate: dirs and entries whose type
        // the current import does not accept (e.g. a .wma during a music
        // import) have no checkbox and must neither be selected nor counted.
        val selectable = selectableEntries(_entries.value, allowTypes.value).map { it.path }
        if (selectable.isEmpty()) {
            return
        }
        val allSelected = _selected.value.containsAll(selectable)
        _selected.update { selected ->
            if (allSelected) {
                selected.clear()
            } else {
                selected.clear().addAll(selectable)
            }
        }
    }

    private fun selectableEntries(
        entries: List<StorageEntry>,
        types: List<StorageEntryType>
    ): List<StorageEntry> {
        return entries.filter { entry -> !entry.isDir && types.contains(entry.entryTyp()) }
    }

    fun reload() {
        val storage = currentStorage() ?: return

        // The local storage reads by raw path (list + get) — without
        // all-files access Android denies both for files contributed by
        // other apps, so gate on the permission first (the NEED_PERMISSION
        // state renders the grant prompt). Cloud storages browse as usual.
        if (storage.handle is StorageHandle.Local &&
            !permissionRepository.havePermission.value
        ) {
            _loadState.value = CurrentStorageStateType.NEED_PERMISSION
            return
        }

        _loadState.value = CurrentStorageStateType.LOADING
        _entries.value = emptyList()

        viewModelScope.launch {
            val loc = StorageEntryLoc(
                storageId = storage.id,
                path = currentPath(),
            )
            val resp: ListStorageEntryChildrenResp? = try {
                bridge.call(BridgeMethods.Storage.LIST_ENTRY_CHILDREN, loc).unwrapOrThrow().payload
            } catch (e: Throwable) {
                null
            }

            when (resp) {
                null -> {
                    _loadState.value = CurrentStorageStateType.UNKNOWN_ERROR
                }
                is ListStorageEntryChildrenResp.Ok -> {
                    _loadState.value = CurrentStorageStateType.OK
                    _entries.value = resp.data
                }

                ListStorageEntryChildrenResp.AuthenticationFailed -> {
                    _loadState.value = CurrentStorageStateType.AUTHENTICATION_FAILED
                }

                ListStorageEntryChildrenResp.Timeout -> {
                    _loadState.value = CurrentStorageStateType.TIMEOUT
                }

                ListStorageEntryChildrenResp.Unknown -> {
                    _loadState.value = CurrentStorageStateType.UNKNOWN_ERROR
                }
            }
        }
    }

    fun undo() {
        val current = popCurrentFromUndoStack()
        if (current != null) {
            navigateDirImpl(current)
        }
    }

    private fun currentPath(): String {
        val p = _splitPaths.value.lastOrNull()?.path

        if (p == null) {
            return "/"
        }
        return p
    }

    private fun currentStorage(): Storage? {
        val storage = storages.value.find { storage -> storage.id == _selectedStorageId.value }
        return storage
    }

    private fun pushCurrentToUndoStack() {
        val currentUndoStack = _undoStack.value
        val nextUndoStack = currentUndoStack.add(currentPath())
        _undoStack.value = nextUndoStack
    }

    private fun popCurrentFromUndoStack(): String? {
        val currentUndoStack = _undoStack.value
        val current = currentUndoStack.lastOrNull()
        if (current != null) {
            val next = currentUndoStack.removeAt(currentUndoStack.lastIndex)
            _undoStack.value = next
        }
        return current
    }


    private fun navigateDirImpl(path: String) {
        _currentPath.value = path
        _selected.update { selected ->
            selected.clear()
        }

        reload()
    }
}

class VImportStorageEntry(private val storage: Storage) {
    val id: StorageId
        get() = storage.id

    val isLocal: Boolean
        get() = storage.handle is StorageHandle.Local

    val name: String
        get() = storage.alias

    /** Provider name for plugin storages (e.g. "webdav" / "onedrive"). */
    val subtitle: String
        get() = when (val handle = storage.handle) {
            is StorageHandle.Plugin -> handle.pluginStorageId.id.substringBefore(':')
            else -> ""
        }
}
