package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import uniffi.ease_client_backend.LrcMetadata
import uniffi.ease_client_backend.LyricLoadState
import uniffi.ease_client_backend.Lyrics
import javax.inject.Inject



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
    val loadedState: LyricLoadState = LyricLoadState.LOADING
)

@HiltViewModel
class LyricVM @Inject constructor() : ViewModel() {

    private val _lyricState = MutableStateFlow(LyricState())
    private val _lyricIndex = MutableStateFlow(0)
    val lyricState = _lyricState.asStateFlow()
    val lyricIndex = _lyricIndex.asStateFlow()

    fun remove() {}
}