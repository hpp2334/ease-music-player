package com.kutedev.easemusicplayer.singleton

import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.Player.COMMAND_PLAY_PAUSE
import androidx.media3.common.Player.COMMAND_SEEK_IN_CURRENT_MEDIA_ITEM
import androidx.media3.common.Player.COMMAND_STOP
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.session.MediaController
import com.kutedev.easemusicplayer.core.BuildMediaContext
import com.kutedev.easemusicplayer.core.playUtil
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import uniffi.ease_client_backend.ArgRemoveMusicFromPlaylist
import uniffi.ease_client_backend.Playlist
import uniffi.ease_client_backend.ctGetMusic
import uniffi.ease_client_backend.ctGetPlaylist
import uniffi.ease_client_backend.ctRemoveMusicFromPlaylist
import uniffi.ease_client_backend.easeError
import uniffi.ease_client_backend.easeLog
import uniffi.ease_client_schema.MusicId
import uniffi.ease_client_schema.PlaylistId
import java.time.Duration
import kotlin.math.max

class PlayerControllerRepository constructor(
    private val playerRepository: PlayerRepository,
    private val toastRepository: ToastRepository,
    private val playlistRepository: PlaylistRepository,
    private val storageRepository: StorageRepository,
    private val bridge: Bridge,
    private val _scope: CoroutineScope
) : PlayerController {
    private var _mediaController: MediaController? = null
    private val _playlist = playerRepository.playlist
    private val _music = playerRepository.music
    private val _sleep = MutableStateFlow(SleepModeState())

    private var _sleepJob: Job? = null
    private val nextMusic = playerRepository.nextMusic
    private val previousMusic = playerRepository.previousMusic

    override val sleepState = _sleep.asStateFlow()

    init {
        _scope.launch(Dispatchers.Main) {
            playlistRepository.preRemovePlaylistEvent.collect { id ->
                if (_playlist.value?.abstr?.meta?.id == id) {
                    stop()
                }
            }
        }
        _scope.launch(Dispatchers.Main) {
            playlistRepository.preRemoveMusicEvent.collect { arg ->
                if (_playlist.value?.abstr?.meta?.id == arg.playlistId && _music.value?.meta?.id == arg.musicId) {
                    stop()
                }
            }
        }
        _scope.launch(Dispatchers.Main) {
            storageRepository.preRemoveStorageEvent.collect { id ->
                if (_music.value?.loc?.storageId == id) {
                    stop()
                }
            }
        }
    }

    fun setupMediaController(mediaController: MediaController) {
        _mediaController = mediaController

        mediaController.addListener(object : Player.Listener {
            override fun onPlayerError(error: PlaybackException) {
                super.onPlayerError(error)

                _scope.launch {
                    toastRepository.emitToast(error.toString())
                }
            }
        })
        _scope.launch {
            playerRepository.reload()
        }
        easeLog("media controller setup")
    }

    fun destroyMediaController() {
        _mediaController?.release()
        _mediaController = null

        easeLog("media controller destroy")
    }

    override fun getCurrentPosition(): Long {
        return _mediaController?.currentPosition ?: 0
    }

    override fun getBufferedPosition(): Long {
        return _mediaController?.bufferedPosition ?: 0
    }

    override fun play(id: MusicId, playlistId: PlaylistId) {
        val mediaController = _mediaController ?: return

        if (_music.value?.meta?.id == id && _playlist.value?.abstr?.meta?.id == playlistId) {
            resume()
            return
        }

        _scope.launch(Dispatchers.Main) {
            stop()

            val music = bridge.run { ctGetMusic(it, id) }
            val playlist = bridge.run { ctGetPlaylist(it, playlistId) }
            val inPlaylist = music != null && playlist != null && playlist.musics.find { music -> music.meta.id == id }.let { it -> it != null }

            if (inPlaylist) {
                playerRepository.setCurrent(music, playlist)

                playUtil(BuildMediaContext(bridge = bridge, scope = _scope), music, mediaController)
            } else {
                playerRepository.resetCurrent()
            }
        }
    }

    override fun resume() {
        val mediaController = _mediaController ?: return

        if (mediaController.isCommandAvailable(COMMAND_PLAY_PAUSE)) {
            mediaController.play()
        } else {
            easeError("media controller resume failed, command COMMAND_PLAY_PAUSE is unavailable")
        }
    }

    override fun pause() {
        val mediaController = _mediaController ?: return

        if (mediaController.isCommandAvailable(COMMAND_PLAY_PAUSE)) {
            mediaController.pause()
        } else {
            easeError("media controller pause failed, command COMMAND_PLAY_PAUSE is unavailable")
        }
    }

    override fun stop() {
        val mediaController = _mediaController ?: return

        if (mediaController.isCommandAvailable(COMMAND_STOP)) {
            mediaController.stop()
        } else {
            easeError("media controller stop failed, command COMMAND_STOP is unavailable")
        }

        playerRepository.resetCurrent()
    }

    override fun playNext() {
        val m = nextMusic.value
        val p = _playlist.value
        if (m != null && p != null) {
            play(m.meta.id, p.abstr.meta.id)
        }
    }

    override fun playPrevious() {
        val m = previousMusic.value
        val p = _playlist.value
        if (m != null && p != null) {
            play(m.meta.id, p.abstr.meta.id)
        }
    }

    override fun seek(ms: ULong) {
        val mediaController = _mediaController ?: return

        if (mediaController.isCommandAvailable(COMMAND_SEEK_IN_CURRENT_MEDIA_ITEM)) {
            mediaController.seekTo(ms.toLong())
        } else {
            easeError("media controller seek failed, command COMMAND_SEEK_IN_CURRENT_MEDIA_ITEM is unavailable")
        }
    }

    override fun scheduleSleep(newExpiredMs: Long) {
        _sleepJob?.cancel()

        val delayMs = max(newExpiredMs - System.currentTimeMillis(), 0)
        _sleepJob = _scope.launch {
            _sleep.update { state -> state.copy(enabled = true, expiredMs = newExpiredMs) }
            easeLog("schedule sleep")
            delay(delayMs)
            easeLog("sleep scheduled")
            playerRepository.emitPauseRequest()
            _sleep.update { state -> state.copy(enabled = false, expiredMs = 0) }
        }
    }

    override fun refreshPlaylistIfMatch(playlist: Playlist) {
        playerRepository.refreshPlaylistIfMatch(playlist)
    }

    override fun cancelSleep() {
        _sleepJob?.cancel()
        _sleepJob = null
        _sleep.update { state -> state.copy(enabled = false, expiredMs = 0) }
    }

    override fun remove() {
        val m = _music.value
        val p = _playlist.value
        _scope.launch {
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

}
