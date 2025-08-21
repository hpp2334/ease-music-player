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
import uniffi.ease_client_backend.ctRemovePlaylist
import uniffi.ease_client_backend.ctUpdatePlaylist
import uniffi.ease_client_schema.PlaylistId
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
            reload()
        }
    }

    fun editPlaylist(arg: ArgUpdatePlaylist) {
        _scope.launch {
            ctUpdatePlaylist(bridge.backend, arg)
            reload()
        }
    }

    fun removePlaylist(id: PlaylistId) {
        _scope.launch {
            ctRemovePlaylist(bridge.backend, id)
            reload()
        }
    }

    suspend fun reload() {
        _playlists.value = ctListPlaylist(bridge.backend)
    }
}