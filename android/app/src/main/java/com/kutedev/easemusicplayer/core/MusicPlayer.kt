package com.kutedev.easemusicplayer.core

import androidx.media3.common.MediaItem
import androidx.media3.exoplayer.ExoPlayer
import uniffi.ease_client.IMusicPlayerService

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

    override fun getCurrentDurationS(): ULong {
        val player = getInternal()
        return player.currentPosition.toULong()
    }
}