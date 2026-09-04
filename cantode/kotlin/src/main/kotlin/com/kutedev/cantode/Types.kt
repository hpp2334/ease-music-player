package com.kutedev.cantode

import kotlinx.serialization.Serializable

/**
 * The externally-observable engine state (mirrors cantode's
 * `PlayerState`; wire values are `SCREAMING_SNAKE_CASE`).
 */
@Serializable
enum class PlayerState {
    /** No source loaded, no output stream open. */
    IDLE,

    /** A source is being opened / decoded for the first time. */
    LOADING,

    /** Source loaded, output stream open but paused. */
    PAUSED,

    /** Audio is flowing to the output device. */
    PLAYING,

    /** Decode pipeline waiting on the source (network stall, slow disk). */
    BUFFERING,

    /** Loaded source reached its end. Seeking back restarts. */
    ENDED,

    /** Unrecoverable error; wedged until a fresh load or stop. */
    ERROR,
}

/** One entry of the engine's transition history. */
@Serializable
internal data class FfiTransition(
    val seq: Long,
    val state: PlayerState,
)

/** Wire shape of [CantodeNative.poll]. */
@Serializable
internal data class FfiPollSnapshot(
    val state: PlayerState,
    val stateSeq: Long = 0,
    val transitions: List<FfiTransition> = emptyList(),
    val positionMs: Long = 0,
    val durationMs: Long? = null,
    /** Buffered frontier in media time; `null` for non-buffering
     *  sources or while total length / duration is unknown. */
    val bufferedMs: Long? = null,
)
