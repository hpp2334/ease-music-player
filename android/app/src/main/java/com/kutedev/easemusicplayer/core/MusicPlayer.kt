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
import android.media.AudioPlaybackConfiguration
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.PowerManager
import android.net.wifi.WifiManager
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
    private var wakeLock: PowerManager.WakeLock? = null
    private var wifiLock: WifiManager.WifiLock? = null

    @Volatile private var lastMusic: Music? = null
    @Volatile private var lastPlaylist: Playlist? = null
    @Volatile private var lastPlaying: Boolean = false
    @Volatile private var lastLoading: Boolean = false
    @Volatile private var focusHeld: Boolean = false

    /**
     * True while playback is paused *because audio focus was taken away*
     * (vs. a user-initiated pause). Arms the auto-resume paths: the
     * [AudioManager.AUDIOFOCUS_GAIN] listener and the active-playback
     * watchdog below. Only set when playback was actually underway, so
     * an already-idle session is never resurrected by a stray focus
     * event.
     */
    @Volatile private var pausedByFocusLoss: Boolean = false

    private val mainHandler = Handler(Looper.getMainLooper())
    private var focusRecoveryPending: Runnable? = null

    private var becomingNoisyReceiverRegistered = false

    private val becomingNoisyReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context?, intent: Intent?) {
            if (intent?.action == AudioManager.ACTION_AUDIO_BECOMING_NOISY) {
                bridge.logRaw("info", "audio becoming noisy → pause")
                // The playback context changed under us (unplugged
                // headphones) — this is not a focus-caused pause, so the
                // focus-recovery watchdog must not resurrect it later.
                pausedByFocusLoss = false
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
        // Watches system-wide playback while we're focus-paused so the
        // recovery watchdog below notices when the last other player
        // goes silent (see [maybeRecoverFocus]).
        audioManager?.registerAudioPlaybackCallback(playbackCallback, null)
        buildSession()
        observeState()
        bridge.logRaw("info", "Playback service created")
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        bridge.logRaw("info", "Playback service onStartCommand action=${intent?.action ?: "-"}")
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
        // Task removal (a swipe, or MIUI's lock-screen memory cleanup,
        // which removes the app's task a few minutes after lock) must
        // not kill an active player: the foreground media service
        // outlives its task on purpose. Only an idle player lets the
        // stop through.
        if (playerRepository.isActive()) {
            bridge.logRaw("info", "task removed — playback active, keeping the service")
            return
        }
        bridge.logRaw("info", "task removed — idle, stopping the service")
        stopSelf()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onDestroy() {
        super.onDestroy()
        serviceScope.cancel()
        unregisterBecomingNoisy()
        audioManager?.unregisterAudioPlaybackCallback(playbackCallback)
        focusRecoveryPending?.let { mainHandler.removeCallbacks(it) }
        abandonAudioFocus()
        releaseWakeLock()
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
                // Every transport callback is a user action: void any
                // pending focus-recovery so the watchdog never
                // auto-resumes over an explicit user pause/stop.
                override fun onPlay() {
                    pausedByFocusLoss = false
                    playerControllerRepository.resume()
                }

                override fun onPause() {
                    pausedByFocusLoss = false
                    playerControllerRepository.pause()
                }

                override fun onSkipToNext() = playerControllerRepository.playNext()
                override fun onSkipToPrevious() = playerControllerRepository.playPrevious()
                override fun onSeekTo(pos: Long) {
                    playerControllerRepository.seek(pos.toULong())
                }

                override fun onStop() {
                    pausedByFocusLoss = false
                    playerControllerRepository.stop()
                }
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
                // Keep the wake lock decision fresh even if a state
                // collector was missed (bounded-acquire re-arm).
                ensureWakeLock()
                delay(POSITION_TICK_INTERVAL_MS)
            }
        }
    }

    private fun onPlayStateChanged() {
        if (lastPlaying) {
            // Playback is happening again (user picked a track, or a
            // resumed load reached PLAYING) — whatever focus-recovery
            // was pending is moot.
            pausedByFocusLoss = false
            registerBecomingNoisy()
        }
        ensureWakeLock()
    }

    // ----- wake lock -----

    /**
     * Hold a partial wake lock while a track is loaded and playback is
     * active (`playing` **or** loading/buffering). Decode and network
     * readahead run on ordinary Rust/tokio threads — without a wake lock
     * the CPU can suspend once the screen goes off, the sink ring drains,
     * and playback silently dies (the network refill stalls too, so
     * `Buffering` never resolves until the screen comes back).
     *
     * The lock also covers the loading/buffering window on purpose: a
     * screen-off readahead refill needs the CPU just as much as decode.
     *
     * Hold a Wi-Fi lock alongside: with the screen off the Wi-Fi radio
     * drops into power-save and streaming throughput collapses (or dies
     * outright on aggressive OEM builds) a few minutes in — exactly the
     * "plays a while, then buffers forever" stall. The high-perf mode
     * keeps the radio at full performance for the same window the CPU
     * lock covers. No manifest permission needed.
     *
     * Both locks are re-evaluated on every state change and on the
     * position ticker (self-healing), with a bounded acquire timeout as
     * leak insurance: if a release is ever missed, the ticker re-arms
     * them on the next tick while playback is still active.
     */
    private fun ensureWakeLock() {
        // isActive() spans playing/loading plus the auto-advance grace
        // window: the next track's storage probe runs before the engine
        // publishes LOADING, and with the screen off that window must
        // stay protected (locks + task-removal guard) or MIUI kills the
        // handoff and playback ends with the playlist.
        val shouldHold = playerRepository.isActive()
        if (shouldHold) {
            val lock = wakeLock ?: getSystemService(PowerManager::class.java)
                .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, WAKE_LOCK_TAG)
                .apply {
                    setReferenceCounted(false)
                    wakeLock = this
                }
            if (!lock.isHeld) {
                lock.acquire(WAKE_LOCK_TIMEOUT_MS)
                bridge.logRaw("info", "playback wake lock acquired")
            }
            val wfLock = wifiLock ?: getSystemService(WifiManager::class.java)
                .createWifiLock(WifiManager.WIFI_MODE_FULL_HIGH_PERF, WAKE_LOCK_TAG + ":wifi")
                .apply {
                    setReferenceCounted(false)
                    wifiLock = this
                }
            // No acquire(timeout) on WifiLock — the release path is the
            // state change below (and onDestroy); the ticker re-arms it.
            if (!wfLock.isHeld) {
                wfLock.acquire()
                bridge.logRaw("info", "playback wifi lock acquired")
            }
        } else {
            releaseWakeLock()
        }
    }

    private fun releaseWakeLock() {
        val lock = wakeLock ?: return
        if (lock.isHeld) {
            lock.release()
            bridge.logRaw("info", "playback wake lock released")
        }
        wifiLock?.let { wfLock ->
            if (wfLock.isHeld) {
                wfLock.release()
                bridge.logRaw("info", "playback wifi lock released")
            }
        }
    }

    /**
     * Refresh the foreground notification + MediaSession state. Called
     * whenever something user-visible changes (music, play/pause, loading)
     * plus on the position ticker.
     */
    private fun refreshForeground() {
        updateSessionState()
        ensureWakeLock()
        val notification = buildNotification()
        if (lastMusic != null) {
            // (Re)acquire audio focus for the visible player: the first
            // promotion requests it; later promotions (user plays again
            // after another app took focus) re-request it when not held.
            // Same request object every time — see [buildAudioFocusRequest].
            //
            // NOT while paused-by-focus-loss: the pause itself runs
            // through here, and re-requesting 100 ms after the loss
            // would grant us the focus back (the framework re-grants the
            // requester immediately), making the interruption's
            // AUDIOFOCUS_GAIN never arrive and the watchdog's re-request
            // a no-op — the player then sat paused forever. Recovery is
            // owned by the GAIN listener / the watchdog below; a user
            // play clears the flag first (transport callback or
            // [onPlayStateChanged]).
            if (!pausedByFocusLoss) {
                requestAudioFocus()
            }
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
     * [AudioManager.requestAudioFocus] by [requestAudioFocus] whenever
     * focus is not currently held.
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
     * Submit [audioFocusRequest] whenever focus is not currently held:
     * on the first foreground promotion, when the user plays after the
     * focus was taken away (heals a focus-less manual resume), and from
     * the focus-recovery watchdog. Returns silently when focus is
     * already held.
     */
    private fun requestAudioFocus() {
        val am = audioManager ?: return
        if (focusHeld) return
        val req = audioFocusRequest
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            if (req == null) return
            val result = am.requestAudioFocus(req)
            bridge.logRaw("info", "requestAudioFocus: result=$result")
            if (result == AudioManager.AUDIOFOCUS_REQUEST_GRANTED) {
                focusHeld = true
            }
        } else {
            @Suppress("DEPRECATION")
            am.requestAudioFocus(
                ::onAudioFocusChange,
                AudioManager.STREAM_MUSIC,
                AudioManager.AUDIOFOCUS_GAIN,
            )
            focusHeld = true
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
        focusHeld = false
    }

    /**
     * Active-playback watchdog: while paused because the focus was taken
     * away, every system-wide playback change re-checks whether anyone
     * else is still playing. When the last other player goes silent —
     * the other app paused or stopped, and it is under no obligation to
     * *abandon* focus, in which case Android notifies nobody — re-request
     * focus; the grant arrives as [AudioManager.AUDIOFOCUS_GAIN] and
     * [onAudioFocusChange] resumes playback.
     */
    private val playbackCallback = object : AudioManager.AudioPlaybackCallback() {
        override fun onPlaybackConfigChanged(configs: MutableList<AudioPlaybackConfiguration>) {
            if (!pausedByFocusLoss) return
            scheduleFocusRecoveryCheck()
        }
    }

    private fun scheduleFocusRecoveryCheck() {
        // Config changes arrive in bursts (start → pause → release); the
        // debounce keeps the recovery from racing an in-flight transition.
        focusRecoveryPending?.let { mainHandler.removeCallbacks(it) }
        val r = Runnable { maybeRecoverFocus() }
        focusRecoveryPending = r
        mainHandler.postDelayed(r, 1_000)
    }

    private fun maybeRecoverFocus() {
        if (!pausedByFocusLoss) return
        // `activePlaybackConfigurations` lists only players that are
        // actively playing — ours is paused, so anything listed belongs
        // to another app. A non-empty list (another player, or a call)
        // means the interruption is still ongoing: wait for the next
        // config change.
        val others = audioManager?.activePlaybackConfigurations.orEmpty()
        if (others.isNotEmpty()) {
            bridge.logRaw("info", "focus recovery: ${others.size} other active player(s) — waiting")
            return
        }
        bridge.logRaw("info", "focus recovery: no other active player — re-requesting audio focus")
        requestAudioFocus()
    }

    private fun onAudioFocusChange(focusChange: Int) {
        when (focusChange) {
            AudioManager.AUDIOFOCUS_LOSS -> {
                focusHeld = false
                if (playerRepository.playing.value || playerRepository.loading.value) {
                    bridge.logRaw("info", "audio focus lost → pause (auto-resume armed)")
                    pausedByFocusLoss = true
                    playerControllerRepository.pause()
                } else {
                    bridge.logRaw("info", "audio focus lost (idle — nothing to pause)")
                }
            }
            AudioManager.AUDIOFOCUS_LOSS_TRANSIENT,
            AudioManager.AUDIOFOCUS_LOSS_TRANSIENT_CAN_DUCK -> {
                focusHeld = false
                if (playerRepository.playing.value || playerRepository.loading.value) {
                    bridge.logRaw("info", "audio focus transient loss → pause (auto-resume armed)")
                    pausedByFocusLoss = true
                    playerControllerRepository.pause()
                } else {
                    bridge.logRaw("info", "audio focus transient loss (idle — nothing to pause)")
                }
            }
            AudioManager.AUDIOFOCUS_GAIN -> {
                focusHeld = true
                if (pausedByFocusLoss) {
                    pausedByFocusLoss = false
                    bridge.logRaw("info", "audio focus regained → resume")
                    playerControllerRepository.resume()
                } else {
                    bridge.logRaw("info", "audio focus regained (was not focus-paused)")
                }
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
        private const val WAKE_LOCK_TAG = "ease:playback"

        /** Leak insurance: re-armed by the position ticker while playing. */
        private const val WAKE_LOCK_TIMEOUT_MS = 60L * 60L * 1000L

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
