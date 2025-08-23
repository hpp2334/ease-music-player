package com.kutedev.easemusicplayer.core

import android.app.PendingIntent
import android.content.Intent
import android.os.Bundle
import androidx.annotation.OptIn
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.C.WAKE_MODE_NETWORK
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.session.CommandButton
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import androidx.media3.session.SessionCommand
import androidx.media3.session.SessionResult
import com.google.common.collect.ImmutableList
import com.google.common.util.concurrent.ListenableFuture
import com.kutedev.easemusicplayer.MainActivity
import com.kutedev.easemusicplayer.singleton.PlayerRepository
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import uniffi.ease_client_backend.Playlist
import uniffi.ease_client_backend.ctGetMusic
import javax.inject.Inject
import com.kutedev.easemusicplayer.singleton.Bridge
import dagger.hilt.android.AndroidEntryPoint
import uniffi.ease_client_backend.MusicAbstract
import uniffi.ease_client_backend.easeLog


const val PLAYER_TO_PREV_COMMAND = "PLAYER_TO_PREV_COMMAND";
const val PLAYER_TO_NEXT_COMMAND = "PLAYER_TO_NEXT_COMMAND";



@AndroidEntryPoint
class PlaybackService : MediaSessionService() {
    @Inject lateinit var playerRepository: PlayerRepository
    @Inject lateinit var bridge: Bridge
    private val serviceScope = CoroutineScope(Dispatchers.Main + Job())
    private var _mediaSession: MediaSession? = null

    override fun onCreate() {
        super.onCreate()
        easeLog("Playback service creating...")
        val context = this

        val intent = Intent(this, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_NEW_TASK
        }
        val pendingIntent = PendingIntent.getActivity(this, 0, intent,
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)

        val player = ExoPlayer.Builder(context)
            .setAudioAttributes(
                AudioAttributes.Builder()
                    .setUsage(C.USAGE_MEDIA)
                    .setContentType(C.AUDIO_CONTENT_TYPE_MUSIC)
                    .build(),
                true
            )
            .setHandleAudioBecomingNoisy(true)
            .setWakeMode(WAKE_MODE_NETWORK)
            .build()
        _mediaSession = MediaSession.Builder(this, player)
            .setSessionActivity(pendingIntent)
            .setCallback(object : MediaSession.Callback {
                @OptIn(UnstableApi::class)
                override fun onConnect(
                    session: MediaSession,
                    controller: MediaSession.ControllerInfo
                ): MediaSession.ConnectionResult {
                    if (session.isMediaNotificationController(controller)) {
                        val customPrevCommand = SessionCommand(PLAYER_TO_PREV_COMMAND, Bundle.EMPTY)
                        val customNextCommand = SessionCommand(PLAYER_TO_NEXT_COMMAND, Bundle.EMPTY)

                        val sessionCommands =
                            MediaSession.ConnectionResult.DEFAULT_SESSION_COMMANDS.buildUpon()
                                .add(customPrevCommand)
                                .add(customNextCommand)
                                .build()
                        val playerCommands =
                            MediaSession.ConnectionResult.DEFAULT_PLAYER_COMMANDS.buildUpon()
                                .remove(Player.COMMAND_SEEK_TO_PREVIOUS)
                                .remove(Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM)
                                .remove(Player.COMMAND_SEEK_TO_NEXT)
                                .remove(Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM)
                                .remove(Player.COMMAND_SEEK_BACK)
                                .remove(Player.COMMAND_SEEK_FORWARD)
                                .remove(Player.COMMAND_SEEK_TO_DEFAULT_POSITION)
                                .build()
                        // Custom layout and available commands to configure the legacy/framework session.
                        return MediaSession.ConnectionResult.AcceptedResultBuilder(session)
                            .setCustomLayout(
                                ImmutableList.of(
                                    CommandButton.Builder()
                                        .setSessionCommand(customPrevCommand)
                                        .setIconResId(CommandButton.getIconResIdForIconConstant(CommandButton.ICON_PREVIOUS))
                                        .setDisplayName("Previous")
                                        .build(),
                                    CommandButton.Builder()
                                        .setSessionCommand(customNextCommand)
                                        .setIconResId(CommandButton.getIconResIdForIconConstant(CommandButton.ICON_NEXT))
                                        .setDisplayName("Next")
                                        .build(),
                                )
                            )
                            .setAvailablePlayerCommands(playerCommands)
                            .setAvailableSessionCommands(sessionCommands)
                            .build()
                    }
                    return MediaSession.ConnectionResult.AcceptedResultBuilder(session).build()
                }

                override fun onCustomCommand(
                    session: MediaSession,
                    controller: MediaSession.ControllerInfo,
                    customCommand: SessionCommand,
                    args: Bundle
                ): ListenableFuture<SessionResult> {
                    if (customCommand.customAction == PLAYER_TO_PREV_COMMAND) {
                        playPrevious()
                    } else if (customCommand.customAction == PLAYER_TO_NEXT_COMMAND) {
                        playNext()
                    }
                    return super.onCustomCommand(session, controller, customCommand, args)
                }
            })
            .build()

        player.addListener(object : Player.Listener {
            override fun onIsPlayingChanged(isPlaying: Boolean) {
                playerRepository.setIsPlaying(isPlaying)
            }

            override fun onPlaybackStateChanged(playbackState: Int) {
                if (playbackState == Player.STATE_ENDED) {
                    playOnComplete()
                } else if (playbackState == Player.STATE_READY) {
                    playerRepository.setIsLoading(false)
                    syncMetadataUtil(serviceScope, bridge, player)
                } else if (playbackState == Player.STATE_BUFFERING) {
                    playerRepository.setIsLoading(true)
                }
            }

            override fun onPositionDiscontinuity(
                oldPosition: Player.PositionInfo,
                newPosition: Player.PositionInfo,
                reason: Int
            ) {
                playerRepository.notifyDurationChanged()
            }
        })
        easeLog("Playback service created")
    }


    override fun onTaskRemoved(rootIntent: Intent?) {
        stopSelf()
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? {
        return _mediaSession
    }

    override fun onDestroy() {
        super.onDestroy()
        _mediaSession?.player?.stop()
        _mediaSession?.player?.release()
        _mediaSession?.release()
        _mediaSession = null
        serviceScope.cancel()
    }


    fun play(musicAbstract: MusicAbstract, playlist: Playlist) {
        val player = _mediaSession?.player ?: return

        serviceScope.launch {
            val music = bridge.run { ctGetMusic(it, musicAbstract.meta.id) } ?: return@launch
            playerRepository.setCurrent(music, playlist)
            playUtil(musicAbstract, player)
        }
    }

    private fun playOnComplete() {
        val m = playerRepository.onCompleteMusic.value
        val p = playerRepository.playlist.value
        if (m != null && p != null) {
            play(m, p)
        }
    }

    private fun playNext() {
        val m = playerRepository.nextMusic.value
        val p = playerRepository.playlist.value
        if (m != null && p != null) {
            play(m, p)
        }
    }

    private fun playPrevious() {
        val m = playerRepository.previousMusic.value
        val p = playerRepository.playlist.value
        if (m != null && p != null) {
            play(m, p)
        }
    }
}
