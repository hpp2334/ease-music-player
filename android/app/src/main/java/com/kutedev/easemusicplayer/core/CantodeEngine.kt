package com.kutedev.easemusicplayer.core

import com.kutedev.easemusicplayer.singleton.PlayerRepository
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.launch
import uniffi.ease_client_backend.PlayerHandle
import uniffi.ease_client_backend.PlayerStateRecord
import uniffi.ease_client_backend.ctPlayerDurationMs
import uniffi.ease_client_backend.ctPlayerPositionMs
import uniffi.ease_client_backend.ctPlayerState
import uniffi.ease_client_backend.ctPlayerStop
import uniffi.ease_client_schema.MusicId

/**
 * Plain Kotlin wrapper around a cantode [PlayerHandle] (Rust audio engine
 * over UniFFI). Owns the 10 Hz state-poll loop and surfaces cantode state
 * via `@Volatile` fields (safe to read from any thread) plus an
 * [endedEvent] flow.
 *
 * Replaces the old `CantodePlayer extends SimpleBasePlayer` adapter.
 * media3 has been removed entirely; the system media notification and
 * transport callbacks now go through a platform `android.media.session.MediaSession`
 * owned by [PlaybackService], which reads from this class directly.
 *
 * Position updates: cantode emits `PositionChanged` at ~10 Hz internally;
 * we poll `ctPlayerPositionMs` + `ctPlayerState` at the same rate and
 * mirror the result into [PlayerRepository] so ViewModels keep working
 * unchanged.
 */
class CantodeEngine(
    private val playerRepository: PlayerRepository,
    private val handle: PlayerHandle,
    private val scope: CoroutineScope,
) {
    @Volatile var currentMusicId: MusicId? = null
        private set
    @Volatile var currentTitle: String? = null
        private set
    @Volatile var lastState: PlayerStateRecord = PlayerStateRecord.IDLE
        private set
    @Volatile var lastPositionMs: ULong = 0uL
        private set
    @Volatile var lastDurationMs: ULong? = null
        private set

    /**
     * Fires once per track when cantode transitions into `ENDED`.
     * Collected by [com.kutedev.easemusicplayer.singleton.PlayerControllerRepository]
     * to drive auto-advance and emit the `MusicComplete` plugin event.
     */
    private val _endedEvent = MutableSharedFlow<MusicId>(extraBufferCapacity = 4)
    val endedEvent = _endedEvent.asSharedFlow()

    private var pollJob: Job? = null
    @Volatile private var endedHandledForCurrent: Boolean = false
    @Volatile private var released: Boolean = false

    init {
        pollJob = scope.launch(Dispatchers.Default) {
            while (true) {
                if (released) return@launch
                pollOnce()
                delay(POLL_INTERVAL_MS)
            }
        }
    }

    /** Update the music id + title that the next poll cycle will report. */
    fun setCurrentMedia(musicId: MusicId?, title: String?) {
        currentMusicId = musicId
        currentTitle = title
        endedHandledForCurrent = false
    }

    /** Clear the current media marker (used by `stop()`). */
    fun clearMedia() {
        currentMusicId = null
        currentTitle = null
        endedHandledForCurrent = false
    }

    private fun pollOnce() {
        // Sync (non-suspend) UniFFI reads — safe from any thread.
        val newState = ctPlayerState(handle)
        val newPos = ctPlayerPositionMs(handle)
        val newDuration = ctPlayerDurationMs(handle)

        // ENDED detection — fire once per track.
        val mid = currentMusicId
        if (newState == PlayerStateRecord.ENDED
            && lastState != PlayerStateRecord.ENDED
            && !endedHandledForCurrent
            && mid != null
        ) {
            endedHandledForCurrent = true
            scope.launch { _endedEvent.emit(mid) }
        }

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
    }

    fun release() {
        released = true
        pollJob?.cancel()
        scope.launch { ctPlayerStop(handle) }
    }

    companion object {
        /** Cantode emits PositionChanged at 10 Hz; poll at the same rate. */
        private const val POLL_INTERVAL_MS = 100L
    }
}
