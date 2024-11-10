package com.kutedev.easemusicplayer.core

import android.content.ComponentName
import android.content.Intent
import android.media.MediaMetadataRetriever
import androidx.core.text.isDigitsOnly
import androidx.media3.common.C.TIME_UNSET
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.session.MediaController
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import androidx.media3.session.SessionToken
import com.google.common.util.concurrent.MoreExecutors
import com.kutedev.easemusicplayer.utils.nextTickOnMain
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Semaphore
import kotlinx.coroutines.sync.withPermit
import uniffi.ease_client.IMusicPlayerService
import uniffi.ease_client.MusicToPlay
import uniffi.ease_client.PlayerEvent
import uniffi.ease_client.ViewAction
import uniffi.ease_client_shared.MusicId


private fun syncTotalDurationImpl(player: Player) {
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

class PlaybackService : MediaSessionService() {
    private var _mediaSession: MediaSession? = null

    // Create your player and media session in the onCreate lifecycle event
    override fun onCreate() {
        super.onCreate()

        val player = ExoPlayer.Builder(this).build()
        _mediaSession = MediaSession.Builder(this, player)
            .build()

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

    override fun onTaskRemoved(rootIntent: Intent?) {
        val player = _mediaSession?.player ?: return

        if (!player.playWhenReady || player.mediaItemCount == 0) {
            stopSelf()
        }
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? {
        return _mediaSession
    }

    override fun onDestroy() {
        _mediaSession?.run {
            player.release()
            release()
            _mediaSession = null
        }
        super.onDestroy()
    }

    private fun syncTotalDuration() {
        val player = _mediaSession?.player ?: return
        syncTotalDurationImpl(player)
    }
}

class EaseMusicController : IMusicPlayerService {
    private var _context: android.content.Context? = null
    private var _internal: MediaController? = null
    private val _requestSemaphore = Semaphore(4)
    val customScope = CoroutineScope(Dispatchers.Main)

    fun onActivityCreate(context: android.content.Context) {
        _context = context

        val factory = MediaController.Builder(
            context,
            SessionToken(context, ComponentName(context, PlaybackService::class.java))
        ).buildAsync()
        factory.addListener(
            {
                // MediaController is available here with controllerFuture.get()
                _internal = factory.let {
                    if (it.isDone)
                        it.get()
                    else
                        null
                }
            },
            MoreExecutors.directExecutor()
        )
    }

    fun onActivityStart() {
    }

    fun onActivityStop() {
    }

    fun onActivityDestroy() {
        _internal = null
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

    override fun setMusicUrl(item: MusicToPlay) {
        val player = _internal ?: return

        val mediaItem = MediaItem.Builder()
            .setMediaId(item.id.value.toString())
            .setUri(item.url)
            .setMediaMetadata(
                MediaMetadata.Builder()
                    .setTitle(item.title)
                    .build()
            )
            .build()
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
        val context = _context ?: return

        customScope.launch {
            _requestSemaphore.acquire()
            try {
                val player = ExoPlayer.Builder(context).build()

                player.addListener(object : Player.Listener {
                    override fun onPlaybackStateChanged(playbackState: Int) {
                        if (playbackState == Player.STATE_READY) {
                            syncTotalDurationImpl(player)
                            player.release()
                            _requestSemaphore.release()
                        }
                    }

                    override fun onPlayerError(error: PlaybackException) {
                        player.release()
                        _requestSemaphore.release()
                    }
                })
                val mediaItem = MediaItem.Builder()
                    .setMediaId(id.value.toString())
                    .setUri(url)
                    .build()
                player.setMediaItem(mediaItem)
                player.prepare()
            } catch (e: Exception) {
                println(e)
            }
        }
    }
}