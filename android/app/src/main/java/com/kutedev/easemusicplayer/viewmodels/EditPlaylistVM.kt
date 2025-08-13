package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import uniffi.ease_client_backend.CreatePlaylistMode
import uniffi.ease_client_schema.DataSourceKey
import javax.inject.Inject

private enum class Status {
    Closed,
    Edit,
    Create
}

@HiltViewModel
class EditPlaylistVM @Inject constructor() : ViewModel() {
    private val _mode = MutableStateFlow(CreatePlaylistMode.FULL)
    private val _fullImported = MutableStateFlow(false)
    private val _musicCount = MutableStateFlow(0)
    private val _name = MutableStateFlow("")
    private val _cover = MutableStateFlow<DataSourceKey?>(null)
    private val _modalOpen = MutableStateFlow(Status.Closed)
    private val _canSubmit = MutableStateFlow(false)

    val mode = _mode.asStateFlow()
    val musicCount = _musicCount.asStateFlow()
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
    val canSubmit = _canSubmit.asStateFlow()

    fun updateName(name: String) {
        _name.value = name
    }

    fun clearCover() {
        _cover.value = null
    }

    fun openEditModal() {
        _modalOpen.value = Status.Edit
    }

    fun openCreateModal() {
        _modalOpen.value = Status.Create
    }

    fun closeModal() {
        _modalOpen.value = Status.Closed

        _mode.value = CreatePlaylistMode.FULL
        _fullImported.value = false
        _musicCount.value = 0
        _name.value = ""
        _cover.value = null
    }

    fun updateMode(mode: CreatePlaylistMode) {
        _mode.value = mode
    }

    fun reset() {}

    fun finish() {}
}
