package com.kutedev.cantode

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json

/**
 * The engine facade: owns the 10 Hz poll loop over [CantodeNative], the
 * transition fold (sub-tick excursions are replayed, never missed) and
 * the transport commands. Knows nothing about any embedder's business
 * logic — no music ids, no repositories, no plugin events; consumers map
 * [state]/[loading]/[ended] onto their own world.
 *
 * @param playerHandle the embedder's opaque id for the player (on the
 *   Android app: the bridge handle id from `player.new`).
 * @param scope lifecycle owner for the poll loop; cancelling it (or
 *   [release]) stops polling.
 */
class Cantode(
    private val playerHandle: Long,
    private val scope: CoroutineScope,
) {
    private val _state = MutableStateFlow(PlayerState.IDLE)
    /** Current engine state — engine truth, folded from the transition history. */
    val state: StateFlow<PlayerState> = _state.asStateFlow()

    private val _loading = MutableStateFlow(false)
    /**
     * `true` while the engine is loading/buffering **or** for one poll
     * tick after such an excursion completed between two polls (the
     * one-tick visibility hold): fast sources still show loading
     * feedback. Derived purely from engine transitions.
     */
    val loading: StateFlow<Boolean> = _loading.asStateFlow()

    private val _positionMs = MutableStateFlow(0L)
    /** Current playback position in milliseconds. */
    val positionMs: StateFlow<Long> = _positionMs.asStateFlow()

    private val _durationMs = MutableStateFlow<Long?>(null)
    /** Duration of the loaded source in milliseconds, `null` until known. */
    val durationMs: StateFlow<Long?> = _durationMs.asStateFlow()

    private val _bufferedMs = MutableStateFlow<Long?>(null)
    /**
     * Buffered frontier in milliseconds (media time) — how far ahead of
     * the read cursor contiguous data is buffered. `null` when the
     * engine can't compute it: non-buffering sources (local files) or
     * unknown total length / duration. Linear byte→time approximation;
     * see the engine's `Player::buffered_position`.
     */
    val bufferedMs: StateFlow<Long?> = _bufferedMs.asStateFlow()

    private val _ended = MutableSharedFlow<Unit>(extraBufferCapacity = 4)
    /**
     * Fired once per completed playthrough (one emission per `ENDED`
     * entry in the engine's transition history — a seek-back replay that
     * reaches the end again fires again). No music id attached: consumers
     * stamp their own current music.
     */
    val ended: SharedFlow<Unit> = _ended.asSharedFlow()

    @Volatile
    var isReleased = false
        private set

    /** Last drained transition seq (the engine's monotonic counter). */
    @Volatile
    private var lastTransitionSeq = 0L

    private val json = Json { ignoreUnknownKeys = true }

    private var pollJob: Job? = null

    init {
        pollJob = scope.launch(Dispatchers.Default) {
            while (!isReleased) {
                pollOnce()
                delay(POLL_INTERVAL_MS)
            }
        }
    }

    // ----- transport -----

    /** Begin or resume playback. */
    fun play() {
        if (!isReleased) CantodeNative.play(playerHandle)
    }

    /** Pause playback. */
    fun pause() {
        if (!isReleased) CantodeNative.pause(playerHandle)
    }

    /** Stop and drop the loaded source (back to `Idle`). */
    fun stop() {
        if (!isReleased) CantodeNative.stop(playerHandle)
    }

    /** Seek to [ms] from source start (no-op without a loaded source). */
    fun seek(ms: Long) {
        if (!isReleased) CantodeNative.seek(playerHandle, ms)
    }

    /** Set linear gain: `1.0` unity, `0.0` silent. */
    fun setVolume(volume: Float) {
        if (!isReleased) CantodeNative.setVolume(playerHandle, volume)
    }

    /**
     * Load a pre-registered source straight into `Playing`. Blocks until
     * the source is open (network-bound). `false` = no player / token
     * already consumed / load failed (the next poll reports `ERROR`).
     */
    suspend fun loadAndPlay(sourceToken: Long): Boolean =
        withContext(Dispatchers.IO) {
            CantodeNative.loadAndPlay(playerHandle, sourceToken)
        }

    /** Stop polling. Safe to call more than once. */
    fun release() {
        isReleased = true
        pollJob?.cancel()
    }

    // ----- poll loop -----

    private suspend fun pollOnce() {
        val raw = CantodeNative.poll(playerHandle, lastTransitionSeq)
        if (raw.isEmpty()) return // no player under this handle — skip
        val poll = json.decodeFromString(FfiPollSnapshot.serializer(), raw)

        // Replay the drained transitions in order — a sub-tick excursion
        // (fast `Loading → Playing` between two polls) is applied exactly
        // as if it had been observed live. If the history overran (first
        // entry's seq isn't lastTransitionSeq + 1), it is partial: trust
        // only the current state.
        val entries = poll.transitions
        val contiguous = entries.isEmpty() ||
            entries.first().seq == lastTransitionSeq + 1
        lastTransitionSeq = poll.stateSeq

        if (contiguous) {
            for (entry in entries) {
                applyObserved(entry.state, poll.positionMs, poll.durationMs)
            }
        }
        // The current state is the freshest read — normally identical to
        // the last drained transition; if one landed between the reads,
        // the next poll drains it.
        applyObserved(poll.state, poll.positionMs, poll.durationMs)

        // The buffered frontier is a plain observable, not a
        // transition-derived value: apply it once per poll.
        _bufferedMs.value = poll.bufferedMs

        // One-tick visibility hold: surface a Loading/Buffering excursion
        // that already completed between polls for this single tick.
        if (contiguous && poll.state == PlayerState.PLAYING) {
            val loadishExcursion = entries.any {
                it.state == PlayerState.LOADING || it.state == PlayerState.BUFFERING
            }
            if (loadishExcursion) {
                _loading.value = true
            }
        }
    }

    private fun applyObserved(newState: PlayerState, positionMs: Long, durationMs: Long?) {
        if (newState == PlayerState.ENDED && _state.value != PlayerState.ENDED) {
            // One emission per ENDED entry (transitions only record real
            // changes, so the folded replay never double-fires).
            _ended.tryEmit(Unit)
        }
        _state.value = newState
        _positionMs.value = positionMs
        _durationMs.value = durationMs
        _loading.value = newState == PlayerState.LOADING || newState == PlayerState.BUFFERING
    }

    companion object {
        /** Matches the engine's ~10 Hz observable cadence. */
        private const val POLL_INTERVAL_MS = 100L
    }
}
