package com.kutedev.easemusicplayer

import com.kutedev.easemusicplayer.singleton.PlayerController
import com.kutedev.easemusicplayer.singleton.PlayerRepository
import com.kutedev.easemusicplayer.singleton.SleepModeState
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import uniffi.ease_client_backend.Playlist
import uniffi.ease_client_schema.MusicId
import uniffi.ease_client_schema.PlaylistId

class TestPlayerController(
    private val playerRepository: PlayerRepository
) : PlayerController {

    private val _sleep = MutableStateFlow(SleepModeState())
    override val sleepState = _sleep.asStateFlow()

    var isPlaying = false
        private set
    var playCallCount = 0
        private set
    var pauseCallCount = 0
        private set
    var currentMusicId: MusicId? = null
        private set

    override fun play(id: MusicId, playlistId: PlaylistId) {
        playCallCount++
        currentMusicId = id
        isPlaying = true
        playerRepository.setIsPlaying(true)
        playerRepository.setIsLoading(false)
    }

    override fun resume() {
        isPlaying = true
        playerRepository.setIsPlaying(true)
    }

    override fun pause() {
        pauseCallCount++
        isPlaying = false
        playerRepository.setIsPlaying(false)
    }

    override fun stop() {
        isPlaying = false
        playerRepository.setIsPlaying(false)
        playerRepository.resetCurrent()
    }

    override fun playNext() {}
    override fun playPrevious() {}
    override fun seek(ms: ULong) {}
    override fun getCurrentPosition(): Long = 0L
    override fun getBufferedPosition(): Long = 0L
    override fun scheduleSleep(newExpiredMs: Long) {}
    override fun cancelSleep() {}
    override fun refreshPlaylistIfMatch(playlist: Playlist) {}
    override fun remove() {}
}
