package com.kutedev.easemusicplayer.singleton

import com.kutedev.easemusicplayer.audio.StreamingHttpServer
import javafx.application.Platform
import javafx.scene.media.Media
import javafx.scene.media.MediaPlayer
import javafx.util.Duration
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import uniffi.ease_client_backend.ArgRemoveMusicFromPlaylist
import uniffi.ease_client_backend.ArgUpdateMusicDuration
import uniffi.ease_client_backend.Playlist
import uniffi.ease_client_backend.ctGetMusic
import uniffi.ease_client_backend.ctGetPlaylist
import uniffi.ease_client_backend.ctRemoveMusicFromPlaylist
import uniffi.ease_client_backend.ctsUpdateMusicDuration
import uniffi.ease_client_backend.easeError
import uniffi.ease_client_backend.easeLog
import uniffi.ease_client_schema.MusicId
import uniffi.ease_client_schema.PlaylistId
import java.time.Duration as JavaDuration
import kotlin.math.max

class DesktopPlayerController(
    private val playerRepository: PlayerRepository,
    private val toastRepository: ToastRepository,
    private val playlistRepository: PlaylistRepository,
    private val storageRepository: StorageRepository,
    private val bridge: Bridge,
    private val scope: CoroutineScope
) : PlayerController {

    private val httpServer = StreamingHttpServer(bridge, scope)
    private var _mediaPlayer: MediaPlayer? = null

    private val _sleep = MutableStateFlow(SleepModeState())
    private var _sleepJob: Job? = null
    @Volatile private var _currentMusicId: MusicId? = null

    override val sleepState = _sleep.asStateFlow()

    init {
        httpServer.start()
        easeLog("desktop http server started on port ${httpServer.port}")

        scope.launch {
            playlistRepository.preRemovePlaylistEvent.collect { id ->
                if (playerRepository.playlist.value?.abstr?.meta?.id == id) {
                    stop()
                }
            }
        }
        scope.launch {
            playlistRepository.preRemoveMusicEvent.collect { arg ->
                if (playerRepository.playlist.value?.abstr?.meta?.id == arg.playlistId &&
                    playerRepository.music.value?.meta?.id == arg.musicId) {
                    stop()
                }
            }
        }
        scope.launch {
            storageRepository.preRemoveStorageEvent.collect { id ->
                if (playerRepository.music.value?.loc?.storageId == id) {
                    stop()
                }
            }
        }
    }

    private fun createMediaPlayer(url: String, musicId: MusicId) {
        Platform.runLater {
            val old = _mediaPlayer
            if (old != null) {
                old.stop()
                old.dispose()
            }

            val media = Media(url)
            val player = MediaPlayer(media)

            player.setOnPlaying {
                playerRepository.setIsPlaying(true)
                playerRepository.setIsLoading(false)
            }

            player.setOnPaused {
                playerRepository.setIsPlaying(false)
            }

            player.setOnStopped {
                playerRepository.setIsPlaying(false)
            }

            player.setOnStalled {
                playerRepository.setIsLoading(true)
            }

            player.setOnReady {
                playerRepository.setIsLoading(false)
                val durationMs = media.duration.toMillis()
                if (durationMs > 0 && !durationMs.isNaN() && !durationMs.isInfinite()) {
                    probeDuration(musicId, durationMs.toLong())
                }
            }

            player.setOnEndOfMedia {
                playerRepository.setIsPlaying(false)
                val next = playerRepository.onCompleteMusic.value
                val playlist = playerRepository.playlist.value
                if (next != null && playlist != null) {
                    play(next.meta.id, playlist.abstr.meta.id)
                }
            }

            player.setOnError {
                playerRepository.setIsLoading(false)
                val err = player.error
                val msg = err?.message ?: "Unknown playback error"
                easeError("media player error: $msg")
                scope.launch { toastRepository.emitToast(msg) }
            }

            _mediaPlayer = player
            player.play()
        }
    }

    private fun probeDuration(musicId: MusicId, durationMs: Long) {
        scope.launch {
            val current = _currentMusicId
            if (current != null && current == musicId) {
                val duration = JavaDuration.ofMillis(durationMs)
                bridge.run {
                    ctsUpdateMusicDuration(it, ArgUpdateMusicDuration(
                        id = musicId,
                        duration = duration
                    ))
                }
                playerRepository.notifyDurationChanged()
            }
        }
    }

    override fun play(id: MusicId, playlistId: PlaylistId) {
        if (playerRepository.music.value?.meta?.id == id &&
            playerRepository.playlist.value?.abstr?.meta?.id == playlistId) {
            resume()
            return
        }

        scope.launch(Dispatchers.IO) {
            val music = bridge.run { ctGetMusic(it, id) }
            val playlist = bridge.run { ctGetPlaylist(it, playlistId) }
            val inPlaylist = music != null && playlist != null &&
                playlist.musics.any { it.meta.id == id }

            if (inPlaylist) {
                _currentMusicId = id
                playerRepository.setCurrent(music!!, playlist!!)
                playerRepository.setIsLoading(true)

                val url = "${httpServer.baseUrl}/music/${id.value}"
                easeLog("desktop player playing: $url")
                createMediaPlayer(url, id)
            } else {
                playerRepository.resetCurrent()
            }
        }
    }

    override fun resume() {
        Platform.runLater { _mediaPlayer?.play() }
    }

    override fun pause() {
        Platform.runLater { _mediaPlayer?.pause() }
    }

    override fun stop() {
        Platform.runLater {
            _mediaPlayer?.stop()
        }
        playerRepository.resetCurrent()
    }

    override fun playNext() {
        val m = playerRepository.nextMusic.value
        val p = playerRepository.playlist.value
        if (m != null && p != null) {
            play(m.meta.id, p.abstr.meta.id)
        }
    }

    override fun playPrevious() {
        val m = playerRepository.previousMusic.value
        val p = playerRepository.playlist.value
        if (m != null && p != null) {
            play(m.meta.id, p.abstr.meta.id)
        }
    }

    override fun seek(ms: ULong) {
        Platform.runLater {
            _mediaPlayer?.seek(Duration.millis(ms.toLong().toDouble()))
        }
    }

    override fun getCurrentPosition(): Long {
        return _mediaPlayer?.currentTime?.toMillis()?.toLong() ?: 0L
    }

    override fun getBufferedPosition(): Long {
        return _mediaPlayer?.currentTime?.toMillis()?.toLong() ?: 0L
    }

    override fun scheduleSleep(newExpiredMs: Long) {
        _sleepJob?.cancel()

        val delayMs = max(newExpiredMs - System.currentTimeMillis(), 0)
        _sleepJob = scope.launch {
            _sleep.update { it.copy(enabled = true, expiredMs = newExpiredMs) }
            easeLog("schedule sleep")
            delay(delayMs)
            easeLog("sleep expired, pausing")
            pause()
            _sleep.update { it.copy(enabled = false, expiredMs = 0) }
        }
    }

    override fun cancelSleep() {
        _sleepJob?.cancel()
        _sleepJob = null
        _sleep.update { it.copy(enabled = false, expiredMs = 0) }
    }

    override fun refreshPlaylistIfMatch(playlist: Playlist) {
        playerRepository.refreshPlaylistIfMatch(playlist)
    }

    override fun remove() {
        val m = playerRepository.music.value
        val p = playerRepository.playlist.value
        scope.launch {
            if (m != null && p != null) {
                bridge.run {
                    ctRemoveMusicFromPlaylist(it, ArgRemoveMusicFromPlaylist(
                        playlistId = p.abstr.meta.id,
                        musicId = m.meta.id
                    ))
                }
            }
        }
    }

    fun destroy() {
        Platform.runLater {
            _mediaPlayer?.stop()
            _mediaPlayer?.dispose()
            _mediaPlayer = null
        }
        httpServer.stop()
        easeLog("desktop player destroyed")
    }
}
