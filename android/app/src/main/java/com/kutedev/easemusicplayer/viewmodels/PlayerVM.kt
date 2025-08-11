package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import uniffi.ease_client_backend.DataSourceKey
import uniffi.ease_client_backend.LrcMetadata
import uniffi.ease_client_backend.Lyrics
import uniffi.ease_client_backend.MusicId
import uniffi.ease_client_backend.PlayMode
import uniffi.ease_client_backend.PlaylistId
import javax.inject.Inject

data class MusicState(
    val id: MusicId? = null,
    val playing: Boolean = false,
    val title: String = "",
    val cover: DataSourceKey? = null,
    val previousCover: DataSourceKey? = null,
    val nextCover: DataSourceKey? = null,
    val currentDurationMs: ULong = 0uL,
    val currentDuration: String = "00:00",
    val totalDuration: String = "00:00",
    val totalDurationMs: ULong = 0uL,
    val bufferDurationMs: ULong = 0uL,
    val canPlayNext: Boolean = false,
    val canPlayPrevious: Boolean = false,
    val loading: Boolean = false
)

@HiltViewModel
class PlayerVM @Inject constructor() : ViewModel() {
    private val _musicState = MutableStateFlow(MusicState())
    private val _playMode = MutableStateFlow(PlayMode.SINGLE)
    val musicState = _musicState.asStateFlow()
    val playMode = _playMode.asStateFlow()

    fun resume() {}

    fun pause() {}

    fun stop() {
    }

    fun playNext() {}

    fun playPrevious() {}

    fun remove() {}

    fun seek(ms: ULong) {

    }

    fun play(id: MusicId) {}

    fun changePlayMode() {

    }
}
