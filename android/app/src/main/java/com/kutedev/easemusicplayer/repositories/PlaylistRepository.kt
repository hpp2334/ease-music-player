package com.kutedev.easemusicplayer.repositories

import androidx.lifecycle.viewModelScope
import com.kutedev.easemusicplayer.core.Bridge
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import uniffi.ease_client_backend.ArgCreatePlaylist
import uniffi.ease_client_backend.ArgUpdatePlaylist
import uniffi.ease_client_backend.PlaylistAbstract
import uniffi.ease_client_backend.ctCreatePlaylist
import uniffi.ease_client_backend.ctListPlaylist
import uniffi.ease_client_backend.ctUpdatePlaylist
import javax.inject.Inject
import javax.inject.Singleton


@Singleton
class PlaylistRepository @Inject constructor(
    private val bridge: Bridge,
    private val _scope: CoroutineScope
) {
    private val _playlists = MutableStateFlow(listOf<PlaylistAbstract>())

    val playlists = _playlists.asStateFlow()

    fun createPlaylist(arg: ArgCreatePlaylist) {
        _scope.launch {
            ctCreatePlaylist(bridge.backend, arg)
        }
    }

    fun editPlaylist(arg: ArgUpdatePlaylist) {
        _scope.launch {
            ctUpdatePlaylist(bridge.backend, arg)
        }
    }

    suspend fun reload() {
        _playlists.value = ctListPlaylist(bridge.backend)
    }
}