package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kutedev.easemusicplayer.singleton.PlayerControllerRepository
import com.kutedev.easemusicplayer.singleton.PlayerRepository
import com.kutedev.easemusicplayer.utils.formatDuration
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import uniffi.ease_client_schema.DataSourceKey
import uniffi.ease_client_schema.MusicId
import uniffi.ease_client_schema.PlaylistId
import java.time.Duration
import javax.inject.Inject
import kotlin.time.DurationUnit
import kotlin.time.toDuration
import kotlin.time.toJavaDuration

@HiltViewModel
class PlayerVM @Inject constructor(
    private val playerRepository: PlayerRepository,
    private val playerControllerRepository: PlayerControllerRepository
) : ViewModel() {
    private val _currentDuration = MutableStateFlow(Duration.ZERO)
    private val _bufferDuration = MutableStateFlow(Duration.ZERO)
    val music = playerRepository.music
    val previousMusic = playerRepository.previousMusic
    val nextMusic = playerRepository.nextMusic
    val playing = playerRepository.playing
    val currentDuration = _currentDuration.asStateFlow()
    val bufferDuration = _bufferDuration.asStateFlow()
    val playMode = playerRepository.playMode
    val loading = playerRepository.loading

    init {
        viewModelScope.launch {
            while (true) {
                syncPosition()
                delay(1000)
            }
        }
        viewModelScope.launch {
            playerRepository.durationChanged.collect {
                syncPosition()
            }
        }
    }

    fun resume() {
        playerRepository.remove()
    }

    fun pause() {
        playerControllerRepository.pause()
    }

    fun stop() {
        playerControllerRepository.stop()
    }

    fun playNext() {
        playerControllerRepository.playNext()
    }

    fun playPrevious() {
        playerControllerRepository.playPrevious()
    }

    fun remove() {
        playerRepository.remove()
    }

    fun seek(ms: ULong) {
        playerControllerRepository.seek(ms)
    }

    fun play(id: MusicId, playlistId: PlaylistId) {
        playerControllerRepository.play(id, playlistId)
    }

    fun changePlayModeToNext() {
        playerRepository.changePlayModeToNext()
    }

    fun syncPosition() {
        _currentDuration.value = playerControllerRepository.getCurrentPosition().toDuration(
            DurationUnit.MILLISECONDS).toJavaDuration()
        _bufferDuration.value = playerControllerRepository.getBufferedPosition().toDuration(
            DurationUnit.MILLISECONDS).toJavaDuration()
    }
}
