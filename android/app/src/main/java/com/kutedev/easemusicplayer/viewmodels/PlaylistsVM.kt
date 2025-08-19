package com.kutedev.easemusicplayer.viewmodels

import androidx.compose.runtime.collectAsState
import androidx.lifecycle.ViewModel
import com.kutedev.easemusicplayer.repositories.PlaylistRepository
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import uniffi.ease_client_backend.Playlist
import uniffi.ease_client_backend.PlaylistAbstract
import javax.inject.Inject

data class PlaylistsState(
    val playlists: List<PlaylistAbstract> = listOf()
)

@HiltViewModel
class PlaylistsVM @Inject constructor(
    private val playlistRepository: PlaylistRepository
) : ViewModel() {
    val playlists = playlistRepository.playlists
}