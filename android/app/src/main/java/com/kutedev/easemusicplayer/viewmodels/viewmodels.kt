package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import com.kutedev.easemusicplayer.core.IOnNotifyView
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import uniffi.ease_client.RootRouteSubKey
import uniffi.ease_client.RootViewModelState
import uniffi.ease_client.VCreatePlaylistState
import uniffi.ease_client.VCurrentMusicState
import uniffi.ease_client.VCurrentPlaylistState
import uniffi.ease_client.VCurrentStorageEntriesState
import uniffi.ease_client.VEditStorageState
import uniffi.ease_client.VPlaylistListState
import uniffi.ease_client.VRootSubKeyState
import uniffi.ease_client.VStorageListState
import uniffi.ease_client.VTimeToPauseState
import uniffi.ease_client_shared.CurrentStorageImportType
import uniffi.ease_client_shared.CurrentStorageStateType
import uniffi.ease_client_shared.PlayMode

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
            modalOpen = false,
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


class CurrentPlaylistViewModel : ViewModel(), IOnNotifyView {
    private val _state = MutableStateFlow(run {
        VCurrentPlaylistState(
            id = null,
            items = emptyList(),
            title = "",
            duration = "",
            coverUrl = "",
        )
    })
    val state = _state.asStateFlow()

    override fun onNotifyView(v: RootViewModelState): Unit {
        if (v.currentPlaylist != null) {
            _state.value = v.currentPlaylist!!.copy();
        }
    }
}



class CurrentMusicViewModel : ViewModel(), IOnNotifyView {
    private val _state = MutableStateFlow(run {
        VCurrentMusicState(
            id = null,
            title = "",
            currentDuration = "",
            totalDuration = "",
            currentDurationMs = 0UL,
            totalDurationMs = 0UL,
            canChangePosition = false,
            canPlayNext = false,
            canPlayPrevious = false,
            previousCover = "",
            nextCover = "",
            cover = "",
            playMode = PlayMode.SINGLE,
            playing = false,
            lyricIndex = 0,
            loading = false,
        )
    })
    val state = _state.asStateFlow()

    override fun onNotifyView(v: RootViewModelState): Unit {
        if (v.currentMusic != null) {
            _state.value = v.currentMusic!!.copy();
        }
    }
}


class CurrentStorageEntriesViewModel : ViewModel(), IOnNotifyView {
    private val _state = MutableStateFlow(run {
        VCurrentStorageEntriesState(
            importType = CurrentStorageImportType.None,
            stateType = CurrentStorageStateType.LOADING,
            currentStorageId = null,
            storageItems = emptyList(),
            entries = emptyList(),
            selectedCount = 0,
            splitPaths = emptyList(),
            currentPath = "",
            disabledToggleAll = false,
            canUndo = false,
        )
    })
    val state = _state.asStateFlow()

    override fun onNotifyView(v: RootViewModelState): Unit {
        if (v.currentStorageEntries != null) {
            _state.value = v.currentStorageEntries!!.copy();
        }
    }
}