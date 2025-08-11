package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import uniffi.ease_client_backend.CreatePlaylistMode
import uniffi.ease_client_backend.DataSourceKey
import javax.inject.Inject


@HiltViewModel
class EditPlaylistVM @Inject constructor() : ViewModel() {
    private val _mode = MutableStateFlow(CreatePlaylistMode.FULL)
    private val _fullImported = MutableStateFlow(false)
    private val _musicCount = MutableStateFlow(0)
    private val _name = MutableStateFlow("")
    private val _cover = MutableStateFlow<DataSourceKey?>(null)
    private val _modalOpen = MutableStateFlow(false)
    private val _canSubmit = MutableStateFlow(false)

    val mode = _mode.asStateFlow()
    val musicCount = _musicCount.asStateFlow()
    val name = _name.asStateFlow()
    val recommendPlaylistNames = MutableStateFlow(listOf<String>())
    val cover = _cover.asStateFlow()
    val modalOpen = _modalOpen.asStateFlow()
    val fullImported = _fullImported.asStateFlow()
    val canSubmit = _canSubmit.asStateFlow()

    fun updateName(name: String) {
        _name.update { _ -> name }
    }

    fun clearCover() {
        _cover.update { _ -> null }
    }

    fun openModal() {}

    fun closeModal() {}

    fun updateMode(mode: CreatePlaylistMode) {}

    fun reset() {}

    fun finish() {}
}
