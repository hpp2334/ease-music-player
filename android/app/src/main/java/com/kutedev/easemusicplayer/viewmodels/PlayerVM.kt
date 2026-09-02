package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kutedev.easemusicplayer.singleton.PlayerControllerRepository
import com.kutedev.easemusicplayer.singleton.PlayerRepository
import com.kutedev.easemusicplayer.singleton.ToastRepository
import com.kutedev.easemusicplayer.utils.formatDuration
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import com.kutedev.easemusicplayer.singleton.types.DataSourceKey
import com.kutedev.easemusicplayer.singleton.types.MusicId
import com.kutedev.easemusicplayer.singleton.types.PlaylistId
import javax.inject.Inject

@HiltViewModel
class PlayerVM @Inject constructor(
    private val playerRepository: PlayerRepository,
    private val playerControllerRepository: PlayerControllerRepository,
) : ViewModel() {
    // Position / buffered-position in milliseconds (Long, not java.time.Duration).
    private val _currentMs = MutableStateFlow(0L)
    private val _bufferMs = MutableStateFlow(0L)
    val music = playerRepository.music
    val previousMusic = playerRepository.previousMusic
    val nextMusic = playerRepository.nextMusic
    val playing = playerRepository.playing
    val currentMs = _currentMs.asStateFlow()
    val bufferMs = _bufferMs.asStateFlow()
    val playMode = playerRepository.playMode
    val loading = playerRepository.loading

    /** Music id mirrored from [music]; used to detect track switches. */
    private var lastMusicId: MusicId? = null

    val lyricIndex = combine(currentMs, music) { currentMs, music ->
        music?.lyric?.data?.lines?.indexOfLast { it.duration <= currentMs } ?: -1
    }.stateIn(viewModelScope, SharingStarted.Lazily, -1)

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
        viewModelScope.launch {
            // Track switch: zero the progress immediately instead of
            // showing the previous track's position for up to one poll
            // tick (the 1 Hz loop below refreshes it right after).
            playerRepository.music.collect { music ->
                val id = music?.meta?.id
                if (id != lastMusicId) {
                    lastMusicId = id
                    _currentMs.value = 0L
                    _bufferMs.value = 0L
                    syncPosition()
                }
            }
        }
    }

    fun resume() = playerControllerRepository.resume()
    fun pause() = playerControllerRepository.pause()
    fun stop() = playerControllerRepository.stop()
    fun playNext() = playerControllerRepository.playNext()
    fun playPrevious() = playerControllerRepository.playPrevious()
    fun remove() = playerRepository.remove()
    fun seek(ms: ULong) = playerControllerRepository.seek(ms)
    fun play(id: MusicId, playlistId: PlaylistId) = playerControllerRepository.play(id, playlistId)
    fun changePlayModeToNext() = playerRepository.changePlayModeToNext()
    fun removeLyric() = playerRepository.removeLyric()

    fun syncPosition() {
        _currentMs.value = playerControllerRepository.getCurrentPosition()
        _bufferMs.value = playerControllerRepository.getBufferedPosition()
    }
}
