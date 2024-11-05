package com.kutedev.easemusicplayer.core

import android.media.AudioAttributes
import android.media.MediaMetadataRetriever
import android.media.MediaPlayer
import android.net.Uri
import com.kutedev.easemusicplayer.utils.nextTickOnMain
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import uniffi.ease_client.IMusicPlayerService
import uniffi.ease_client.PlayerEvent
import uniffi.ease_client.ViewAction
import uniffi.ease_client_shared.MusicId

class MusicPlayer : IMusicPlayerService {
    private var _internal: MediaPlayer = MediaPlayer()
    private var _context: android.content.Context? = null
    val customScope = CoroutineScope(Dispatchers.IO)

    fun getInternal(): MediaPlayer {
        return _internal
    }

    fun install(context: android.content.Context) {
        _context = context
    }

    fun destroy() {
        _internal.release()
        _context = null
    }

    override fun resume() {
        val player = getInternal()
        player.start()

        syncPlayingState()
    }

    override fun pause() {
        val player = getInternal()
        player.setOnPreparedListener {}
        player.pause()

        nextTickOnMain {
            Bridge.dispatchAction(ViewAction.Player(PlayerEvent.Pause));
        }
    }

    override fun stop() {
        val player = getInternal()
        player.setOnPreparedListener {}
        player.setOnSeekCompleteListener {}
        player.stop()

        nextTickOnMain {
            Bridge.dispatchAction(ViewAction.Player(PlayerEvent.Stop));
        }
    }

    override fun seek(arg: ULong) {
        val player = getInternal()
        player.seekTo(arg.toInt())

        syncPlayingState()
    }

    override fun setMusicUrl(id: MusicId, url: String) {
        val player = getInternal()
        player.reset()
        player.setAudioAttributes(
            AudioAttributes.Builder()
            .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC)
            .setUsage(AudioAttributes.USAGE_MEDIA)
            .build()
        )
        player.setDataSource(_context!!, Uri.parse(url))
        player.setOnCompletionListener {}
        player.setOnPreparedListener {
            player.setOnCompletionListener {
                player.setOnCompletionListener {}
                android.os.Handler(android.os.Looper.getMainLooper()).post {
                    Bridge.dispatchAction(ViewAction.Player(PlayerEvent.Complete))
                }
            }
            this.resume()
            syncPlayingState()
        }
        player.prepareAsync()

        requestTotalDuration(id, url)
        syncPlayingState()
    }

    override fun getCurrentDurationS(): ULong {
        val player = getInternal()

        return (player.currentPosition / 1000).toULong();
    }

    private fun syncPlayingState() {
        val player = getInternal()

        nextTickOnMain {
            val isPlaying = player.isPlaying

            if (isPlaying) {
                Bridge.dispatchAction(ViewAction.Player(PlayerEvent.Play));
            }
        }
    }

    private fun requestTotalDuration(id: MusicId, url: String) {
        customScope.launch {
            val retriever = MediaMetadataRetriever()
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
        }
    }
}