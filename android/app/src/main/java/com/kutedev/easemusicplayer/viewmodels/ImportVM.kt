package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kutedev.easemusicplayer.singleton.Bridge
import com.kutedev.easemusicplayer.singleton.ImportRepository
import com.kutedev.easemusicplayer.singleton.StorageRepository
import dagger.hilt.android.lifecycle.HiltViewModel
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
import uniffi.ease_client_backend.CurrentStorageStateType
import uniffi.ease_client_backend.ListStorageEntryChildrenResp
import uniffi.ease_client_backend.Storage
import uniffi.ease_client_backend.StorageEntry
import uniffi.ease_client_backend.ctListStorageEntryChildren
import uniffi.ease_client_schema.StorageEntryLoc
import uniffi.ease_client_schema.StorageId
import uniffi.ease_client_schema.StorageType

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
            splitPaths.add(SplitPathItem(currentPath, component))
        }

        splitPaths
    }.stateIn(viewModelScope, SharingStarted.Lazily, defaultSplitPaths())
    private val _selected = MutableStateFlow(persistentHashSetOf<String>())
    private val _entries = MutableStateFlow(listOf<StorageEntry>())
    private val _selectedStorageId = MutableStateFlow(storageRepository.storages.value.firstOrNull()?.id)
    private val _loadState = MutableStateFlow(CurrentStorageStateType.LOADING)
    private val _disabledToggleAll = _entries.map { entries ->
        entries.all { it.isDir }
    }.stateIn(viewModelScope, SharingStarted.Lazily, true)
    private val _undoStack = MutableStateFlow(persistentListOf<String>())

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
            storageRepository.storages.collect { storages ->
                val storage = storages.find { storage -> storage.id == _selectedStorageId.value }
                if (storage == null) {
                    _selectedStorageId.value = storageRepository.storages.value.firstOrNull()?.id
                }

                reload()
            }

            reload()
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

    fun selectStorage(storageId: StorageId) {
        _selectedStorageId.value = storageId
    }

    fun toggleAll() {
        val allSelected = _selected.value.size == _entries.value.size
        if (allSelected) {
            _selected.update { selected ->
                selected.clear()
            }
        } else {
            _selected.update { selected ->
                selected.clear().addAll(_entries.value.map { it.path })
            }
        }
    }

    fun reload() {
        val storageId = _selectedStorageId.value
        if (storageId == null) {
            return
        }

        _loadState.value = CurrentStorageStateType.LOADING
        _entries.value = emptyList()

        viewModelScope.launch {
            val resp = bridge.runRaw {
                ctListStorageEntryChildren(
                    it, StorageEntryLoc(
                        storageId = storageId,
                        path = currentPath()
                    )
                )
            }

            when (resp) {
                is ListStorageEntryChildrenResp.Ok -> {
                    _loadState.value = CurrentStorageStateType.OK
                    _entries.value = resp.v1
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
        val storage = storageRepository.storages.value.find { storage -> storage.id == _selectedStorageId.value }
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
        get() = storage.typ == StorageType.LOCAL

    val name: String
        get() {
            if (storage.alias != "") {
                return storage.alias
            }
            return storage.addr
        }

    val subtitle: String
        get() {
            if (storage.alias != "") {
                return storage.addr
            }
            return ""
        }
}
