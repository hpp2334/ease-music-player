package com.kutedev.easemusicplayer.singleton

import android.content.Context
import android.os.SystemClock
import com.kutedev.cantode.Cantode
import com.kutedev.cantode.PlayerState
import com.kutedev.easemusicplayer.core.PlaybackService
import com.kutedev.easemusicplayer.singleton.SleepModeState
import com.kutedev.easemusicplayer.singleton.types.ArgRemoveMusicFromPlaylist
import com.kutedev.easemusicplayer.singleton.types.LyricLoadState
import com.kutedev.easemusicplayer.singleton.types.Music
import com.kutedev.easemusicplayer.singleton.types.Playlist
import com.kutedev.easemusicplayer.singleton.types.MusicId
import com.kutedev.easemusicplayer.singleton.types.PlaylistId
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put
import javax.inject.Inject
import javax.inject.Singleton
import kotlin.math.abs
import kotlin.math.max

/**
 * Transport control surface for the player.
 *
 * Owns the cantode player handle IDs (registered on the Rust side) and
 * the [Cantode] engine facade (cantode's own Kotlin half, reached
 * through cantode's JNI bridge under the same handle id).
 *
 * Lifecycle:
 * 1. [com.kutedev.easemusicplayer.MainActivity.onCreate] runs `bridge.initialize()`,
 *    which creates the backend handle and starts [KeepBackendService].
 * 2. [com.kutedev.easemusicplayer.MainActivity.onStart] calls [setupCantodeEngine],
 *    which constructs the cantode handles + engine.
 * 3. [PlaybackService] collects [cantodeEngine] and wires its
 *    [android.support.v4.media.session.MediaSessionCompat] when the engine
 *    becomes available.
 */
@Singleton
class PlayerControllerRepository @Inject constructor(
    @ApplicationContext private val cx: Context,
    private val playerRepository: PlayerRepository,
    private val toastRepository: ToastRepository,
    private val playlistRepository: PlaylistRepository,
    private val storageRepository: StorageRepository,
    private val bridge: Bridge,
    private val _scope: CoroutineScope,
) {
    private val _playlist = playerRepository.playlist
    private val _music = playerRepository.music
    private val _sleep = MutableStateFlow(SleepModeState())

    private var _sleepJob: Job? = null
    private var lyricJob: Job? = null

    /**
     * Fetch + parse the current track's lyric over the storage seam (and
     * the embedded-tag fallback) in the background and patch the result
     * into the current music. Superseded by each new [play] and
     * id-guarded at apply time, so a stale fetch can't land on a newer
     * track.
     */
    private fun launchLyricFetch(id: MusicId) {
        lyricJob?.cancel()
        lyricJob = _scope.launch {
            val lyric = bridge.call(BridgeMethods.Music.LOAD_LYRIC, id)
                .unwrapOrNull()?.payload
            if (_music.value?.meta?.id == id) {
                playerRepository.updateMusicLyric(id, lyric)
            }
        }
    }

    /** Bumped on every [play] — lets a scheduled auto-advance retry detect
     *  that a newer play superseded it while it was backing off. */
    @Volatile private var playGeneration = 0L
    private val nextMusic = playerRepository.nextMusic
    private val previousMusic = playerRepository.previousMusic

    private val _endedEvent = MutableSharedFlow<Unit>(extraBufferCapacity = 4)
    val endedEvent = _endedEvent.asSharedFlow()

    private val _pluginEvents = MutableSharedFlow<PluginEvent>(extraBufferCapacity = 16)
    val pluginEvents = _pluginEvents.asSharedFlow()

    val sleepState = _sleep.asStateFlow()

    private val _cantodeEngine = MutableStateFlow<Cantode?>(null)
    val cantodeEngine = _cantodeEngine.asStateFlow()

    @Volatile private var playerContextId: Long = -1L
    @Volatile private var playerId: Long = -1L

    /**
     * Optimistic seek target while the engine catches up — see [seek]
     * and [getCurrentPosition].
     */
    @Volatile private var overridedSeek: OverridedSeek? = null

    /**
     * Parked resume position: the track [restoreLastPlayed] rehydrated
     * and where it was parked (user stop mid-track / process death while
     * playing). Applied once by the first successful [play] of that same
     * music id; any other track's play abandons it. Null = nothing parked.
     */
    private var parkedResumeMusicId: MusicId? = null
    private var parkedResumePositionMs: Long? = null

    private var setupStarted = false

    init {
        _scope.launch(Dispatchers.Main) {
            playlistRepository.preRemovePlaylistEvent.collect { id ->
                if (_playlist.value?.abstr?.meta?.id == id) {
                    stop()
                }
            }
        }
        _scope.launch(Dispatchers.Main) {
            playlistRepository.preRemoveMusicEvent.collect { arg ->
                if (_playlist.value?.abstr?.meta?.id == arg.playlistId
                    && _music.value?.meta?.id == arg.musicId) {
                    stop()
                }
            }
        }
        _scope.launch(Dispatchers.Main) {
            storageRepository.preRemoveStorageEvent.collect { id ->
                if (_music.value?.loc?.storageId == id) {
                    stop()
                }
            }
        }
        _scope.launch(Dispatchers.Main) {
            _endedEvent.collect {
                // Natural end: mark the persisted session completed
                // (replay from 0 — never "resume at the end"). When an
                // auto-advance follows, its `setCurrent` overwrites the
                // record anyway; this covers the exhausted-playlist and
                // single-track ends.
                playerRepository.markSessionCompleted()
                playOnComplete()
            }
        }
        // Parked-position snapshots: keep the persisted `last_played`
        // position fresh while audio is active, so a user stop or a
        // process death mid-track resumes where it left off (±5 s).
        // Cheap local write; skipped entirely when idle.
        _scope.launch {
            while (true) {
                delay(POSITION_SNAPSHOT_INTERVAL_MS)
                if (playerRepository.playing.value || playerRepository.loading.value) {
                    playerRepository.savePositionSnapshot(getCurrentPosition())
                }
            }
        }
    }

    /**
     * Constructs the cantode player context + player + the engine
     * facade ([Cantode] — cantode's own Kotlin half, reached through
     * cantode's JNI bridge under the same handle id).
     *
     * [engineFactory] receives the player handle ID (opaque Long — the
     * bridge id `player.new` also registered with cantode's FFI) and
     * returns a [Cantode] wrapping it.
     */
    fun setupCantodeEngine(engineFactory: (Long) -> Cantode) {
        if (setupStarted) return
        setupStarted = true
        _scope.launch(Dispatchers.Main) {
            try {
                // player.contextNew + player.new stay on callRaw — they
                // return raw `{handle: N}` payloads that we extract here.
                val ctxResp = bridge.callRaw("player.contextNew", handle = 0L)
                    .unwrapOrThrow().rawPayloadJson as JsonObject
                val ctxId = ctxResp["handle"]!!.jsonPrimitive.content.toLong()
                bridge.setPlayerContextId(ctxId)
                playerContextId = ctxId

                val playerResp = bridge.callRaw("player.new", handle = ctxId)
                    .unwrapOrThrow().rawPayloadJson as JsonObject
                val pId = playerResp["handle"]!!.jsonPrimitive.content.toLong()
                bridge.setPlayerId(pId)
                playerId = pId

                val engine = engineFactory(pId)
                _scope.launch {
                    engine.ended.collect {
                        _endedEvent.emit(Unit)
                        val m = _music.value
                        if (m != null) {
                            _pluginEvents.tryEmit(
                                PluginEvent.MusicComplete(
                                    musicId = m.meta.id,
                                    title = m.meta.title,
                                    timestamp = System.currentTimeMillis(),
                                )
                            )
                        }
                    }
                }
                // Engine truth → app state: the state mapping is app
                // policy, so it lives here, not inside cantode.
                _scope.launch {
                    var last: PlayerState? = null
                    engine.state.collect { st ->
                        if (last != null && last != st) {
                            bridge.logRaw("info", "engine state: $last → $st")
                        }
                        last = st
                        playerRepository.setIsPlaying(st == PlayerState.PLAYING)
                    }
                }
                _scope.launch {
                    engine.loading.collect { loading ->
                        playerRepository.setIsLoading(loading)
                    }
                }
                _scope.launch {
                    engine.error.collect { err ->
                        if (err != null) {
                            // The engine has already parked on `Paused`
                            // (the terminal source-error contract): the
                            // loading state cleared and the wake lock
                            // releases through the state change. Surface
                            // the stall once per episode — the next play
                            // is the retry (see [resume]).
                            bridge.logRaw("error", "source error: $err")
                            toastRepository.emitToast("Playback stalled — tap play to retry")
                        }
                    }
                }
                _cantodeEngine.value = engine
                playerRepository.reload()
                bridge.logRaw("info", "cantode engine setup complete (ctx=$ctxId player=$pId)")
            } catch (e: Exception) {
                setupStarted = false
                bridge.logRaw("error", "cantode engine setup failed: $e")
                _scope.launch { toastRepository.emitToast("Player setup failed: $e") }
            }
        }
    }

    /**
     * Rehydrate the player state after a process death in the background.
     *
     * The current track lives only in process memory, but the nav back
     * stack survives (saved instance state): relaunching from Recents
     * can land the user back on a PlayerPage whose repository is empty —
     * placeholder cover, no title, `--:--:--` duration, prev/next dead.
     * The `last_played` preference (kept current by
     * [PlayerRepository.setCurrent]/[PlayerRepository.resetCurrent])
     * restores the track as paused — no engine load, nothing sounds.
     *
     * The saved pair is a hint, validated exactly like [play]'s
     * membership check: the music must still exist and still belong to
     * the saved playlist. Skips when a track is already current (a
     * user-initiated play raced the restore and wins).
     */
    fun restoreLastPlayed() {
        if (_music.value != null) return
        _scope.launch(Dispatchers.Main) {
            val last = bridge.call(BridgeMethods.Preference.GET_LAST_PLAYED)
                .unwrapOrNull()?.payload ?: return@launch
            val music = bridge.call(BridgeMethods.Music.GET, last.musicId)
                .unwrapOrNull()?.payload ?: return@launch
            val playlist = bridge.call(BridgeMethods.Playlist.GET, last.playlistId)
                .unwrapOrNull()?.payload ?: return@launch
            if (playlist.musics.none { it.meta.id == last.musicId }) {
                bridge.logRaw(
                    "info",
                    "restoreLastPlayed: music ${last.musicId.value} no longer in " +
                        "playlist ${last.playlistId.value} — skipping",
                )
                return@launch
            }
            if (_music.value != null) return@launch
            bridge.logRaw(
                "info",
                "restoreLastPlayed: music ${last.musicId.value} " +
                    "(playlist ${last.playlistId.value})" +
                    (last.positionMs?.let { " parked at ${it}ms" } ?: ""),
            )
            // The parked position rides the persist (surviving a second
            // process death) and arms the resume: the first play of this
            // same track seeks to it instead of restarting at 0:00.
            playerRepository.setCurrent(music, playlist, last.positionMs)
            last.positionMs?.let {
                parkedResumeMusicId = last.musicId
                parkedResumePositionMs = it
            }
            launchLyricFetch(last.musicId)
        }
    }

    /**
     * Current position in ms (for the PlayerVM poll).
     *
     * Stateful: while an optimistic seek override is active (see
     * [seek]), this returns the override target until the engine
     * observable lands within [SEEK_SETTLE_TOLERANCE_MS] of it — or the
     * override deadline passes (a seek the engine dropped/failed) — at
     * which point it clears the override and returns engine truth again.
     */
    fun getCurrentPosition(): Long {
        val engine = _cantodeEngine.value
        val engineMs = engine?.positionMs?.value ?: 0L
        val overrided = overridedSeek ?: run {
            // Parked deck (restored but not yet re-loaded): the engine
            // holds no source and reports 0 — surface the parked position
            // so the slider and the lock-screen seekbar show where the
            // session stopped.
            if (engine == null || engine.state.value == PlayerState.IDLE) {
                parkedResumePositionMs?.let { return it }
            }
            return engineMs
        }
        val settled = abs(engineMs - overrided.targetMs) <= SEEK_SETTLE_TOLERANCE_MS
        val expired = SystemClock.elapsedRealtime() >= overrided.expiresAtMs
        return if (settled || expired) {
            overridedSeek = null
            engineMs
        } else {
            overrided.targetMs
        }
    }

    /**
     * Buffered frontier in ms (media time) — how far ahead of playback
     * contiguous data is buffered, from the engine facade's 10 Hz poll.
     * Falls back to the duration for sources the engine can't map to
     * media time (non-buffering sources like local files are effectively
     * fully buffered), then to 0.
     */
    fun getBufferedPosition(): Long {
        val engine = _cantodeEngine.value ?: return 0L
        return engine.bufferedMs.value ?: engine.durationMs.value ?: 0L
    }

    fun play(id: MusicId, playlistId: PlaylistId, autoAdvance: Boolean = false, attempt: Int = 0) {
        val generation = ++playGeneration
        bridge.logRaw("info", "play(${id.value}): start (autoAdvance=$autoAdvance attempt=$attempt)")
        if (playerId < 0) {
            bridge.logRaw("error", "play: cantode player not ready"); return
        }
        val engine = _cantodeEngine.value ?: return

        // A natural-end replay (repeat-one auto-replay, or re-tapping the
        // finished track) must go through the fresh-load branch below so it
        // emits a countable MusicPlay — only a PAUSED current track resumes
        // in place. An ERROR current track does too: the machine wedges
        // play/pause in `Error` (a failed load ends there), so the only
        // way forward is a fresh load.
        val ended = engine.state.value == PlayerState.ENDED
        val errorState = engine.state.value == PlayerState.ERROR
        // An IDLE engine holds no source at all — same-track "resume"
        // would be a dead `engine.play()` no-op (that is the state after
        // a user stop parked the session, or a restored-but-not-yet-
        // loaded deck). Route it through the fresh-load path below, which
        // also applies the parked resume position.
        val freshSource = ended || errorState || engine.state.value == PlayerState.IDLE
        if (!freshSource && _music.value?.meta?.id == id && _playlist.value?.abstr?.meta?.id == playlistId) {
            bridge.logRaw("info", "play(${id.value}): same track → resume")
            resume(); return
        }

        runCatching { PlaybackService.start(cx) }

        // Stop the old track at tap time — its audio must not keep
        // sounding while the new track's metadata is read and its source
        // buffers. The engine reports Loading (spinner) as soon as
        // `player.loadMusic` below reaches the worker; the optimistic
        // flag keeps the UI in its loading state until then.
        engine.stop()
        // Position continuity breaks here — drop any live seek
        // override so it can't pin the old position onto the new track.
        overridedSeek = null
        playerRepository.setIsLoading(true)

        _scope.launch(Dispatchers.Main) {
            // No early `resetCurrent()` here: blanking the current music
            // first makes the title vanish, the mini bar disappear and the
            // slider go stale for the whole load window. `music.get` is
            // DB-only (the lyric arrives via the `lyricJob` follow-up
            // below), so `setCurrent(new)` lands within milliseconds —
            // until then the old track's UI stays visible, silent.
            _pluginEvents.tryEmit(
                PluginEvent.MusicStop(timestamp = System.currentTimeMillis())
            )

            val music: Music? = bridge.call(BridgeMethods.Music.GET, id).unwrapOrNull()?.payload
            val playlist: Playlist? = bridge.call(BridgeMethods.Playlist.GET, playlistId)
                .unwrapOrNull()?.payload
            val inPlaylist = music != null && playlist != null &&
                playlist.musics.any { it.meta.id == id }

            if (inPlaylist) {
                playerRepository.setCurrent(music!!, playlist!!)

                // Lyric follow-up: fetch + parse over the storage seam in
                // the background and patch the result into the current
                // music — the pane shows its LOADING spinner meanwhile.
                // Superseded by each new play() and id-guarded at apply
                // time, so a stale fetch can't land on a newer track.
                launchLyricFetch(id)

                // The load itself stays on the backend bridge — source
                // construction (storage plugins) and the metadata→DB
                // writeback are business logic. `autoplay` completes it
                // straight into Playing; no follow-up `play` command.
                val loaded = bridge.callRaw(
                    "player.loadMusic",
                    buildJsonObject {
                        put("backendHandle", bridge.getBackendId())
                        put("musicId", id.value)
                        put("autoplay", true)
                    },
                    handle = playerId,
                ).unwrapOrNull()

                if (loaded == null) {
                    // The load failed (backend logged the error envelope) —
                    // without this the UI sits on the optimistic loading
                    // flag / BUFFERING spinner forever. Nothing is playing:
                    // reset the state and surface a visible failure.
                    playerRepository.setIsLoading(false)
                    bridge.logRaw(
                        "error",
                        "play(${id.value}): loadMusic failed (attempt $attempt)",
                    )
                    // A screen-off auto-advance can fail on a throttled
                    // fresh connection (MIUI background limits stall the
                    // new track's probe until the watchdog gives up). Ride
                    // short throttles out with a backoff instead of ending
                    // the listening session; user-initiated plays surface
                    // the failure immediately (they're watching). The
                    // generation check keeps a stale retry from fighting
                    // a newer play().
                    if (autoAdvance && attempt < ADVANCE_LOAD_RETRY_MAX) {
                        _scope.launch {
                            delay(ADVANCE_LOAD_RETRY_DELAY_MS)
                            if (playGeneration == generation) {
                                bridge.logRaw(
                                    "info",
                                    "play(${id.value}): auto-advance retry " +
                                        "(attempt ${attempt + 1})",
                                )
                                play(id, playlistId, autoAdvance = true, attempt = attempt + 1)
                            }
                        }
                        return@launch
                    }
                    toastRepository.emitToast("Play failed")
                    return@launch
                }

                _pluginEvents.tryEmit(
                    PluginEvent.MusicPlay(
                        musicId = id,
                        title = music.meta.title,
                        timestamp = System.currentTimeMillis(),
                    )
                )

                // The load's Rust-side writeback (embedded cover art,
                // probed duration, backfilled artist) is inline in
                // `player.loadMusic`, so it has landed in the DB by now.
                // Re-fetch and patch the current music so the player UI
                // shows the extracted cover + artist immediately; the
                // debounced playlist reload refreshes the playlist cards'
                // `showCover` (first music with a cover), and
                // `notifyMusicsUpdated` re-queries any open playlist
                // page's rows — the `_playlists` StateFlow can suppress
                // an unchanged abstract list, which used to leave
                // first-play artists invisible in the rows.
                bridge.call(BridgeMethods.Music.GET, id).unwrapOrNull()?.payload?.let { extracted ->
                    playerRepository.updateMusicExtractedMeta(id, extracted)
                    playlistRepository.scheduleReload()
                    playlistRepository.notifyMusicsUpdated()
                }

                // Parked-position resume: restoreLastPlayed rehydrated
                // this track from a mid-track stop / process death — the
                // fresh load above started at 0, so jump to the parked
                // position once. Guards skip a trivial position and the
                // end-of-track window (a completed session replays from
                // 0). Cleared unconditionally: playing any other track
                // abandons the parked position.
                val parkedId = parkedResumeMusicId
                val parkedMs = parkedResumePositionMs
                parkedResumeMusicId = null
                parkedResumePositionMs = null
                if (parkedId == id && parkedMs != null && parkedMs > RESUME_POSITION_MIN_MS) {
                    val durMs = playerRepository.music.value?.meta?.duration
                    if (durMs == null || parkedMs < durMs - RESUME_POSITION_TAIL_MS) {
                        bridge.logRaw(
                            "info",
                            "play(${id.value}): resume at parked ${parkedMs}ms",
                        )
                        seek(parkedMs.toULong())
                    }
                }

                // The same writeback also captures embedded tag lyrics —
                // which the earlier lyric fetch could not have seen on a
                // track's FIRST play (it races the load; the column was
                // still NULL). If that fetch ended MISSING and the probe
                // found lyric tags, re-run it once — the embedded text is
                // in the DB now. Gating on the probed tags keeps this a
                // no-op for the common lyricless/sidecar cases.
                val probeFoundLyricTag = runCatching {
                    loaded?.payload?.jsonObject?.get("tags")
                        ?.jsonArray
                        ?.any { tag ->
                            val obj = tag.jsonObject
                            obj["key"]?.jsonPrimitive?.content == "Lyrics" &&
                                !obj["value"]?.jsonPrimitive?.content.isNullOrBlank()
                        } ?: false
                }.getOrDefault(false)
                if (probeFoundLyricTag) {
                    // LOADING: the fetch launched before `loadMusic` is
                    // still in flight — but it read the DB before the
                    // writeback committed, so it is doomed to land
                    // MISSING; supersede it. MISSING: it already did.
                    // (First plays only — later plays always find the
                    // column filled on the first fetch.)
                    val state = _music.value?.takeIf { it.meta.id == id }?.lyric?.loadedState
                    if (state == null || state == LyricLoadState.LOADING || state == LyricLoadState.MISSING) {
                        launchLyricFetch(id)
                    }
                }
            } else if (music == null || playlist == null) {
                // Fetch FAILURE (backend busy / restarting / handle
                // invalidated), not proof the music is gone. Resetting
                // here is the "miniplayer disappears + playing page goes
                // empty" failure mode: this path runs on unattended
                // auto-advance too, e.g. while the screen is off. Keep
                // the current track's UI, surface the failure, and let
                // the user retry. The engine was already stopped above,
                // so nothing sounds — but the deck stays readable.
                bridge.logRaw(
                    "error",
                    "play($id): music/playlist fetch failed; keeping current state",
                )
                playerRepository.setIsLoading(false)
                toastRepository.emitToast("Play failed")
            } else {
                // Music/playlist really gone (removed meanwhile): reset
                // the UI and the loading flag — the engine was already
                // stopped above; nothing to load.
                playerRepository.setIsLoading(false)
                playerRepository.resetCurrent()
            }
        }
    }

    fun resume() {
        val engine = _cantodeEngine.value ?: return
        // The engine wedges play/pause once `Ended` or `Error` (by
        // design — see the state machine's rows for both; a failed load
        // ends in `Error`): without this, the play button at
        // end-of-track / end-of-playlist — or after a failed load — was
        // a dead no-op. An IDLE engine holds no source either (a user
        // stop parked the deck, or a restored-but-not-yet-loaded
        // session) — `engine.play()` would be equally dead. Replay the
        // current track via a fresh load instead (a brand-new source
        // epoch; the engine's poll error slot clears with the new
        // session; a parked position applies in [play]).
        val m = _music.value
        val p = _playlist.value
        if ((engine.state.value == PlayerState.ENDED ||
                engine.state.value == PlayerState.ERROR ||
                engine.state.value == PlayerState.IDLE) && m != null && p != null
        ) {
            play(m.meta.id, p.abstr.meta.id)
            return
        }
        // A session parked on a hard source error resumes through a
        // seek: the seek is the engine's retry epoch (it re-opens the
        // source with a fresh retry budget and clears the error), so
        // the play that follows is a genuine fresh attempt at the same
        // position instead of an instant re-error. Without it the
        // sticky source would fail the very next read.
        if (engine.error.value != null) {
            val pos = getCurrentPosition()
            bridge.logRaw("info", "resume: source error outstanding — seek to ${pos}ms (retry epoch) then play")
            runCatching { engine.seek(pos) }
                .onFailure { bridge.logRaw("error", "resume-after-error seek failed: $it") }
        }
        engine.play()
        _pluginEvents.tryEmit(
            PluginEvent.MusicResume(
                musicId = _music.value?.meta?.id,
                timestamp = System.currentTimeMillis(),
                positionMs = getCurrentPosition(),
            )
        )
    }

    fun pause() {
        val engine = _cantodeEngine.value ?: return
        engine.pause()
        // Park the position immediately — a notification dismissal or a
        // process death right after this pause must resume from here,
        // not from the last 5 s snapshot.
        playerRepository.savePositionSnapshot(getCurrentPosition())
        _pluginEvents.tryEmit(
            PluginEvent.MusicPause(
                musicId = _music.value?.meta?.id,
                timestamp = System.currentTimeMillis(),
                positionMs = getCurrentPosition(),
            )
        )
    }

    fun stop() {
        val engine = _cantodeEngine.value ?: return
        engine.stop()
        overridedSeek = null
        playerRepository.resetCurrent()
        _pluginEvents.tryEmit(
            PluginEvent.MusicStop(timestamp = System.currentTimeMillis())
        )
    }

    /**
     * User explicit stop (mini-bar X / notification dismissal /
     * transport stop): stop the audio and hide the deck, but PARK the
     * session — `last_played` survives with the parked position, so the
     * next foreground's [restoreLastPlayed] brings the track back and
     * the first play of it resumes in place instead of from 0:00.
     *
     * Contrast [stop] — the session-teardown variant used when the
     * deck's playlist/music/storage is being removed: it wipes the
     * record, since the parked pair could not be restored anyway.
     * A finished session (ENDED) parks with no position — replay from 0.
     */
    fun userStop() {
        val engine = _cantodeEngine.value ?: return
        val position = getCurrentPosition()
        val ended = engine.state.value == PlayerState.ENDED
        engine.stop()
        overridedSeek = null
        playerRepository.parkSession(if (ended) null else position)
        _pluginEvents.tryEmit(
            PluginEvent.MusicStop(timestamp = System.currentTimeMillis())
        )
    }

    private fun playOnComplete() {
        val m = playerRepository.onCompleteMusic.value ?: return
        val p = _playlist.value ?: return
        bridge.logRaw("info", "track completed → auto-advance to ${m.meta.id.value}")
        // autoAdvance: an advance failing on a screen-off throttle retries
        // with a backoff instead of silently ending the session (the user
        // isn't watching; nobody would see the toast until much later).
        play(m.meta.id, p.abstr.meta.id, autoAdvance = true)
    }

    fun playNext() {
        val m = nextMusic.value
        val p = _playlist.value
        if (m != null && p != null) {
            play(m.meta.id, p.abstr.meta.id)
        }
    }

    fun playPrevious() {
        val m = previousMusic.value
        val p = _playlist.value
        if (m != null && p != null) {
            play(m.meta.id, p.abstr.meta.id)
        }
    }

    fun seek(ms: ULong) {
        val engine = _cantodeEngine.value ?: return
        // Optimistic override: the engine applies the seek (decoder
        // seek + sink flush, possibly an out-of-window source reopen)
        // before its reply returns, but the new position only reaches
        // the UI through the 10 Hz engine poll + the 1 Hz VM poll — up
        // to ~1.1 s of stale position otherwise. Serve the target from
        // [getCurrentPosition] until the engine observable settles on
        // it (or the deadline gives up on a dropped seek).
        overridedSeek = OverridedSeek(
            targetMs = ms.toLong(),
            expiresAtMs = SystemClock.elapsedRealtime() + SEEK_SETTLE_TIMEOUT_MS,
        )
        engine.seek(ms.toLong())
    }

    fun scheduleSleep(newExpiredMs: Long) {
        _sleepJob?.cancel()
        val delayMs = max(newExpiredMs - System.currentTimeMillis(), 0)
        _sleepJob = _scope.launch {
            _sleep.update { it.copy(enabled = true, expiredMs = newExpiredMs) }
            bridge.logRaw("info", "schedule sleep")
            delay(delayMs)
            bridge.logRaw("info", "sleep scheduled")
            playerRepository.emitPauseRequest()
            _sleep.update { it.copy(enabled = false, expiredMs = 0) }
        }
    }

    fun refreshPlaylistIfMatch(playlist: Playlist) {
        playerRepository.refreshPlaylistIfMatch(playlist)
    }

    fun cancelSleep() {
        _sleepJob?.cancel()
        _sleepJob = null
        _sleep.update { it.copy(enabled = false, expiredMs = 0) }
    }

    fun remove() {
        val m = _music.value
        val p = _playlist.value
        _scope.launch {
            if (m != null && p != null) {
                bridge.call(
                    BridgeMethods.Playlist.REMOVE_MUSIC,
                    ArgRemoveMusicFromPlaylist(
                        playlistId = p.abstr.meta.id,
                        musicId = m.meta.id,
                    ),
                ).unwrapOrNull()
            }
        }
    }

    /** Optimistic seek target held until the engine position settles on it. */
    private data class OverridedSeek(
        val targetMs: Long,
        val expiresAtMs: Long,
    )

    companion object {
        /** Engine position within this window of the seek target counts as settled. */
        private const val SEEK_SETTLE_TOLERANCE_MS = 500L

        /** Give up on the override after this long (failed/unsupported seeks revert to engine truth). */
        private const val SEEK_SETTLE_TIMEOUT_MS = 3_000L

        /** Auto-advance load retries: a screen-off advance can fail on a
         *  throttled fresh connection (MIUI background limits stall the
         *  probe until the watchdog gives up); ride it out with a backoff
         *  instead of ending the listening session. */
        private const val ADVANCE_LOAD_RETRY_MAX = 2
        private const val ADVANCE_LOAD_RETRY_DELAY_MS = 5_000L

        /** Parked-resume guards (applied in [play]'s post-load): a
         *  position this close to 0 isn't worth a seek, and one this
         *  close to the end means the session had completed — replay
         *  from 0 instead. */
        private const val RESUME_POSITION_MIN_MS = 5_000L
        private const val RESUME_POSITION_TAIL_MS = 10_000L

        /** How often the persisted `last_played` position is refreshed
         *  while audio is active — the resume-position granularity for
         *  process deaths. */
        private const val POSITION_SNAPSHOT_INTERVAL_MS = 5_000L
    }
}
