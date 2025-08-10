package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import dagger.hilt.android.lifecycle.HiltViewModel
import javax.inject.Inject
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import uniffi.ease_client_backend.CurrentStorageStateType
import uniffi.ease_client_backend.Storage
import uniffi.ease_client_backend.StorageEntry
import uniffi.ease_client_backend.StorageEntryType
import uniffi.ease_client_backend.StorageId
import uniffi.ease_client_backend.StorageType

data class SplitPathItem(
        val path: String,
        val name: String,
)

@HiltViewModel
class ImportVM @Inject constructor() : ViewModel() {
    private val _splitPaths = MutableStateFlow(mutableListOf<SplitPathItem>())
    private val _selectedCount = MutableStateFlow(0)
    private val _entries = MutableStateFlow(listOf<StorageEntry>())
    private val _allowTypes = MutableStateFlow(listOf<StorageEntryType>())
    private val _selectedStorageId = MutableStateFlow(StorageId(0))
    private val _loadState = MutableStateFlow(CurrentStorageStateType.LOADING)
    private val _disabledToggleAll = MutableStateFlow(false)
    private val _canUndo = MutableStateFlow(false)

    val splitPaths = _splitPaths.asStateFlow()
    val selectedCount = _selectedCount.asStateFlow()
    val entries = _entries.asStateFlow()
    val allowTypes = _allowTypes.asStateFlow()
    val selectedStorageId = _selectedStorageId.asStateFlow()
    val loadState = _loadState.asStateFlow()
    val disabledToggleAll = _disabledToggleAll.asStateFlow()
    val canUndo = _canUndo.asStateFlow()

    fun navigateDir(path: String) {}

    fun toggleSelect(path: String) {}

    fun finish() {}

    fun selectStorage(storageId: StorageId) {}

    fun toggleAll() {}

    fun reload() {}

    fun undo() {}

    fun redo() {}
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
