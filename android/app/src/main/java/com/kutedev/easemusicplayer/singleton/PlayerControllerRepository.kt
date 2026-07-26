package com.kutedev.easemusicplayer.singleton

import android.content.Context
import com.kutedev.easemusicplayer.core.CantodeEngine
import com.kutedev.easemusicplayer.core.PlaybackService
import com.kutedev.easemusicplayer.singleton.SleepModeState
import dagger.hilt.android.qualifiers.ApplicationContext
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
import uniffi.ease_client_backend.ctPlayerDurationMs
import uniffi.ease_client_backend.ctPlayerLoadMusic
import uniffi.ease_client_backend.ctPlayerNew
import uniffi.ease_client_backend.ctPlayerPause
import uniffi.ease_client_backend.ctPlayerPlay
import uniffi.ease_client_backend.ctPlayerPositionMs
import uniffi.ease_client_backend.ctPlayerSeek
import uniffi.ease_client_backend.ctPlayerState
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
    @ApplicationContext private val cx: Context,
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

    /**
     * Plugin event bus. Collected by [PluginRepository] and dispatched to
     * each enabled plugin whose `manifest.json` `events` array contains
     * the event's [PluginEvent.type] string.
     *
     * `extraBufferCapacity = 16` so a slow plugin consumer never blocks
     * the producer (play/pause/stop run on the UI thread).
     */
    private val _pluginEvents = MutableSharedFlow<PluginEvent>(extraBufferCapacity = 16)
    val pluginEvents = _pluginEvents.asSharedFlow()

    val sleepState = _sleep.asStateFlow()

    // ---- cantode handles ----

    private val _cantodeEngine = MutableStateFlow<CantodeEngine?>(null)
    /** The [CantodeEngine] once [setupCantodeEngine] completes; null before. */
    val cantodeEngine = _cantodeEngine.asStateFlow()

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
        // Auto-advance on ENDED (replaces the old PlaybackService-side
        // playOnComplete). _endedEvent is fed by collecting
        // [CantodeEngine.endedEvent] inside [setupCantodeEngine].
        _scope.launch(Dispatchers.Main) {
            _endedEvent.collect {
                playOnComplete()
            }
        }
    }

    /**
     * Called from [com.kutedev.easemusicplayer.MainActivity] once the
     * backend is initialized. Constructs the cantode [PlayerContextHandle]
     * + [PlayerHandle] + [CantodeEngine], then publishes the engine via
     * [cantodeEngine].
     *
     * [engineFactory] is supplied by the caller (which has the app
     * [CoroutineScope] needed to construct a [CantodeEngine]). This
     * keeps the scope out of this singleton's constructor.
     *
     * Safe to call multiple times — second+ calls are no-ops.
     */
    fun setupCantodeEngine(engineFactory: (PlayerHandle) -> CantodeEngine) {
        if (setupStarted) return
        setupStarted = true
        _scope.launch(Dispatchers.Main) {
            try {
                val ctx = ctPlayerContextNew()
                val handle = ctPlayerNew(ctx)
                val engine = engineFactory(handle)
                playerContext = ctx
                this@PlayerControllerRepository.handle = handle

                // Route cantode ENDED → _endedEvent (drives auto-advance
                // via the init {} collector above) and emit the
                // MusicComplete plugin event.
                _scope.launch {
                    engine.endedEvent.collect { musicId ->
                        _endedEvent.emit(Unit)
                        _pluginEvents.tryEmit(
                            PluginEvent.MusicComplete(
                                musicId = musicId,
                                title = _music.value?.meta?.title ?: "",
                                timestamp = System.currentTimeMillis(),
                            )
                        )
                    }
                }

                _cantodeEngine.value = engine
                playerRepository.reload()
                easeLog("cantode engine setup complete")
            } catch (e: Exception) {
                setupStarted = false
                easeError("cantode engine setup failed: $e")
                _scope.launch { toastRepository.emitToast("player setup failed: $e") }
            }
        }
    }

    /**
     * Current position in milliseconds (for the PlayerVM poll).
     *
     * Calls cantode FFI directly (sync UniFFI reads are safe from any
     * thread) instead of going through the [CantodePlayer] media3 wrapper,
     * which would require a main-thread affinity check and crash when
     * called from a background coroutine.
     */
    fun getCurrentPosition(): Long {
        val handle = handle ?: return 0L
        return ctPlayerPositionMs(handle).toLong()
    }

    /**
     * Buffered position in milliseconds.
     *
     * Cantode has no separate buffered-position FFI; the decoder runs
     * ahead of the output device but exposes only the rendered position,
     * so we report [getCurrentPosition] as a best-effort lower bound.
     */
    fun getBufferedPosition(): Long = getCurrentPosition()

    fun play(id: MusicId, playlistId: PlaylistId) {
        val handle = handle ?: run {
            easeError("play: cantode player not ready"); return
        }
        val engine = _cantodeEngine.value ?: return

        // Same music already current → just resume.
        if (_music.value?.meta?.id == id && _playlist.value?.abstr?.meta?.id == playlistId) {
            resume(); return
        }

        // Ensure the playback service is up so the media notification +
        // MediaSession exist before we start pushing state.
        runCatching { PlaybackService.start(cx) }

        _scope.launch(Dispatchers.Main) {
            stop()

            val music = bridge.run { ctGetMusic(it, id) }
            val playlist = bridge.run { ctGetPlaylist(it, playlistId) }
            val inPlaylist = music != null && playlist != null &&
                playlist.musics.any { it.meta.id == id }

            if (inPlaylist) {
                playerRepository.setCurrent(music!!, playlist!!)
                engine.setCurrentMedia(id, music.meta.title)
                bridge.run { ctPlayerLoadMusic(it, handle, id) }
                ctPlayerPlay(handle)
                _pluginEvents.tryEmit(
                    PluginEvent.MusicPlay(
                        musicId = id,
                        title = music.meta.title,
                        timestamp = System.currentTimeMillis(),
                    )
                )
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
        _scope.launch {
            ctPlayerPause(handle)
            _pluginEvents.tryEmit(
                PluginEvent.MusicPause(
                    musicId = _music.value?.meta?.id,
                    timestamp = System.currentTimeMillis(),
                    positionMs = getCurrentPosition(),
                )
            )
        }
    }

    fun stop() {
        val handle = handle ?: return
        _scope.launch {
            ctPlayerStop(handle)
            playerRepository.resetCurrent()
            _cantodeEngine.value?.clearMedia()
            _pluginEvents.tryEmit(
                PluginEvent.MusicStop(timestamp = System.currentTimeMillis())
            )
        }
    }

    /**
     * Advance to [PlayerRepository.onCompleteMusic] when the current
     * track ends. Replaces the old `PlaybackService.playOnComplete`.
     */
    private fun playOnComplete() {
        val m = playerRepository.onCompleteMusic.value ?: return
        val p = _playlist.value ?: return
        play(m.meta.id, p.abstr.meta.id)
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
