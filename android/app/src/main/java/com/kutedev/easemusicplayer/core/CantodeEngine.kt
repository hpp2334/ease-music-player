package com.kutedev.easemusicplayer.core

import com.kutedev.easemusicplayer.singleton.Bridge
import com.kutedev.easemusicplayer.singleton.BridgeMethods
import com.kutedev.easemusicplayer.singleton.PlayerRepository
import com.kutedev.easemusicplayer.singleton.types.MusicId
import com.kutedev.easemusicplayer.singleton.types.PlayerStateRecord
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.launch

/**
 * Plain Kotlin wrapper around the cantode player (Rust audio engine behind
 * the unified JSON bridge). Owns the 10 Hz state-poll loop and surfaces
 * cantode state via `@Volatile` fields (safe to read from any thread) plus
 * an [endedEvent] flow.
 *
 * Position updates: cantode emits `PositionChanged` at ~10 Hz internally;
 * we poll `player.pollState` (one batched JSON call returning state +
 * positionMs + durationMs) at the same rate and mirror the result into
 * [PlayerRepository] so ViewModels keep working unchanged.
 *
 * @param playerHandleId Opaque Long ID returned by the Rust bridge for
 *   this player; passed as the `handle` field on every `player.*` call.
 */
class CantodeEngine(
    private val bridge: Bridge,
    private val playerRepository: PlayerRepository,
    private val playerHandleId: Long,
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

    fun setCurrentMedia(musicId: MusicId?, title: String?) {
        currentMusicId = musicId
        currentTitle = title
        endedHandledForCurrent = false
    }

    fun clearMedia() {
        currentMusicId = null
        currentTitle = null
        endedHandledForCurrent = false
    }

    private suspend fun pollOnce() {
        // Single batched call: state + positionMs + durationMs.
        val poll = bridge.call(BridgeMethods.Player.POLL_STATE).unwrapOrNull()?.payload ?: return

        val newState = poll.state
        val newPos = poll.positionMs
        val newDuration = poll.durationMs

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
        scope.launch {
            bridge.call(BridgeMethods.Player.STOP).unwrapOrNull()
        }
    }

    companion object {
        /** Cantode emits PositionChanged at 10 Hz; poll at the same rate. */
        private const val POLL_INTERVAL_MS = 100L
    }
}
