package com.kutedev.easemusicplayer.singleton

import com.kutedev.easemusicplayer.singleton.types.MusicId
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/**
 * Events emitted by [PlayerControllerRepository] that plugins can subscribe
 * to via their `manifest.json` `events` field.
 *
 * Subscriptions are matched by the [type] string — e.g. a plugin that
 * declares `"events": ["music:play"]` receives only [MusicPlay] events.
 * The host forwards each event to the subscribing plugin's backend JS
 * module over the `tur:rpc` channel (`registerHandler(event.type, …)`);
 * [toJsonElement] builds the payload the backend sees.
 *
 * Timestamps are wall-clock milliseconds (System.currentTimeMillis()).
 */
sealed class PluginEvent {
    abstract val type: String
    abstract val timestamp: Long

    /** Fired after `ctPlayerPlay(handle)` succeeds for a freshly loaded track. */
    data class MusicPlay(
        val musicId: MusicId,
        val title: String,
        override val timestamp: Long,
    ) : PluginEvent() {
        override val type: String = MUSIC_PLAY
    }

    /** Fired when the user (or sleep timer) pauses playback. */
    data class MusicPause(
        val musicId: MusicId?,
        override val timestamp: Long,
        val positionMs: Long,
    ) : PluginEvent() {
        override val type: String = MUSIC_PAUSE
    }

    /** Fired when playback is stopped (current music cleared). */
    data class MusicStop(
        override val timestamp: Long,
    ) : PluginEvent() {
        override val type: String = MUSIC_STOP
    }

    /** Fired when the current track finishes naturally (STATE_ENDED). */
    data class MusicComplete(
        val musicId: MusicId,
        val title: String,
        override val timestamp: Long,
    ) : PluginEvent() {
        override val type: String = MUSIC_COMPLETE
    }

    /** The payload a JS backend receives as the handler arg. */
    fun toJsonElement(): JsonElement = when (this) {
        is MusicPlay -> buildJsonObject {
            put("musicId", musicId.value)
            put("title", title)
            put("ts", timestamp)
        }
        is MusicPause -> buildJsonObject {
            musicId?.let { put("musicId", it.value) }
            put("ts", timestamp)
            put("positionMs", positionMs)
        }
        is MusicStop -> buildJsonObject {
            put("ts", timestamp)
        }
        is MusicComplete -> buildJsonObject {
            put("musicId", musicId.value)
            put("title", title)
            put("ts", timestamp)
        }
    }

    companion object {
        const val MUSIC_PLAY = "music:play"
        const val MUSIC_PAUSE = "music:pause"
        const val MUSIC_STOP = "music:stop"
        const val MUSIC_COMPLETE = "music:complete"
    }
}
