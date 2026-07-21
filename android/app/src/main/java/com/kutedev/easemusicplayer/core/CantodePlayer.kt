package com.kutedev.easemusicplayer.core

import android.os.Handler
import android.os.Looper
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.Player
import androidx.media3.common.SimpleBasePlayer
import androidx.media3.common.util.UnstableApi
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture
import com.kutedev.easemusicplayer.singleton.PlayerRepository
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import uniffi.ease_client_backend.PlayerHandle
import uniffi.ease_client_backend.PlayerStateRecord
import uniffi.ease_client_backend.ctPlayerDurationMs
import uniffi.ease_client_backend.ctPlayerPause
import uniffi.ease_client_backend.ctPlayerPlay
import uniffi.ease_client_backend.ctPlayerPositionMs
import uniffi.ease_client_backend.ctPlayerSeek
import uniffi.ease_client_backend.ctPlayerState
import uniffi.ease_client_backend.ctPlayerStop
import uniffi.ease_client_schema.MusicId

/**
 * A [SimpleBasePlayer] backed by a cantode [PlayerHandle] (Rust audio
 * engine over UniFFI).
 *
 * Replaces the old ExoPlayer + MusicPlayerDataSource pipeline. Cantode
 * owns the decoder (symphonia), the output device (cpal → AAudio), and
 * the worker thread; this class is a thin adapter that:
 *
 * - Reports cantode state to the media3 [androidx.media3.session.MediaSession]
 *   via [getState] + [invalidateState], so notification / lock-screen /
 *   Bluetooth controls keep working exactly as before.
 * - Forwards transport commands (play/pause/seek/stop) issued by system
 *   controllers back into cantode.
 * - Bridges cantode state changes into [PlayerRepository] flows (the
 *   existing engine-agnostic state surface that ViewModels consume).
 *
 * Position updates: cantode emits `PositionChanged` at ~10 Hz internally;
 * we poll `ctPlayerPositionMs` + `ctPlayerState` at the same rate and call
 * [invalidateState] whenever they change. This keeps media3's position
 * display smooth without needing the EventSink callback interface (which
 * is reserved for a follow-up).
 */
@OptIn(UnstableApi::class)
class CantodePlayer(
    private val playerRepository: PlayerRepository,
    private val handle: PlayerHandle,
    private val scope: CoroutineScope,
) : SimpleBasePlayer(Looper.getMainLooper()) {

    @Volatile private var currentMusicId: MusicId? = null
    @Volatile private var currentTitle: String? = null

    @Volatile private var lastState: PlayerStateRecord = PlayerStateRecord.IDLE
    @Volatile private var lastPositionMs: ULong = 0u
    @Volatile private var lastDurationMs: ULong? = null
    @Volatile private var endedHandledForCurrent: Boolean = false

    /**
     * [SimpleBasePlayer.invalidateState] enforces application-thread affinity
     * (the main looper). Several callers (the poll loop on Dispatchers.Default,
     * repository methods on Dispatchers.Main or Default) need to trigger a
     * state invalidation, so we funnel them through this [Handler] which is
     * bound to the main looper.
     */
    private val mainHandler = Handler(Looper.getMainLooper())

    /**
     * Thread-safe wrapper around [invalidateState]. Safe to call from any
     * thread; bounces to the main looper.
     */
    private fun invalidateStateOnMain() {
        if (Looper.myLooper() === Looper.getMainLooper()) {
            invalidateState()
        } else {
            mainHandler.post { invalidateState() }
        }
    }

    private var pollJob: Job? = null
    private var released: Boolean = false

    init {
        // Match the old ExoPlayer.Builder audio attributes:
        // USAGE_MEDIA + AUDIO_CONTENT_TYPE_MUSIC + handle audio focus.
        // SimpleBasePlayer handles audio-becoming-noisy too.
        setAudioAttributes(
            AudioAttributes.Builder()
                .setUsage(C.USAGE_MEDIA)
                .setContentType(C.AUDIO_CONTENT_TYPE_MUSIC)
                .build(),
            /* handleAudioFocus = */ true,
        )

        pollJob = scope.launch(Dispatchers.Default) {
            while (true) {
                if (released) return@launch
                pollOnce()
                delay(POLL_INTERVAL_MS)
            }
        }
    }

    /** Update the music id + title that [getState] reports to media3. */
    fun setCurrentMedia(musicId: MusicId?, title: String?) {
        currentMusicId = musicId
        currentTitle = title
        endedHandledForCurrent = false
        invalidateStateOnMain()
    }

    private fun pollOnce() {
        // Sync (non-suspend) UniFFI reads — safe from any thread.
        val newState = ctPlayerState(handle)
        val newPos = ctPlayerPositionMs(handle)
        val newDuration = ctPlayerDurationMs(handle)

        val stateChanged = newState != lastState
        val posChanged = newPos != lastPositionMs
        val durChanged = newDuration != lastDurationMs

        lastState = newState
        lastPositionMs = newPos
        lastDurationMs = newDuration

        // Mirror into PlayerRepository so ViewModels keep working unchanged.
        when (newState) {
            PlayerStateRecord.PLAYING -> {
                playerRepository.setIsPlaying(true)
                playerRepository.setIsLoading(false)
            }
            PlayerStateRecord.PAUSED -> {
                playerRepository.setIsPlaying(false)
                playerRepository.setIsLoading(false)
            }
            PlayerStateRecord.LOADING, PlayerStateRecord.BUFFERING -> {
                playerRepository.setIsLoading(true)
            }
            PlayerStateRecord.ENDED, PlayerStateRecord.ERROR, PlayerStateRecord.IDLE -> {
                playerRepository.setIsPlaying(false)
                playerRepository.setIsLoading(false)
            }
        }

        if (stateChanged || posChanged || durChanged) {
            invalidateStateOnMain()
        }
    }

    // ----- SimpleBasePlayer contract -----

    override fun getState(): State {
        val state = lastState
        val posMs = lastPositionMs.toLong()
        val durationMs = lastDurationMs?.takeIf { it > 0uL }?.toLong()
        val hasMedia = currentMusicId != null

        val playbackState = when (state) {
            PlayerStateRecord.PLAYING, PlayerStateRecord.PAUSED -> Player.STATE_READY
            PlayerStateRecord.LOADING, PlayerStateRecord.BUFFERING -> Player.STATE_BUFFERING
            PlayerStateRecord.ENDED -> Player.STATE_ENDED
            PlayerStateRecord.IDLE, PlayerStateRecord.ERROR -> Player.STATE_IDLE
        }
        val playWhenReady = state == PlayerStateRecord.PLAYING

        val builder = State.Builder()
            .setAvailableCommands(buildAvailableCommands(hasMedia))
            .setPlayWhenReady(playWhenReady, Player.PLAY_WHEN_READY_CHANGE_REASON_USER_REQUEST)
            .setPlaybackState(playbackState)
            .setContentPositionMs(posMs)
            .setCurrentMediaItemIndex(if (hasMedia) 0 else C.INDEX_UNSET)

        if (hasMedia) {
            val mediaItem = MediaItem.Builder()
                .setMediaId(currentMusicId!!.value.toString())
                .build()
            val meta = MediaMetadata.Builder()
                .setTitle(currentTitle ?: "")
                .build()
            val mediaItemData = SimpleBasePlayer.MediaItemData.Builder(ANY_UID)
                .setMediaItem(mediaItem)
                .setMediaMetadata(meta)
                .also { b ->
                    if (durationMs != null) {
                        b.setDurationUs(durationMs * 1000L)
                    }
                }
                .build()
            builder.setPlaylist(listOf(mediaItemData))
        }
        return builder.build()
    }

    private fun buildAvailableCommands(hasMedia: Boolean): Player.Commands {
        val builder = Player.Commands.Builder().addAll(
            Player.COMMAND_PLAY_PAUSE,
            Player.COMMAND_GET_CURRENT_MEDIA_ITEM,
            Player.COMMAND_GET_METADATA,
            Player.COMMAND_GET_TIMELINE,
            Player.COMMAND_GET_TRACKS,
            Player.COMMAND_SET_VOLUME,
        )
        if (hasMedia) {
            builder.add(Player.COMMAND_SEEK_IN_CURRENT_MEDIA_ITEM)
            builder.add(Player.COMMAND_STOP)
        }
        return builder.build()
    }

    override fun handleSetPlayWhenReady(playWhenReady: Boolean): ListenableFuture<*> {
        scope.launch {
            if (playWhenReady) ctPlayerPlay(handle) else ctPlayerPause(handle)
        }
        return Futures.immediateVoidFuture()
    }

    override fun handleStop(): ListenableFuture<*> {
        scope.launch {
            ctPlayerStop(handle)
            endedHandledForCurrent = false
        }
        return Futures.immediateVoidFuture()
    }

    override fun handleSeek(
        mediaItemIndex: Int,
        positionMs: Long,
        @Player.Command seekCommand: Int,
    ): ListenableFuture<*> {
        scope.launch {
            ctPlayerSeek(handle, positionMs.toULong())
            endedHandledForCurrent = false
        }
        return Futures.immediateVoidFuture()
    }

    override fun handleRelease(): ListenableFuture<*> {
        released = true
        pollJob?.cancel()
        scope.launch { ctPlayerStop(handle) }
        return Futures.immediateVoidFuture()
    }

    companion object {
        /** Cantode emits PositionChanged at 10 Hz; poll at the same rate. */
        private const val POLL_INTERVAL_MS = 100L

        /** UID for the single-item playlist we report to media3. */
        private val ANY_UID: Any = "cantode-current"
    }
}
