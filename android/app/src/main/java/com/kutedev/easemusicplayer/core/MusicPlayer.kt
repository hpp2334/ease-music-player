package com.kutedev.easemusicplayer.core

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.ServiceInfo
import android.media.AudioAttributes
import android.media.AudioFocusRequest
import android.media.AudioManager
import android.os.Build
import android.os.IBinder
import android.support.v4.media.MediaMetadataCompat
import android.support.v4.media.session.MediaSessionCompat
import android.support.v4.media.session.PlaybackStateCompat
import androidx.core.app.NotificationCompat
import androidx.media.app.NotificationCompat.MediaStyle
import androidx.media.session.MediaButtonReceiver
import com.kutedev.easemusicplayer.MainActivity
import com.kutedev.easemusicplayer.R
import com.kutedev.easemusicplayer.singleton.PlayerControllerRepository
import com.kutedev.easemusicplayer.singleton.PlayerRepository
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch
import com.kutedev.easemusicplayer.singleton.types.Music
import com.kutedev.easemusicplayer.singleton.types.Playlist
import javax.inject.Inject


/**
 * Background [android.app.Service] that owns a [MediaSessionCompat] exposed
 * to system controllers (notification / lock-screen / Bluetooth / Auto).
 *
 * The session is backed by a [PlayerControllerRepository] (cantode audio
 * engine over UniFFI). Audio decode and output happen entirely in Rust;
 * this service is the system-integration surface.
 *
 * Replaces the previous `PlaybackService extends MediaSessionService`
 * (media3). Differences:
 * - Plain [android.app.Service], not `MediaSessionService`.
 * - Builds a [MediaSessionCompat] directly — no `SimpleBasePlayer`
 *   adapter, no media3 dependency.
 * - Owns the foreground media notification via
 *   [androidx.media.app.NotificationCompat.MediaStyle] (from
 *   `androidx.media:media`, the older compat lib — not media3).
 * - Manages audio focus + audio-becoming-noisy handling that media3
 *   previously did for us.
 *
 * Started lazily on first [PlayerControllerRepository.play]; stays
 * foreground while a track is loaded, stops itself on transport stop.
 */
@AndroidEntryPoint
class PlaybackService : android.app.Service() {
    @Inject lateinit var playerRepository: PlayerRepository
    @Inject lateinit var playerControllerRepository: PlayerControllerRepository
    @Inject lateinit var bridge: com.kutedev.easemusicplayer.singleton.Bridge

    private val serviceScope = CoroutineScope(Dispatchers.Main + Job())

    private var mediaSession: MediaSessionCompat? = null
    private var sessionActivityPendingIntent: PendingIntent? = null
    private var notificationManager: NotificationManager? = null
    private var audioManager: AudioManager? = null
    private var audioFocusRequest: AudioFocusRequest? = null

    @Volatile private var lastMusic: Music? = null
    @Volatile private var lastPlaylist: Playlist? = null
    @Volatile private var lastPlaying: Boolean = false
    @Volatile private var lastLoading: Boolean = false
    @Volatile private var focusRequested: Boolean = false

    private var becomingNoisyReceiverRegistered = false

    private val becomingNoisyReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context?, intent: Intent?) {
            if (intent?.action == AudioManager.ACTION_AUDIO_BECOMING_NOISY) {
                bridge.logRaw("info", "audio becoming noisy → pause")
                playerControllerRepository.pause()
            }
        }
    }

    override fun onCreate() {
        super.onCreate()
        bridge.logRaw("info", "Playback service creating...")
        notificationManager = getSystemService(NotificationManager::class.java)
        audioManager = getSystemService(AudioManager::class.java)
        createNotificationChannel()
        buildAudioFocusRequest()
        buildSession()
        observeState()
        bridge.logRaw("info", "Playback service created")
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        // MediaButton intents (Bluetooth / wired-headset media keys)
        // are forwarded to the active MediaSession by androidx.media.
        if (intent != null) {
            MediaButtonReceiver.handleIntent(mediaSession, intent)
        }
        // Every startForegroundService() call arms a fresh ~10 s
        // "must call startForeground()" window — even when the service
        // is already running. Satisfy it immediately, unconditionally,
        // with no dependency on the (async, network-bound) track-load
        // chain: gating startForeground() on `music != null` crashed
        // the app (ForegroundServiceDidNotStartInTimeException) whenever
        // a load stalled. The state observers refresh / detach the
        // notification afterwards.
        promoteToForeground()
        return START_NOT_STICKY
    }

    override fun onTaskRemoved(rootIntent: Intent?) {
        stopSelf()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onDestroy() {
        super.onDestroy()
        serviceScope.cancel()
        unregisterBecomingNoisy()
        abandonAudioFocus()
        mediaSession?.run {
            isActive = false
            release()
        }
        mediaSession = null
        bridge.logRaw("info", "Playback service destroyed")
    }

    // ----- session -----

    private fun buildSession() {
        val sessionActivity = PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java).apply {
                flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP
            },
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        sessionActivityPendingIntent = sessionActivity

        val session = MediaSessionCompat(this, "EaseMusicPlayer").apply {
            setCallback(object : MediaSessionCompat.Callback() {
                override fun onPlay() = playerControllerRepository.resume()
                override fun onPause() = playerControllerRepository.pause()
                override fun onSkipToNext() = playerControllerRepository.playNext()
                override fun onSkipToPrevious() = playerControllerRepository.playPrevious()
                override fun onSeekTo(pos: Long) {
                    playerControllerRepository.seek(pos.toULong())
                }
                override fun onStop() = playerControllerRepository.stop()
            })
            setSessionActivity(sessionActivity)
            isActive = true
        }
        mediaSession = session

        // Push an initial empty state so the session is well-formed.
        updateSessionState()
    }

    // ----- state observation + notification refresh -----

    private fun observeState() {
        serviceScope.launch {
            playerRepository.music.collectLatest { m ->
                lastMusic = m
                refreshForeground()
            }
        }
        serviceScope.launch {
            playerRepository.playlist.collectLatest { p ->
                lastPlaylist = p
                refreshForeground()
            }
        }
        serviceScope.launch {
            playerRepository.playing.collectLatest { playing ->
                lastPlaying = playing
                onPlayStateChanged()
                refreshForeground()
            }
        }
        serviceScope.launch {
            playerRepository.loading.collectLatest { loading ->
                lastLoading = loading
                refreshForeground()
            }
        }
        // Sleep-timer pause requests arrive via PlayerRepository.
        serviceScope.launch {
            playerRepository.pauseRequest.collect {
                playerControllerRepository.pause()
            }
        }
        // Position ticker — 2 Hz is enough for the notification /
        // lock-screen position display; PlaybackStateCompat extrapolates
        // between updates using the playback speed.
        serviceScope.launch {
            while (true) {
                updateSessionState()
                delay(POSITION_TICK_INTERVAL_MS)
            }
        }
    }

    private fun onPlayStateChanged() {
        if (lastPlaying) {
            registerBecomingNoisy()
        }
    }

    /**
     * Refresh the foreground notification + MediaSession state. Called
     * whenever something user-visible changes (music, play/pause, loading)
     * plus on the position ticker.
     */
    private fun refreshForeground() {
        updateSessionState()
        val notification = buildNotification()
        if (lastMusic != null) {
            // First time we promote to foreground for this service lifetime:
            // request audio focus once. Subsequent promotions (e.g. user
            // plays → pauses → plays again) reuse the same focus request.
            requestAudioFocus()
            promoteToForeground()
        } else {
            // No track loaded — detach from foreground but keep the
            // service alive so the session stays connected.
            stopForeground(STOP_FOREGROUND_DETACH)
            notificationManager?.notify(NOTIFICATION_ID, notification)
        }
    }

    /**
     * Promote this service to foreground (mediaPlayback type on U+) with the
     * current notification. Cheap and idempotent: safe to call on every
     * [onStartCommand] delivery and on every state refresh.
     */
    private fun promoteToForeground() {
        val notification = buildNotification()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            startForeground(
                NOTIFICATION_ID,
                notification,
                ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK,
            )
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }
    }

    private fun updateSessionState() {
        val session = mediaSession ?: return
        val music = lastMusic

        val (state, actions) = when {
            music == null -> PlaybackStateCompat.STATE_NONE to 0L
            lastLoading -> PlaybackStateCompat.STATE_BUFFERING to defaultActions()
            lastPlaying -> PlaybackStateCompat.STATE_PLAYING to defaultActions()
            else -> PlaybackStateCompat.STATE_PAUSED to defaultActions()
        }

        val posMs = if (music != null) playerControllerRepository.getCurrentPosition() else 0L
        val playbackState = PlaybackStateCompat.Builder()
            .setActions(actions)
            .setState(state, posMs, if (lastPlaying) 1.0f else 0.0f)
            .build()
        session.setPlaybackState(playbackState)

        if (music != null) {
            val meta = MediaMetadataCompat.Builder()
                .putString(
                    MediaMetadataCompat.METADATA_KEY_TITLE,
                    music.meta.title,
                )
                .putString(
                    MediaMetadataCompat.METADATA_KEY_ARTIST,
                    lastPlaylist?.abstr?.meta?.title ?: "",
                )
                .putString(
                    MediaMetadataCompat.METADATA_KEY_ALBUM,
                    lastPlaylist?.abstr?.meta?.title ?: "",
                )
                .putLong(
                    MediaMetadataCompat.METADATA_KEY_DURATION,
                    music.meta.duration ?: 0L,
                )
                .build()
            session.setMetadata(meta)
        } else {
            session.setMetadata(null)
        }
    }

    private fun defaultActions(): Long = (
        PlaybackStateCompat.ACTION_PLAY
            or PlaybackStateCompat.ACTION_PAUSE
            or PlaybackStateCompat.ACTION_PLAY_PAUSE
            or PlaybackStateCompat.ACTION_SKIP_TO_NEXT
            or PlaybackStateCompat.ACTION_SKIP_TO_PREVIOUS
            or PlaybackStateCompat.ACTION_SEEK_TO
            or PlaybackStateCompat.ACTION_STOP
        )

    // ----- notification -----

    private fun createNotificationChannel() {
        val channel = NotificationChannel(
            NOTIFICATION_CHANNEL_ID,
            "Music Playback",
            NotificationManager.IMPORTANCE_LOW,
        ).apply {
            description = "Media playback controls and now-playing metadata"
            setShowBadge(false)
        }
        notificationManager?.createNotificationChannel(channel)
    }

    private fun buildNotification(): android.app.Notification {
        val music = lastMusic
        val session = mediaSession

        val playPauseIcon = if (lastPlaying) R.drawable.icon_pause else R.drawable.icon_play
        val playPauseTitle = if (lastPlaying) "Pause" else "Play"

        val builder = NotificationCompat.Builder(this, NOTIFICATION_CHANNEL_ID)
            .setSmallIcon(R.drawable.icon_music_note)
            .setOnlyAlertOnce(true)
            .setShowWhen(false)
            .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)
            .setContentTitle(music?.meta?.title ?: "Ease Music Player")
            .setContentText(lastPlaylist?.abstr?.meta?.title ?: "")
            .setContentIntent(sessionActivityPendingIntent)
            .addAction(
                R.drawable.icon_play_previous, "Previous",
                mediaButtonPendingIntent(PlaybackStateCompat.ACTION_SKIP_TO_PREVIOUS),
            )
            .addAction(
                playPauseIcon, playPauseTitle,
                mediaButtonPendingIntent(PlaybackStateCompat.ACTION_PLAY_PAUSE),
            )
            .addAction(
                R.drawable.icon_play_next, "Next",
                mediaButtonPendingIntent(PlaybackStateCompat.ACTION_SKIP_TO_NEXT),
            )

        if (session != null) {
            builder.setStyle(
                MediaStyle()
                    .setShowActionsInCompactView(0, 1, 2)
                    .setMediaSession(session.sessionToken),
            )
            builder.setDeleteIntent(
                mediaButtonPendingIntent(PlaybackStateCompat.ACTION_STOP),
            )
        }
        return builder.build()
    }

    private fun mediaButtonPendingIntent(action: Long): PendingIntent =
        MediaButtonReceiver.buildMediaButtonPendingIntent(this, action)

    // ----- audio focus -----

    /**
     * Build the single [AudioFocusRequest] used for this service's
     * lifetime. Held in [audioFocusRequest] and submitted to
     * [AudioManager.requestAudioFocus] at most once by [requestAudioFocus].
     *
     * Reusing the same request object is essential — Android's
     * [AudioManager] treats each unique listener / request instance as a
     * distinct focus owner, so building a new request per play would
     * cause the previous owner (us) to receive [AudioManager.AUDIOFOCUS_LOSS]
     * immediately and pause our own playback.
     */
    @androidx.annotation.RequiresApi(Build.VERSION_CODES.O)
    private fun buildAudioFocusRequest() {
        if (audioFocusRequest != null) return
        val attrs = AudioAttributes.Builder()
            .setUsage(AudioAttributes.USAGE_MEDIA)
            .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC)
            .build()
        audioFocusRequest = AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
            .setAudioAttributes(attrs)
            .setOnAudioFocusChangeListener(::onAudioFocusChange)
            .setWillPauseWhenDucked(false)
            .setAcceptsDelayedFocusGain(false)
            .build()
    }

    /**
     * Submit [audioFocusRequest] exactly once per service lifetime.
     * Returns silently on subsequent calls.
     */
    private fun requestAudioFocus() {
        val am = audioManager ?: return
        if (focusRequested) return
        val req = audioFocusRequest
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            if (req == null) return
            val result = am.requestAudioFocus(req)
            bridge.logRaw("info", "requestAudioFocus: result=$result")
            if (result == AudioManager.AUDIOFOCUS_REQUEST_GRANTED) {
                focusRequested = true
            }
        } else {
            @Suppress("DEPRECATION")
            am.requestAudioFocus(
                ::onAudioFocusChange,
                AudioManager.STREAM_MUSIC,
                AudioManager.AUDIOFOCUS_GAIN,
            )
            focusRequested = true
        }
    }

    private fun abandonAudioFocus() {
        val am = audioManager ?: return
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            audioFocusRequest?.let { am.abandonAudioFocusRequest(it) }
        } else {
            @Suppress("DEPRECATION")
            am.abandonAudioFocus(::onAudioFocusChange)
        }
        focusRequested = false
    }

    private fun onAudioFocusChange(focusChange: Int) {
        when (focusChange) {
            AudioManager.AUDIOFOCUS_LOSS -> {
                bridge.logRaw("info", "audio focus lost → pause (will not auto-resume)")
                playerControllerRepository.pause()
            }
            AudioManager.AUDIOFOCUS_LOSS_TRANSIENT,
            AudioManager.AUDIOFOCUS_LOSS_TRANSIENT_CAN_DUCK -> {
                bridge.logRaw("info", "audio focus transient loss → pause")
                playerControllerRepository.pause()
            }
            AudioManager.AUDIOFOCUS_GAIN -> {
                bridge.logRaw("info", "audio focus regained")
            }
        }
    }

    // ----- audio becoming noisy -----

    private fun registerBecomingNoisy() {
        if (becomingNoisyReceiverRegistered) return
        registerReceiver(
            becomingNoisyReceiver,
            IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY),
        )
        becomingNoisyReceiverRegistered = true
    }

    private fun unregisterBecomingNoisy() {
        if (!becomingNoisyReceiverRegistered) return
        runCatching { unregisterReceiver(becomingNoisyReceiver) }
        becomingNoisyReceiverRegistered = false
    }

    companion object {
        private const val NOTIFICATION_ID = 1
        private const val NOTIFICATION_CHANNEL_ID = "EaseMusicPlaybackChannel"
        private const val POSITION_TICK_INTERVAL_MS = 500L

        /**
         * Convenience for starting the service from
         * [PlayerControllerRepository.play].
         */
        fun start(context: Context) {
            val intent = Intent(context, PlaybackService::class.java)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                context.startForegroundService(intent)
            } else {
                context.startService(intent)
            }
        }
    }
}
