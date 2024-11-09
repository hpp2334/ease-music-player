package com.kutedev.easemusicplayer.core

import android.media.MediaMetadataRetriever
import androidx.core.text.isDigitsOnly
import androidx.media3.common.C.TIME_UNSET
import androidx.media3.common.MediaItem
import androidx.media3.common.Player
import androidx.media3.exoplayer.ExoPlayer
import com.kutedev.easemusicplayer.utils.nextTickOnMain
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import uniffi.ease_client.IMusicPlayerService
import uniffi.ease_client.PlayerEvent
import uniffi.ease_client.ViewAction
import uniffi.ease_client_shared.MusicId

class MusicPlayer : IMusicPlayerService {

    private var _internal: ExoPlayer? = null
    val customScope = CoroutineScope(Dispatchers.IO)

    fun onActivityCreate(context: android.content.Context) {
        val player = ExoPlayer.Builder(context).build()
        _internal = player

        player.addListener(object : Player.Listener {
            override fun onIsPlayingChanged(isPlaying: Boolean) {
                nextTickOnMain {
                    if (isPlaying) {
                        Bridge.dispatchAction(ViewAction.Player(PlayerEvent.Play));
                    }
                }
            }

            override fun onPlaybackStateChanged(playbackState: Int) {
                if (playbackState == Player.STATE_ENDED) {
                    nextTickOnMain {
                        Bridge.dispatchAction(ViewAction.Player(PlayerEvent.Complete))
                    }
                } else if (playbackState == Player.STATE_READY) {
                    syncTotalDuration()
                }
            }
        })
    }

    fun onActivityStart() {
    }

    fun onActivityStop() {
    }

    fun onActivityDestroy() {
        _internal?.release()
        _internal = null
    }

    private fun syncTotalDuration() {
        val player = _internal ?: return
        if (!player.isCommandAvailable(Player.COMMAND_GET_CURRENT_MEDIA_ITEM)) {
            return
        }
        if (player.duration == TIME_UNSET) {
            return
        }

        val mediaItem = player.currentMediaItem
        if (mediaItem != null && mediaItem.mediaId.isDigitsOnly()) {
            val id = MusicId(mediaItem.mediaId.toLong())
            val durationMS = player.duration.toULong()

            nextTickOnMain {
                Bridge.dispatchAction(ViewAction.Player(PlayerEvent.Total(id, durationMS)))
            }
        }
    }

    override fun resume() {
        val player = _internal ?: return

        player.play()
    }

    override fun pause() {
        val player = _internal ?: return
        player.pause()

        nextTickOnMain {
            Bridge.dispatchAction(ViewAction.Player(PlayerEvent.Pause));
        }
    }

    override fun stop() {
        val player = _internal ?: return
        player.stop()

        nextTickOnMain {
            Bridge.dispatchAction(ViewAction.Player(PlayerEvent.Stop));
        }
    }

    override fun seek(msec: ULong) {
        val player = _internal ?: return
        println("seekTo ${msec.toLong()}")
        player.seekTo(msec.toLong())
    }

    override fun setMusicUrl(id: MusicId, url: String) {
        val player = _internal ?: return

        val mediaItem = MediaItem.Builder().setMediaId(id.value.toString()).setUri(url).build()
        player.stop()
        player.setMediaItem(mediaItem)
        player.prepare()
        player.play()
    }

    override fun getCurrentDurationS(): ULong {
        val player = _internal ?: return 0uL

        return (player.currentPosition / 1000).toULong();
    }

    override fun requestTotalDuration(id: MusicId, url: String) {
        customScope.launch {
            val retriever = MediaMetadataRetriever()
            try {
                retriever.setDataSource(url)

                // Get the duration in milliseconds
                val duration = retriever.extractMetadata(MediaMetadataRetriever.METADATA_KEY_DURATION)

                if (duration != null) {
                    val durationMS = duration.toULong()

                    nextTickOnMain {
                        Bridge.dispatchAction(ViewAction.Player(PlayerEvent.Total(id, durationMS)))
                    }
                }
                retriever.release()
            } catch (_: Exception) {

            }
            retriever.release()
        }
    }
}