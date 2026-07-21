package com.kutedev.easemusicplayer.singleton

import com.kutedev.easemusicplayer.core.CantodePlayer
import com.kutedev.easemusicplayer.singleton.SleepModeState
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import uniffi.ease_client_backend.ArgRemoveMusicFromPlaylist
import uniffi.ease_client_backend.Playlist
import uniffi.ease_client_backend.PlayerHandle
import uniffi.ease_client_backend.PlayerContextHandle
import uniffi.ease_client_backend.ctGetMusic
import uniffi.ease_client_backend.ctGetPlaylist
import uniffi.ease_client_backend.ctPlayerContextNew
import uniffi.ease_client_backend.ctPlayerLoadMusic
import uniffi.ease_client_backend.ctPlayerNew
import uniffi.ease_client_backend.ctPlayerPause
import uniffi.ease_client_backend.ctPlayerPlay
import uniffi.ease_client_backend.ctPlayerSeek
import uniffi.ease_client_backend.ctPlayerStop
import uniffi.ease_client_backend.ctRemoveMusicFromPlaylist
import uniffi.ease_client_backend.easeError
import uniffi.ease_client_backend.easeLog
import uniffi.ease_client_schema.MusicId
import uniffi.ease_client_schema.PlaylistId
import javax.inject.Inject
import javax.inject.Singleton
import kotlin.math.max

/**
 * Transport control surface for the player.
 *
 * Owns the cantode [PlayerHandle] / [PlayerContextHandle] and the
 * [CantodePlayer] wrapper. Exposes the same-shaped play/pause/seek/etc.
 * methods as the old MediaController-backed implementation so ViewModels
 * don't need to change.
 *
 * The lifecycle is:
 * 1. [MainActivity.onCreate] runs `bridge.initialize()`, which starts
 *    [com.kutedev.easemusicplayer.core.KeepBackendService].
 * 2. [MainActivity.onStart] calls [setupCantodePlayer], which constructs
 *    the cantode handles and the [CantodePlayer] wrapper, then publishes
 *    them via [cantodePlayerState].
 * 3. [com.kutedev.easemusicplayer.core.PlaybackService] collects
 *    [cantodePlayerState] and builds its [androidx.media3.session.MediaSession]
 *    the first time it sees a non-null value.
 */
@Singleton
class PlayerControllerRepository @Inject constructor(
    private val playerRepository: PlayerRepository,
    private val toastRepository: ToastRepository,
    private val playlistRepository: PlaylistRepository,
    private val storageRepository: StorageRepository,
    private val bridge: Bridge,
    private val _scope: CoroutineScope,
) {
    private val _playlist = playerRepository.playlist
    private val _music = playerRepository.music
    private val _sleep = MutableStateFlow(SleepModeState())

    private var _sleepJob: Job? = null
    private val nextMusic = playerRepository.nextMusic
    private val previousMusic = playerRepository.previousMusic

    /** Fires once when the cantode player reports `ENDED` for the loaded music. */
    private val _endedEvent = MutableSharedFlow<Unit>(extraBufferCapacity = 4)
    val endedEvent = _endedEvent.asSharedFlow()

    val sleepState = _sleep.asStateFlow()

    // ---- cantode handles ----

    private val _cantodePlayerState = MutableStateFlow<CantodePlayer?>(null)
    /** The [CantodePlayer] once [setupCantodePlayer] completes; null before. */
    val cantodePlayerState = _cantodePlayerState.asStateFlow()

    @Volatile private var playerContext: PlayerContextHandle? = null
    @Volatile private var handle: PlayerHandle? = null

    private var setupStarted = false

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
                if (_playlist.value?.abstr?.meta?.id == arg.playlistId
                    && _music.value?.meta?.id == arg.musicId) {
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

    /**
     * Called from [com.kutedev.easemusicplayer.MainActivity] once the
     * backend is initialized. Constructs the cantode [PlayerContextHandle]
     * + [PlayerHandle] + [CantodePlayer], then publishes the wrapper via
     * [cantodePlayerState] so [com.kutedev.easemusicplayer.core.PlaybackService]
     * can build its [androidx.media3.session.MediaSession] around it.
     *
     * [playerFactory] is supplied by the caller (which has the Context
     * needed to construct a SimpleBasePlayer). This keeps the Context
     * out of this singleton's constructor.
     *
     * Safe to call multiple times — second+ calls are no-ops.
     */
    fun setupCantodePlayer(playerFactory: (PlayerHandle) -> CantodePlayer) {
        if (setupStarted) return
        setupStarted = true
        _scope.launch(Dispatchers.Main) {
            try {
                val ctx = ctPlayerContextNew()
                val handle = ctPlayerNew(ctx)
                val player = playerFactory(handle)
                playerContext = ctx
                this@PlayerControllerRepository.handle = handle

                // Route SimpleBasePlayer STATE_ENDED → _endedEvent so
                // PlaybackService can fire auto-advance.
                player.addListener(object : androidx.media3.common.Player.Listener {
                    override fun onPlaybackStateChanged(playbackState: Int) {
                        if (playbackState == androidx.media3.common.Player.STATE_ENDED) {
                            _scope.launch { _endedEvent.emit(Unit) }
                        }
                    }
                })

                _cantodePlayerState.value = player
                playerRepository.reload()
                easeLog("cantode player setup complete")
            } catch (e: Exception) {
                setupStarted = false
                easeError("cantode player setup failed: $e")
                _scope.launch { toastRepository.emitToast("player setup failed: $e") }
            }
        }
    }

    /** Current position in milliseconds (for the PlayerVM poll). */
    fun getCurrentPosition(): Long {
        return _cantodePlayerState.value?.currentPosition ?: 0L
    }

    /** Buffered position in milliseconds. */
    fun getBufferedPosition(): Long {
        return _cantodePlayerState.value?.bufferedPosition ?: 0L
    }

    fun play(id: MusicId, playlistId: PlaylistId) {
        val handle = handle ?: run {
            easeError("play: cantode player not ready"); return
        }
        val cantodePlayer = _cantodePlayerState.value ?: return

        // Same music already current → just resume.
        if (_music.value?.meta?.id == id && _playlist.value?.abstr?.meta?.id == playlistId) {
            resume(); return
        }

        _scope.launch(Dispatchers.Main) {
            stop()

            val music = bridge.run { ctGetMusic(it, id) }
            val playlist = bridge.run { ctGetPlaylist(it, playlistId) }
            val inPlaylist = music != null && playlist != null &&
                playlist.musics.any { it.meta.id == id }

            if (inPlaylist) {
                playerRepository.setCurrent(music!!, playlist!!)
                cantodePlayer.setCurrentMedia(id, music.meta.title)
                bridge.run { ctPlayerLoadMusic(it, handle, id) }
                ctPlayerPlay(handle)
            } else {
                playerRepository.resetCurrent()
            }
        }
    }

    fun resume() {
        val handle = handle ?: return
        _scope.launch { ctPlayerPlay(handle) }
    }

    fun pause() {
        val handle = handle ?: return
        _scope.launch { ctPlayerPause(handle) }
    }

    fun stop() {
        val handle = handle ?: return
        _scope.launch {
            ctPlayerStop(handle)
            playerRepository.resetCurrent()
            _cantodePlayerState.value?.setCurrentMedia(null, null)
        }
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
        val handle = handle ?: return
        _scope.launch { ctPlayerSeek(handle, ms) }
    }

    fun scheduleSleep(newExpiredMs: Long) {
        _sleepJob?.cancel()
        val delayMs = max(newExpiredMs - System.currentTimeMillis(), 0)
        _sleepJob = _scope.launch {
            _sleep.update { it.copy(enabled = true, expiredMs = newExpiredMs) }
            easeLog("schedule sleep")
            delay(delayMs)
            easeLog("sleep scheduled")
            playerRepository.emitPauseRequest()
            _sleep.update { it.copy(enabled = false, expiredMs = 0) }
        }
    }

    fun refreshPlaylistIfMatch(playlist: Playlist) {
        playerRepository.refreshPlaylistIfMatch(playlist)
    }

    fun cancelSleep() {
        _sleepJob?.cancel()
        _sleepJob = null
        _sleep.update { it.copy(enabled = false, expiredMs = 0) }
    }

    fun remove() {
        val m = _music.value
        val p = _playlist.value
        _scope.launch {
            if (m != null && p != null) {
                bridge.run {
                    ctRemoveMusicFromPlaylist(
                        it,
                        ArgRemoveMusicFromPlaylist(
                            playlistId = p.abstr.meta.id,
                            musicId = m.meta.id,
                        ),
                    )
                }
            }
        }
    }
}
