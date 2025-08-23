package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import com.kutedev.easemusicplayer.singleton.PlaylistRepository
import dagger.hilt.android.lifecycle.HiltViewModel
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