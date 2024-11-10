package com.kutedev.easemusicplayer.core

import android.content.ComponentName
import android.content.Intent
import android.os.Handler
import androidx.annotation.OptIn
import androidx.core.text.isDigitsOnly
import androidx.media3.common.C.TIME_UNSET
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.Player.COMMAND_SEEK_TO_NEXT
import androidx.media3.common.Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM
import androidx.media3.common.Player.COMMAND_SEEK_TO_PREVIOUS
import androidx.media3.common.Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.TransferListener
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.analytics.PlayerId
import androidx.media3.exoplayer.drm.DrmSessionEventListener
import androidx.media3.exoplayer.drm.DrmSessionManagerProvider
import androidx.media3.exoplayer.source.MediaPeriod
import androidx.media3.exoplayer.source.MediaSource
import androidx.media3.exoplayer.source.MediaSourceEventListener
import androidx.media3.exoplayer.upstream.Allocator
import androidx.media3.exoplayer.upstream.LoadErrorHandlingPolicy
import androidx.media3.extractor.metadata.flac.PictureFrame
import androidx.media3.extractor.metadata.id3.ApicFrame
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
import uniffi.ease_client.IMusicPlayerService
import uniffi.ease_client.MusicToPlay
import uniffi.ease_client.PlayerEvent
import uniffi.ease_client.ViewAction
import uniffi.ease_client_shared.MusicId

@OptIn(UnstableApi::class)
private fun extractCurrentTracksCover(player: Player): ByteArray? {
    player.currentTracks.groups.forEach { trackGroup ->
        (0 until trackGroup.length).forEach { i ->
            val format = trackGroup.getTrackFormat(i)
            val metadata = format.metadata
            if (metadata != null) {
                (0 until metadata.length()).forEach { j ->
                    val entry = metadata.get(j)
                    if (entry is ApicFrame) {
                        // ID3
                        return entry.pictureData
                    } else if (entry is PictureFrame) {
                        // FLAC
                        return entry.pictureData
                    }
                }
            }
        }
    }
    return null
}

private fun syncMetadataImpl(player: Player) {
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
        val coverData = extractCurrentTracksCover(player)

        nextTickOnMain {
            Bridge.dispatchAction(ViewAction.Player(PlayerEvent.Total(id, durationMS)))

            if (coverData != null) {
                Bridge.dispatchAction(ViewAction.Player(PlayerEvent.Cover(id, coverData)))
            }
        }
    }
}

class PlaybackService : MediaSessionService() {
    private var _mediaSession: MediaSession? = null

    override fun onCreate() {
        super.onCreate()

        val player = ExoPlayer.Builder(this).build()
        _mediaSession = MediaSession.Builder(this, player)
            .setCallback(object : MediaSession.Callback {
                @OptIn(UnstableApi::class)
                override fun onConnect(
                    session: MediaSession,
                    controller: MediaSession.ControllerInfo
                ): MediaSession.ConnectionResult {
                    if (session.isMediaNotificationController(controller)) {
                        val sessionCommands =
                            MediaSession.ConnectionResult.DEFAULT_SESSION_COMMANDS.buildUpon()
//                                .add(customCommandSeekBackward)
//                                .add(customCommandSeekForward)
                                .build()
                        val playerCommands =
                            MediaSession.ConnectionResult.DEFAULT_PLAYER_COMMANDS.buildUpon()
                                .remove(COMMAND_SEEK_TO_PREVIOUS)
                                .remove(COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM)
                                .remove(COMMAND_SEEK_TO_NEXT)
                                .remove(COMMAND_SEEK_TO_NEXT_MEDIA_ITEM)
                                .build()
                        // Custom layout and available commands to configure the legacy/framework session.
                        return MediaSession.ConnectionResult.AcceptedResultBuilder(session)
//                            .setCustomLayout(
//                                ImmutableList.of(
//                                    createSeekBackwardButton(customCommandSeekBackward),
//                                    createSeekForwardButton(customCommandSeekForward))
//                            )
                            .setAvailablePlayerCommands(playerCommands)
                            .setAvailableSessionCommands(sessionCommands)
                            .build()
                    }
                    return MediaSession.ConnectionResult.AcceptedResultBuilder(session).build()
                }
            })
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
                    syncMetadata()
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

    private fun syncMetadata() {
        val player = _mediaSession?.player ?: return
        syncMetadataImpl(player)
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
        val player = _internal

        if (player == null) {
            nextTickOnMain {
                Bridge.dispatchAction(ViewAction.Player(PlayerEvent.Stop));
            }
            return
        }

        val isPlaying = player.isPlaying
        nextTickOnMain {
            if (isPlaying) {
                Bridge.dispatchAction(ViewAction.Player(PlayerEvent.Play));
            } else {
                Bridge.dispatchAction(ViewAction.Player(PlayerEvent.Pause));
            }
        }
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
                            syncMetadataImpl(player)
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
            } catch (_: Exception) {
            }
        }
    }
}