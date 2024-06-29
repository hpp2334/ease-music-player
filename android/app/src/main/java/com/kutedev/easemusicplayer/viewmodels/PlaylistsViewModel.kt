package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import com.kutedev.easemusicplayer.core.Bridge
import com.kutedev.easemusicplayer.core.IOnNotifyView
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import uniffi.ease_client.RootViewModelState
import uniffi.ease_client.VPlaylistListState

fun buildDefaultVPlaylistListState(): VPlaylistListState {
    return VPlaylistListState(emptyList())
}

class PlaylistsViewModel : ViewModel(), IOnNotifyView {
    private val _state = MutableStateFlow(buildDefaultVPlaylistListState())
    val state = _state.asStateFlow()


    override fun onNotifyView(v: RootViewModelState): Unit {
        if (v.playlistList != null) {
            _state.value = v.playlistList!!.copy();
        }
    }
}