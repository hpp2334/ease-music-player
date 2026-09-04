package com.kutedev.cantode

/**
 * The JNI surface exported by cantode's Rust `ffi` feature
 * (`Java_com_kutedev_cantode_CantodeNative_*`), compiled into
 * `libease_client_backend.so` — the same native library the host app
 * already ships; no second `.so`.
 *
 * All functions take the **player handle** — the same opaque id the
 * embedder uses on its own bridge (registered from `player.new` via
 * `cantode::ffi::register_player`). A stale handle is a safe no-op /
 * empty poll.
 */
internal object CantodeNative {
    init {
        System.loadLibrary("ease_client_backend")
    }

    /**
     * Batched engine snapshot as JSON:
     * `{"state":"LOADING","stateSeq":3,"transitions":[{"seq":1,"state":"LOADING"},…],"positionMs":123,"durationMs":210000,"bufferedMs":15000}`
     * (durationMs / bufferedMs `null` while unknown). Empty string = no
     * player under this handle (gone / not yet created) — treat as a
     * skipped poll tick.
     */
    external fun poll(handle: Long, sinceSeq: Long): String

    /** Begin or resume playback. */
    external fun play(handle: Long)

    /** Pause playback. */
    external fun pause(handle: Long)

    /** Stop and drop the loaded source (back to `Idle`). */
    external fun stop(handle: Long)

    /** Seek to `ms` from source start (no-op without a loaded source). */
    external fun seek(handle: Long, ms: Long)

    /** Set linear gain: `1.0` unity, `0.0` silent. */
    external fun setVolume(handle: Long, volume: Float)

    /**
     * Load the pre-registered source (`sourceToken` from the embedder's
     * `register_source`) straight into `Playing`. Blocks until the source
     * is open — call from a worker dispatcher. `false` = no player / no
     * such token / load failed (the next poll reports `ERROR`).
     */
    external fun loadAndPlay(handle: Long, sourceToken: Long): Boolean
}
