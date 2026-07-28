package com.kutedev.easemusicplayer.singleton

import android.content.Context
import com.kutedev.easemusicplayer.core.CantodeEngine
import com.kutedev.easemusicplayer.core.PlaybackService
import com.kutedev.easemusicplayer.singleton.SleepModeState
import com.kutedev.easemusicplayer.singleton.types.ArgRemoveMusicFromPlaylist
import com.kutedev.easemusicplayer.singleton.types.Music
import com.kutedev.easemusicplayer.singleton.types.Playlist
import com.kutedev.easemusicplayer.singleton.types.MusicId
import com.kutedev.easemusicplayer.singleton.types.PlaylistId
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
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put
import javax.inject.Inject
import javax.inject.Singleton
import kotlin.math.max

/**
 * Transport control surface for the player.
 *
 * Owns the cantode player handle IDs (registered on the Rust side) and
 * the [CantodeEngine] wrapper.
 *
 * Lifecycle:
 * 1. [com.kutedev.easemusicplayer.MainActivity.onCreate] runs `bridge.initialize()`,
 *    which creates the backend handle and starts [KeepBackendService].
 * 2. [com.kutedev.easemusicplayer.MainActivity.onStart] calls [setupCantodeEngine],
 *    which constructs the cantode handles + engine.
 * 3. [PlaybackService] collects [cantodeEngine] and wires its
 *    [android.support.v4.media.session.MediaSessionCompat] when the engine
 *    becomes available.
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

    private val _endedEvent = MutableSharedFlow<Unit>(extraBufferCapacity = 4)
    val endedEvent = _endedEvent.asSharedFlow()

    private val _pluginEvents = MutableSharedFlow<PluginEvent>(extraBufferCapacity = 16)
    val pluginEvents = _pluginEvents.asSharedFlow()

    val sleepState = _sleep.asStateFlow()

    private val _cantodeEngine = MutableStateFlow<CantodeEngine?>(null)
    val cantodeEngine = _cantodeEngine.asStateFlow()

    @Volatile private var playerContextId: Long = -1L
    @Volatile private var playerId: Long = -1L

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
        _scope.launch(Dispatchers.Main) {
            _endedEvent.collect {
                playOnComplete()
            }
        }
    }

    /**
     * Constructs the cantode player context + player + [CantodeEngine].
     *
     * [engineFactory] receives the player handle ID (opaque Long) and
     * returns a [CantodeEngine] wrapping it.
     */
    fun setupCantodeEngine(engineFactory: (Long) -> CantodeEngine) {
        if (setupStarted) return
        setupStarted = true
        _scope.launch(Dispatchers.Main) {
            try {
                // player.contextNew + player.new stay on callRaw — they
                // return raw `{handle: N}` payloads that we extract here.
                val ctxResp = bridge.callRaw("player.contextNew", handle = 0L)
                    .unwrapOrThrow().rawPayloadJson as JsonObject
                val ctxId = ctxResp["handle"]!!.jsonPrimitive.content.toLong()
                bridge.setPlayerContextId(ctxId)
                playerContextId = ctxId

                val playerResp = bridge.callRaw("player.new", handle = ctxId)
                    .unwrapOrThrow().rawPayloadJson as JsonObject
                val pId = playerResp["handle"]!!.jsonPrimitive.content.toLong()
                bridge.setPlayerId(pId)
                playerId = pId

                val engine = engineFactory(pId)
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
                bridge.logRaw("info", "cantode engine setup complete (ctx=$ctxId player=$pId)")
            } catch (e: Exception) {
                setupStarted = false
                bridge.logRaw("error", "cantode engine setup failed: $e")
                _scope.launch { toastRepository.emitToast("player setup failed: $e") }
            }
        }
    }

    /** Current position in ms (for the PlayerVM poll). */
    fun getCurrentPosition(): Long {
        val engine = _cantodeEngine.value ?: return 0L
        return engine.lastPositionMs.toLong()
    }

    fun getBufferedPosition(): Long = getCurrentPosition()

    fun play(id: MusicId, playlistId: PlaylistId) {
        if (playerId < 0) {
            bridge.logRaw("error", "play: cantode player not ready"); return
        }
        val engine = _cantodeEngine.value ?: return

        if (_music.value?.meta?.id == id && _playlist.value?.abstr?.meta?.id == playlistId) {
            resume(); return
        }

        runCatching { PlaybackService.start(cx) }

        _scope.launch(Dispatchers.Main) {
            stop()

            val music: Music? = bridge.call(BridgeMethods.Music.GET, id).unwrapOrNull()?.payload
            val playlist: Playlist? = bridge.call(BridgeMethods.Playlist.GET, playlistId)
                .unwrapOrNull()?.payload
            val inPlaylist = music != null && playlist != null &&
                playlist.musics.any { it.meta.id == id }

            if (inPlaylist) {
                playerRepository.setCurrent(music!!, playlist!!)
                engine.setCurrentMedia(id, music.meta.title)
                // player.loadMusic stays on callRaw — cross-handle arg
                // (backendHandle from Bridge state + musicId).
                bridge.callRaw(
                    "player.loadMusic",
                    buildJsonObject {
                        put("backendHandle", bridge.getBackendId())
                        put("musicId", id.value)
                    },
                    handle = playerId,
                ).unwrapOrNull()
                bridge.call(BridgeMethods.Player.PLAY).unwrapOrNull()
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
        if (playerId < 0) return
        _scope.launch {
            bridge.call(BridgeMethods.Player.PLAY).unwrapOrNull()
        }
    }

    fun pause() {
        if (playerId < 0) return
        _scope.launch {
            bridge.call(BridgeMethods.Player.PAUSE).unwrapOrNull()
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
        if (playerId < 0) return
        _scope.launch {
            bridge.call(BridgeMethods.Player.STOP).unwrapOrNull()
            playerRepository.resetCurrent()
            _cantodeEngine.value?.clearMedia()
            _pluginEvents.tryEmit(
                PluginEvent.MusicStop(timestamp = System.currentTimeMillis())
            )
        }
    }

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
        if (playerId < 0) return
        _scope.launch {
            bridge.call(BridgeMethods.Player.SEEK, ms.toLong()).unwrapOrNull()
        }
    }

    fun scheduleSleep(newExpiredMs: Long) {
        _sleepJob?.cancel()
        val delayMs = max(newExpiredMs - System.currentTimeMillis(), 0)
        _sleepJob = _scope.launch {
            _sleep.update { it.copy(enabled = true, expiredMs = newExpiredMs) }
            bridge.logRaw("info", "schedule sleep")
            delay(delayMs)
            bridge.logRaw("info", "sleep scheduled")
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
                bridge.call(
                    BridgeMethods.Playlist.REMOVE_MUSIC,
                    ArgRemoveMusicFromPlaylist(
                        playlistId = p.abstr.meta.id,
                        musicId = m.meta.id,
                    ),
                ).unwrapOrNull()
            }
        }
    }
}
