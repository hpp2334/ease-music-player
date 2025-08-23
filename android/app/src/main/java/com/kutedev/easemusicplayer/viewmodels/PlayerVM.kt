package com.kutedev.easemusicplayer.viewmodels

import androidx.lifecycle.ViewModel
import com.kutedev.easemusicplayer.repositories.PlayerRepository
import com.kutedev.easemusicplayer.utils.formatDuration
import dagger.hilt.android.lifecycle.HiltViewModel
import uniffi.ease_client_schema.DataSourceKey
import uniffi.ease_client_schema.MusicId
import uniffi.ease_client_schema.PlaylistId
import java.time.Duration
import javax.inject.Inject




data class MusicState(
    val id: MusicId? = null,
    val playing: Boolean = false,
    val title: String = "",
    val cover: DataSourceKey? = null,
    val previousCover: DataSourceKey? = null,
    val nextCover: DataSourceKey? = null,
    val currentDurationMs: ULong = 0uL,
    val currentDuration: String = formatDuration(null as Duration?),
    val totalDuration: String = formatDuration(null as Duration?),
    val totalDurationMs: ULong = 0uL,
    val bufferDurationMs: ULong = 0uL,
    val canPlayNext: Boolean = false,
    val canPlayPrevious: Boolean = false,
    val loading: Boolean = false
)


@HiltViewModel
class PlayerVM @Inject constructor(
    private val playerRepository: PlayerRepository
) : ViewModel() {
    val music = playerRepository.music
    val previousMusic = playerRepository.previousMusic
    val nextMusic = playerRepository.nextMusic
    val playing = playerRepository.playing
    val currentDuration = playerRepository.currentDuration
    val bufferDuration = playerRepository.bufferDuration
    val playMode = playerRepository.playMode
    val loading = playerRepository.loading

    fun resume() {
        playerRepository.remove()
    }

    fun pause() {
        playerRepository.pause()
    }

    fun stop() {
        playerRepository.stop()
    }

    fun playNext() {
        playerRepository.playNext()
    }

    fun playPrevious() {
        playerRepository.playPrevious()
    }

    fun remove() {
        playerRepository.remove()
    }

    fun seek(ms: ULong) {
        playerRepository.seek(ms)
    }

    fun play(id: MusicId, playlistId: PlaylistId) {
        playerRepository.play(id, playlistId)
    }

    fun changePlayModeToNext() {
        playerRepository.changePlayModeToNext()
    }
}
