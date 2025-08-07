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
import javax.inject.Inject

data class MusicState(
    val id: MusicId? = null,
    val playing: Boolean = false,
    val title: String = "",
    val cover: DataSourceKey? = null,
    val currentDurationMs: ULong = 0uL,
    val totalDuration: String = "00:00",
    val totalDurationMs: ULong = 0uL,
    val canPlayNext: Boolean = false,
    val loading: Boolean = false
)

enum class LyricLoadedState {
    Loading,
    Loaded,
    Missing,
}

data class LyricState(
    val lyrics: Lyrics = Lyrics(
        metdata = LrcMetadata(
            artist = "",
            album = "",
            title = "",
            lyricist = "",
            author = "",
            length = "",
            offset = ""
        ),
        lines = emptyList()
    ),
    val loadedState: LyricLoadedState
)

@HiltViewModel
class PlayerVM @Inject constructor() : ViewModel() {

    private val _musicState = MutableStateFlow(MusicState())
    private val _lyricState = MutableStateFlow(LyricLoadedState.Loading)
    val musicState = _musicState.asStateFlow()
    val lyricState = _lyricState.asStateFlow()

    fun resume() {}

    fun pause() {}

    fun stop() {
    }

    fun playNext() {}

    fun playPrevious() {}

    fun removeLyric() {}
}
