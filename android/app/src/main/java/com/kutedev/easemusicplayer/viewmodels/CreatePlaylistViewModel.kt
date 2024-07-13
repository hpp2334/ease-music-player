package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import com.kutedev.easemusicplayer.core.Bridge
import com.kutedev.easemusicplayer.core.IOnNotifyView
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import uniffi.ease_client.CreatePlaylistMode
import uniffi.ease_client.RootViewModelState
import uniffi.ease_client.VCreatePlaylistState
import uniffi.ease_client.prepareCreatePlaylist


class CreatePlaylistViewModel : ViewModel(), IOnNotifyView {
    private val _state = MutableStateFlow(run {
        VCreatePlaylistState(
            mode = CreatePlaylistMode.FULL,
            name = "",
            picture = null,
            musicCount = 0u,
            recommendPlaylistNames = emptyList(),
            preparedSignal = 0u,
            fullImported = false,
        )
    })
    private val _isOpen = MutableStateFlow(false)

    val state = _state.asStateFlow()
    val isOpen = _isOpen.asStateFlow()

    fun closeDialog() {
        _isOpen.value = false
    }

    fun openDialog() {
        Bridge.invoke {
            prepareCreatePlaylist()
        }
        _isOpen.value = true
    }

    override fun onNotifyView(v: RootViewModelState): Unit {
        if (v.createPlaylist != null) {
            _state.value = v.createPlaylist!!.copy();
        }
    }
}