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
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.launch
import uniffi.ease_client_backend.ArgRemoveMusicFromPlaylist
import uniffi.ease_client_backend.ctGetMusic
import uniffi.ease_client_backend.ctGetPlaylist
import uniffi.ease_client_backend.ctRemoveMusicFromPlaylist
import uniffi.ease_client_backend.easeError
import uniffi.ease_client_backend.easeLog
import uniffi.ease_client_schema.MusicId
import uniffi.ease_client_schema.PlaylistId
import java.time.Duration
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class PlayerControllerRepository @Inject constructor(
    private val playerRepository: PlayerRepository,
    private val toastRepository: ToastRepository,
    private val bridge: Bridge,
    private val _scope: CoroutineScope
) {
    private var _mediaController: MediaController? = null
    private val _playlist = playerRepository.playlist
    private val _music = playerRepository.music
    private val nextMusic = playerRepository.nextMusic
    private val previousMusic = playerRepository.previousMusic

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
        easeLog("media controller setup")
    }

    fun destroyMediaController() {
        _mediaController?.release()
        _mediaController = null
    }

    fun getCurrentPosition(): Long {
        return _mediaController?.currentPosition ?: 0
    }

    fun getBufferedPosition(): Long {
        return _mediaController?.bufferedPosition ?: 0
    }

    fun play(id: MusicId, playlistId: PlaylistId) {
        val mediaController = _mediaController ?: return

        if (_music.value?.meta?.id == id && _playlist.value?.abstr?.meta?.id == playlistId) {
            resume()
            return
        }

        _scope.launch(Dispatchers.Main) {
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

    fun resume() {
        val mediaController = _mediaController ?: return

        if (mediaController.isCommandAvailable(COMMAND_PLAY_PAUSE)) {
            mediaController.play()
        } else {
            easeError("media controller resume failed, command COMMAND_PLAY_PAUSE is unavailable")
        }
    }

    fun pause() {
        val mediaController = _mediaController ?: return

        if (mediaController.isCommandAvailable(COMMAND_PLAY_PAUSE)) {
            mediaController.pause()
        } else {
            easeError("media controller pause failed, command COMMAND_PLAY_PAUSE is unavailable")
        }
    }

    fun stop() {
        val mediaController = _mediaController ?: return

        if (mediaController.isCommandAvailable(COMMAND_STOP)) {
            mediaController.stop()
        } else {
            easeError("media controller stop failed, command COMMAND_STOP is unavailable")
        }

        playerRepository.resetCurrent()
    }

    fun playNext() {
        val m = nextMusic.value
        val p = _playlist.value
        if (m != null && p != null) {
            play(m.meta.id, p.abstr.meta.id)
        }
    }

    fun playPrevious() {
        val m = previousMusic.value
        val p = _playlist.value
        if (m != null && p != null) {
            play(m.meta.id, p.abstr.meta.id)
        }
    }

    fun seek(ms: ULong) {
        val mediaController = _mediaController ?: return

        val duration = Duration.ofMillis(ms.toLong())

        if (mediaController.isCommandAvailable(COMMAND_SEEK_IN_CURRENT_MEDIA_ITEM)) {
            mediaController.seekTo(ms.toLong())
        } else {
            easeError("media controller seek failed, command COMMAND_SEEK_IN_CURRENT_MEDIA_ITEM is unavailable")
        }
    }

    fun remove() {
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
