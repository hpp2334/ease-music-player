package com.kutedev.easemusicplayer.core

import android.app.PendingIntent
import android.content.Intent
import android.os.Bundle
import androidx.annotation.OptIn
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.session.CommandButton
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import androidx.media3.session.SessionCommand
import androidx.media3.session.SessionResult
import com.google.common.collect.ImmutableList
import com.google.common.util.concurrent.ListenableFuture
import com.kutedev.easemusicplayer.MainActivity
import com.kutedev.easemusicplayer.singleton.Bridge
import com.kutedev.easemusicplayer.singleton.PlayerControllerRepository
import com.kutedev.easemusicplayer.singleton.PlayerRepository
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch
import uniffi.ease_client_backend.MusicAbstract
import uniffi.ease_client_backend.Playlist
import uniffi.ease_client_backend.ctGetMusic
import uniffi.ease_client_backend.easeLog
import javax.inject.Inject


const val PLAYER_TO_PREV_COMMAND = "PLAYER_TO_PREV_COMMAND"
const val PLAYER_TO_NEXT_COMMAND = "PLAYER_TO_NEXT_COMMAND"


/**
 * Background [MediaSessionService] that owns the [MediaSession] exposed
 * to system controllers (notification / lock-screen / Bluetooth / Auto).
 *
 * The session is backed by a [CantodePlayer] (a [SimpleBasePlayer]
 * wrapping a cantode `PlayerHandle`) instead of ExoPlayer. Audio decode
 * and output happen entirely in Rust.
 *
 * Because [CantodePlayer] is constructed lazily in [PlayerControllerRepository.setupCantodePlayer]
 * (after the backend initializes), this service collects
 * [PlayerControllerRepository.cantodePlayerState] and builds the session
 * the first time it observes a non-null value.
 */
@AndroidEntryPoint
class PlaybackService : MediaSessionService() {
    @Inject lateinit var playerRepository: PlayerRepository
    @Inject lateinit var bridge: Bridge
    @Inject lateinit var playerControllerRepository: PlayerControllerRepository

    private val serviceScope = CoroutineScope(Dispatchers.Main + Job())
    private var _mediaSession: MediaSession? = null
    private var attachedPlayer: CantodePlayer? = null

    @OptIn(UnstableApi::class)
    override fun onCreate() {
        super.onCreate()
        easeLog("Playback service creating...")

        // Sleep timer: pause requests come from PlayerRepository.
        serviceScope.launch(Dispatchers.Main) {
            playerRepository.pauseRequest.collect {
                attachedPlayer?.pause()
            }
        }

        // Build the MediaSession as soon as the CantodePlayer is published.
        serviceScope.launch {
            playerControllerRepository.cantodePlayerState.collectLatest { player ->
                if (player != null && attachedPlayer !== player) {
                    attachPlayer(player)
                }
            }
        }

        // Auto-advance on ENDED.
        serviceScope.launch {
            playerControllerRepository.endedEvent.collect {
                playOnComplete()
            }
        }

        easeLog("Playback service created")
    }

    @OptIn(UnstableApi::class)
    private fun attachPlayer(player: CantodePlayer) {
        val oldSession = _mediaSession
        attachedPlayer = player

        val intent = Intent(this, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_NEW_TASK
        }
        val pendingIntent = PendingIntent.getActivity(
            this, 0, intent,
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

        _mediaSession = buildSession(player, pendingIntent)
        oldSession?.release()
        easeLog("PlaybackService attached CantodePlayer + built MediaSession")
    }

    @OptIn(UnstableApi::class)
    private fun buildSession(player: CantodePlayer, pendingIntent: PendingIntent): MediaSession {
        return MediaSession.Builder(this, player)
            .setSessionActivity(pendingIntent)
            .setCallback(object : MediaSession.Callback {
                override fun onConnect(
                    session: MediaSession,
                    controller: MediaSession.ControllerInfo,
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
                        return MediaSession.ConnectionResult.AcceptedResultBuilder(session)
                            .setCustomLayout(
                                ImmutableList.of(
                                    CommandButton.Builder()
                                        .setSessionCommand(customPrevCommand)
                                        .setIconResId(
                                            CommandButton.getIconResIdForIconConstant(
                                                CommandButton.ICON_PREVIOUS,
                                            ),
                                        )
                                        .setDisplayName("Previous")
                                        .build(),
                                    CommandButton.Builder()
                                        .setSessionCommand(customNextCommand)
                                        .setIconResId(
                                            CommandButton.getIconResIdForIconConstant(
                                                CommandButton.ICON_NEXT,
                                            ),
                                        )
                                        .setDisplayName("Next")
                                        .build(),
                                ),
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
                    args: Bundle,
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
    }

    override fun onTaskRemoved(rootIntent: Intent?) {
        stopSelf()
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? {
        return _mediaSession
    }

    override fun onDestroy() {
        super.onDestroy()
        _mediaSession?.release()
        _mediaSession = null
        attachedPlayer = null
        serviceScope.cancel()
    }

    fun play(musicAbstract: MusicAbstract, playlist: Playlist) {
        serviceScope.launch {
            val music = bridge.run { ctGetMusic(it, musicAbstract.meta.id) } ?: return@launch
            playerRepository.setCurrent(music, playlist)
            playerControllerRepository.play(musicAbstract.meta.id, playlist.abstr.meta.id)
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
