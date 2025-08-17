package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kutedev.easemusicplayer.repositories.ImportRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import uniffi.ease_client_backend.CreatePlaylistMode
import uniffi.ease_client_backend.StorageEntry
import uniffi.ease_client_backend.StorageEntryType
import uniffi.ease_client_schema.DataSourceKey
import uniffi.ease_client_schema.StorageEntryLoc
import javax.inject.Inject
import kotlin.collections.firstOrNull
import kotlin.collections.map

private enum class Status {
    Closed,
    Edit,
    Create
}

@HiltViewModel
class CreatePlaylistVM @Inject constructor(
    private val importRepository: ImportRepository
) : ViewModel() {
    private val _modalOpen = MutableStateFlow(Status.Closed)
    private val _mode = MutableStateFlow(CreatePlaylistMode.FULL)
    private val _fullImported = MutableStateFlow(false)
    private val _entries = MutableStateFlow(listOf<StorageEntry>())
    private val _name = MutableStateFlow("")
    private val _cover = MutableStateFlow<DataSourceKey?>(null)
    val mode = _mode.asStateFlow()
    val musicCount = _entries.map { entries ->
        entries.count { entry ->  entry.entryTyp() == StorageEntryType.MUSIC }
    }.stateIn(viewModelScope, SharingStarted.Lazily, 0)
    val name = _name.asStateFlow()
    val recommendPlaylistNames = MutableStateFlow(listOf<String>())
    val cover = _cover.asStateFlow()
    val createModalOpen = _modalOpen.map { status -> status == Status.Create }.stateIn(
        scope = viewModelScope,
        started = SharingStarted.Lazily,
        initialValue = false
    )
    val editModalOpen = _modalOpen.map { status -> status == Status.Edit }.stateIn(
        scope = viewModelScope,
        started = SharingStarted.Lazily,
        initialValue = false
    )
    val fullImported = _fullImported.asStateFlow()

    val canSubmit = combine(name, mode, musicCount, cover) {
            name, mode, musicCount, cover ->
        if (mode == CreatePlaylistMode.FULL) {
             name.isNotBlank() && (musicCount > 0 || cover != null)
        } else {
            name.isNotBlank()
        }
    }.stateIn(
        scope = viewModelScope,
        started = SharingStarted.Lazily,
        initialValue = false
    )

    fun updateName(name: String) {
        _name.value = name
    }

    fun clearCover() {
        _cover.value = null
    }

    fun updateMode(mode: CreatePlaylistMode) {
        _mode.value = mode
    }

    fun openEditModal() {
        _modalOpen.value = Status.Edit
    }

    fun openCreateModal() {
        _modalOpen.value = Status.Create
    }

    fun closeModal() {
        _modalOpen.value = Status.Closed

        reset()
    }

    fun reset() {
        _mode.value = CreatePlaylistMode.FULL
        _fullImported.value = false
        _name.value = ""
        _cover.value = null
    }

    fun prepareImportCreate() {
        importRepository.prepare(listOf(StorageEntryType.MUSIC, StorageEntryType.IMAGE)) {
                entries ->
            _entries.value = entries.filter { v -> v.entryTyp() == StorageEntryType.IMAGE }
            _cover.value = entries.filter { v -> v.entryTyp() == StorageEntryType.IMAGE }.map { v -> DataSourceKey.AnyEntry(
                StorageEntryLoc(v.storageId, v.path)) }.firstOrNull()
        }
    }

    fun finish() {
    }
}
