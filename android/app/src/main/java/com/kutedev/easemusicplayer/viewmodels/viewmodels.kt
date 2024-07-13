package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import com.kutedev.easemusicplayer.core.IOnNotifyView
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import uniffi.ease_client.ArgUpsertStorage
import uniffi.ease_client.CreatePlaylistMode
import uniffi.ease_client.RootRouteSubKey
import uniffi.ease_client.RootViewModelState
import uniffi.ease_client.StorageConnectionTestResult
import uniffi.ease_client.StorageType
import uniffi.ease_client.VCreatePlaylistState
import uniffi.ease_client.VEditStorageState
import uniffi.ease_client.VPlaylistListState
import uniffi.ease_client.VRootSubKeyState
import uniffi.ease_client.VStorageListState
import uniffi.ease_client.VTimeToPauseState

class PlaylistsViewModel : ViewModel(), IOnNotifyView {
    private val _state = MutableStateFlow(run {
        VPlaylistListState(emptyList())
    })
    val state = _state.asStateFlow()

    override fun onNotifyView(v: RootViewModelState): Unit {
        if (v.playlistList != null) {
            _state.value = v.playlistList!!.copy();
        }
    }
}

class RootSubkeyViewModel : ViewModel(), IOnNotifyView {
    private val _state = MutableStateFlow(run {
        VRootSubKeyState(
            subkey = RootRouteSubKey.PLAYLIST
        )
    })
    val state = _state.asStateFlow()

    override fun onNotifyView(v: RootViewModelState): Unit {
        if (v.currentRouter != null) {
            _state.value = v.currentRouter!!.copy();
        }
    }
}

class TimeToPauseViewModel : ViewModel(), IOnNotifyView {
    private val _state = MutableStateFlow(run {
        VTimeToPauseState (
            enabled = false,
            leftHour = 0u,
            leftMinute = 0u,
        )
    })
    val state = _state.asStateFlow()

    override fun onNotifyView(v: RootViewModelState): Unit {
        if (v.timeToPause != null) {
            _state.value = v.timeToPause!!.copy();
        }
    }
}

class StorageListViewModel : ViewModel(), IOnNotifyView {
    private val _state = MutableStateFlow(run {
        VStorageListState(
            items = emptyList(),
        )
    })
    val state = _state.asStateFlow()

    override fun onNotifyView(v: RootViewModelState): Unit {
        if (v.storageList != null) {
            _state.value = v.storageList!!.copy();
        }
    }
}

