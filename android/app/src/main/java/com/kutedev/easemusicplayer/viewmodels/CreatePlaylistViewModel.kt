package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import com.kutedev.easemusicplayer.core.Bridge
import com.kutedev.easemusicplayer.core.IOnNotifyView
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import uniffi.ease_client.PlaylistListWidget
import uniffi.ease_client.RootViewModelState
import uniffi.ease_client.VCreatePlaylistState
import uniffi.ease_client.VEditPlaylistState
import uniffi.ease_client.VEditStorageState
import uniffi.ease_client.Widget
import uniffi.ease_client_shared.CreatePlaylistMode


class CreatePlaylistViewModel : ViewModel(), IOnNotifyView {
    private val _state = MutableStateFlow(run {
        VCreatePlaylistState(
            mode = CreatePlaylistMode.FULL,
            name = "",
            picture = "",
            musicCount = 0u,
            recommendPlaylistNames = emptyList(),
            fullImported = false,
            modalOpen = false,
            canSubmit = false,
        )
    })

    val state = _state.asStateFlow()

    override fun onNotifyView(v: RootViewModelState): Unit {
        if (v.createPlaylist != null) {
            _state.value = v.createPlaylist!!.copy();
        }
    }
}


class EditPlaylistViewModel : ViewModel(), IOnNotifyView {
    private val _state = MutableStateFlow(run {
        VEditPlaylistState (
            name = "",
            picture = "",
            modalOpen = false,
        )
    })

    val state = _state.asStateFlow()

    override fun onNotifyView(v: RootViewModelState): Unit {
        if (v.editPlaylist != null) {
            _state.value = v.editPlaylist!!.copy();
        }
    }
}