package com.kutedev.easemusicplayer.core

import androidx.media3.common.MediaItem
import androidx.media3.common.Player
import androidx.media3.exoplayer.ExoPlayer
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOn
import uniffi.ease_client.IMusicPlayerService
import uniffi.ease_client.updateCurrentMusicPlayingForPlayerInternal
import kotlin.time.Duration
import kotlin.time.Duration.Companion.seconds
import kotlin.time.DurationUnit
import kotlin.time.toDuration

fun Player.currentPositionFlow(
    updateFrequency: Duration = 1.seconds,
) = flow {
    while (true) {
        if (isPlaying) emit(Unit)
        delay(updateFrequency)
    }
}.flowOn(Dispatchers.Main)

class MusicPlayer : IMusicPlayerService {
    private var _internal: ExoPlayer? = null

    fun getInternal(): ExoPlayer {
        if (_internal == null) {
            throw RuntimeException("player is null")
        }
        return _internal!!
    }

    fun install(context: android.content.Context) {
        _internal = ExoPlayer.Builder(context).build()
    }

    override fun resume() {
        val player = getInternal()
        player.play()
    }

    override fun pause() {
        val player = getInternal()
        player.pause()
    }

    override fun stop() {
        val player = getInternal()
        player.stop()
    }

    override fun seek(arg: ULong) {
        val player = getInternal()
        player.seekTo(arg.toLong())
    }

    override fun setMusicUrl(url: String) {
        val player = getInternal()
        val mediaItem = MediaItem.fromUri(url)
        player.setMediaItem(mediaItem)
        player.prepare()
    }
}