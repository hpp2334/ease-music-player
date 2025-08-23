package com.kutedev.easemusicplayer.repositories

import com.kutedev.easemusicplayer.core.Bridge
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import uniffi.ease_client_backend.ArgRemoveMusicFromPlaylist
import uniffi.ease_client_backend.Music
import uniffi.ease_client_backend.Playlist
import uniffi.ease_client_backend.ctGetMusic
import uniffi.ease_client_backend.ctGetPlaylist
import uniffi.ease_client_backend.ctRemoveMusicFromPlaylist
import uniffi.ease_client_backend.ctsSavePreferencePlaymode
import uniffi.ease_client_backend.easeLog
import uniffi.ease_client_schema.MusicId
import uniffi.ease_client_schema.PlayMode
import uniffi.ease_client_schema.PlaylistId
import java.time.Duration
import javax.inject.Inject
import javax.inject.Singleton
import kotlin.math.max


data class SleepModeState(
    val enabled: Boolean = false,
    val expiredMs: Long = 0
)

@Singleton
class PlayerRepository @Inject constructor(
    private val bridge: Bridge,
    private val _scope: CoroutineScope
) {
    private val _sleep = MutableStateFlow(SleepModeState())
    private val _music = MutableStateFlow(null as Music?)
    private val _playlist = MutableStateFlow(null as Playlist?)
    private val _currentDuration = MutableStateFlow(Duration.ZERO)
    private val _bufferDuration = MutableStateFlow(Duration.ZERO)
    private val _playing = MutableStateFlow(false)
    private val _musicIndex = combine(_music, _playlist) {
            music, playlist ->
        if (music == null || playlist == null) {
            -1
        } else {
            playlist.musics.indexOfFirst { m -> m.meta.id == music.meta.id }
        }
    }

    private var _job: Job? = null
    private val _playMode = MutableStateFlow(PlayMode.SINGLE)
    private val _loading = MutableStateFlow(false)
    val playMode = _playMode.asStateFlow()

    val sleepState = _sleep.asStateFlow()
    val currentDuration = _currentDuration.asStateFlow()
    val bufferDuration = _bufferDuration.asStateFlow()
    val music = _music.asStateFlow()
    val playing = _playing.asStateFlow()
    val loading = _loading.asStateFlow()

    val previousMusic = combine(_playMode, _musicIndex, _playlist) {
        playMode, musicIndex, playlist ->
            if (musicIndex == -1 || playlist == null || playlist.musics.size == 0) {
                null
            } else if (musicIndex == 0 && (playMode == PlayMode.SINGLE || playMode == PlayMode.LIST)) {
                null
            } else {
                val i = (musicIndex + playlist.musics.size - 1) % playlist.musics.size
                playlist.musics[i]
            }
    }.stateIn(_scope, SharingStarted.Lazily, null)

    val nextMusic = combine(_playMode, _musicIndex, _playlist) {
            playMode, musicIndex, playlist ->
        if (musicIndex == -1 || playlist == null || playlist.musics.size == 0) {
            null
        } else if (musicIndex == playlist.musics.size - 1 && (playMode == PlayMode.SINGLE || playMode == PlayMode.LIST)) {
            null
        } else {
            val i = (musicIndex + 1) % playlist.musics.size
            playlist.musics[i]
        }
    }.stateIn(_scope, SharingStarted.Lazily, null)

    fun play(id: MusicId, playlistId: PlaylistId) {
        if (_music.value?.meta?.id == id && _playlist.value?.abstr?.meta?.id == playlistId) {
            resume()
            return
        }

        _scope.launch {
            val music = ctGetMusic(bridge.backend, id)
            val playlist = ctGetPlaylist(bridge.backend, playlistId)
            val inPlaylist = music != null && playlist != null && playlist.musics.find { music -> music.meta.id == id }.let { it -> it != null }

            if (inPlaylist) {
                _music.value = music
                _playlist.value = playlist
            } else {
                _music.value = null
                _playlist.value = null
            }
        }
    }

    fun resume() {
        _playing.value = true;
    }

    fun pause() {
        _playing.value = false;
    }

    fun stop() {
        _music.value = null
        _playlist.value = null
        _playing.value = false;
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

    fun remove() {
        val m = _music.value
        val p = _playlist.value
        _scope.launch {
            if (m != null && p != null) {
                ctRemoveMusicFromPlaylist(bridge.backend, ArgRemoveMusicFromPlaylist(
                    playlistId = p.abstr.meta.id,
                    musicId = m.meta.id
                ))
            }
        }
    }


    fun scheduleSleep(newExpiredMs: Long) {
        _job?.cancel()

        val delayMs = max(newExpiredMs - System.currentTimeMillis(), 0)
        _job = _scope.launch {
            _sleep.update { state -> state.copy(enabled = true, expiredMs = newExpiredMs) }
            easeLog("schedule sleep")
            delay(delayMs)
            easeLog("sleep scheduled")
            _sleep.update { state -> state.copy(enabled = false, expiredMs = 0) }
        }
    }

    fun seek(ms: ULong) {
        _currentDuration.value = Duration.ofMillis(ms.toLong())
    }

    fun cancelSleep() {
        _job?.cancel()
        _job = null
        _sleep.update { state -> state.copy(enabled = false, expiredMs = 0) }
    }

    fun changePlayModeToNext() {
        when (_playMode.value) {
            PlayMode.SINGLE -> {
                _playMode.value = PlayMode.SINGLE_LOOP
            }
            PlayMode.SINGLE_LOOP -> {
                _playMode.value = PlayMode.LIST
            }
            PlayMode.LIST -> {
                _playMode.value = PlayMode.LIST_LOOP
            }
            PlayMode.LIST_LOOP -> {
                _playMode.value = PlayMode.SINGLE
            }
        }

        ctsSavePreferencePlaymode(bridge.backend, _playMode.value)
    }
}
